//! Bounded immutable history bytes copied inside one read transaction. Decoding
//! and replay happen after releasing the application's store guard.
use super::*;
use std::collections::BTreeMap;

pub struct DetachedHistory {
    expected: Stamp,
    catalog: String,
    checkpoint: Vec<u8>,
    archives: BTreeMap<String, (Stamp, Vec<u8>)>,
}

impl DetachedHistory {
    pub fn capture(
        db: &Connection,
        partition: &Partition,
        checkpoint_limit: usize,
        history: HistoryLimits,
    ) -> Result<Self> {
        if db.is_autocommit() {
            return Err(fault("transaction_required", "detached_history"));
        }
        let expected =
            reads::stamp(db, partition)?.ok_or_else(|| fault("record_missing", "engine"))?;
        let checkpoint = reads::blob(
            db,
            partition.engines(),
            "checkpoint",
            "id=?1",
            &[&expected.engine],
            checkpoint_limit,
        )?;
        let mut query = db
            .prepare(&format!(
                "SELECT checksum,stamp,length(payload) FROM {} WHERE engine_id=?1 LIMIT ?2",
                partition.archives()
            ))
            .map_err(|e| sql(e, "archive_list"))?;
        let rows = query
            .query_map(
                params![
                    expected.engine,
                    i64::try_from(history.segments.saturating_add(1)).unwrap_or(i64::MAX)
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .map_err(|e| sql(e, "archive_list"))?;
        let mut archives = BTreeMap::new();
        let mut remaining = history.bytes;
        for row in rows {
            let (checksum, raw, size) = row.map_err(|e| sql(e, "archive_list"))?;
            if archives.len() >= history.segments
                || usize::try_from(size)
                    .ok()
                    .is_none_or(|size| size > remaining)
            {
                return Err(fault("record_byte_limit", "detached_history"));
            }
            let stamp: Stamp = serde_json::from_str(&raw)
                .map_err(|_| fault("history_mismatch", "archive_stamp"))?;
            if stamp.engine != expected.engine || stamp.checksum != checksum {
                return Err(fault("history_mismatch", "archive_stamp"));
            }
            let bytes = reads::blob(
                db,
                partition.archives(),
                "payload",
                "engine_id=?1 AND checksum=?2",
                &[&expected.engine, &checksum],
                remaining,
            )?;
            remaining -= bytes.len();
            archives.insert(checksum, (stamp, bytes));
        }
        Ok(Self {
            expected,
            catalog: partition.catalog.clone(),
            checkpoint,
            archives,
        })
    }

    pub fn stamp(&self) -> &Stamp {
        &self.expected
    }

    /// Cache hits require identical source bytes, not merely a claimed SQL stamp.
    /// Hash outside the store guard; a modified/corrupt body must be revalidated.
    pub fn cache_identity(&self) -> Result<String> {
        let mut hash = Sha256::new();
        hash.update(encode(&(&self.expected, &self.catalog), "cache_identity")?);
        hash.update((self.checkpoint.len() as u64).to_le_bytes());
        hash.update(&self.checkpoint);
        for (key, (stamp, bytes)) in &self.archives {
            hash.update(encode(&(key, stamp), "cache_identity")?);
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        Ok(format!("{:x}", hash.finalize()))
    }

    pub fn checkpoint(&self, limits: CheckpointLimits) -> Result<Checkpoint> {
        let checkpoint = Checkpoint::decode(&self.checkpoint, limits)?;
        if checkpoint.stamp() != &self.expected
            || catalog_id(checkpoint.artifact_ids())? != self.catalog
        {
            return Err(fault("checkpoint_identity", "checkpoint"));
        }
        Ok(checkpoint)
    }
}

impl HistoryStore for DetachedHistory {
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        let (stamp, bytes) = self
            .archives
            .get(&expected.checksum)
            .ok_or_else(|| fault("record_missing", "archive"))?;
        if stamp != expected || bytes.len() > max_bytes {
            return Err(fault("history_mismatch", "archive"));
        }
        Ok(bytes.clone())
    }
}
