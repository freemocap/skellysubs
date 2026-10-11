use super::*;

/// Read native history inside an existing domain transaction. This borrows its
/// snapshot and never commits, recovers, or reconstructs state using SQL rules.
pub struct BorrowedReadStore<'a> {
    db: &'a Connection,
    partition: Partition,
}
impl<'a> BorrowedReadStore<'a> {
    pub fn new(db: &'a Connection, partition: Partition) -> Result<Self> {
        if db.is_autocommit() {
            return Err(fault("transaction_required", "graph_read"));
        }
        Ok(Self { db, partition })
    }
    pub fn checkpoint(&self, limits: CheckpointLimits) -> Result<Option<Checkpoint>> {
        read_checkpoint(self.db, &self.partition, limits)
    }
}
impl RecordStore for BorrowedReadStore<'_> {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize> {
        count(self.db, &self.partition, expected)
    }
    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        record(self.db, &self.partition, expected, key, max_bytes)
    }
}
impl HistoryStore for BorrowedReadStore<'_> {
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        archive(self.db, &self.partition, expected, max_bytes)
    }
}

fn read_checkpoint(
    db: &Connection,
    partition: &Partition,
    limits: CheckpointLimits,
) -> Result<Option<Checkpoint>> {
    let Some(expected) = stamp(db, partition)? else {
        return Ok(None);
    };
    let body = blob(
        db,
        partition.engines(),
        "checkpoint",
        "id=?1",
        &[&expected.engine],
        limits.bytes,
    )?;
    let checkpoint = Checkpoint::decode(&body, limits)?;
    if checkpoint.stamp() != &expected
        || catalog_id(checkpoint.artifact_ids())? != partition.catalog
    {
        return Err(fault("checkpoint_identity", "checkpoint"));
    }
    Ok(Some(checkpoint))
}

/// A stable protected read snapshot. No recovery, mutation or fallback to cache.
pub struct ReadStore<'a> {
    tx: Transaction<'a>,
    partition: Partition,
}
impl<'a> ReadStore<'a> {
    pub fn from_transaction(tx: Transaction<'a>, partition: Partition) -> Self {
        Self { tx, partition }
    }
    pub fn new(db: &'a mut Connection, partition: Partition) -> Result<Self> {
        Ok(Self {
            tx: db.transaction().map_err(|e| sql(e, "read_begin"))?,
            partition,
        })
    }
    pub fn checkpoint(&self, limits: CheckpointLimits) -> Result<Option<Checkpoint>> {
        read_checkpoint(&self.tx, &self.partition, limits)
    }
}
impl RecordStore for ReadStore<'_> {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize> {
        count(&self.tx, &self.partition, expected)
    }
    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        record(&self.tx, &self.partition, expected, key, max_bytes)
    }
}
impl HistoryStore for ReadStore<'_> {
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        archive(&self.tx, &self.partition, expected, max_bytes)
    }
}

pub(super) fn stamp(db: &Connection, partition: &Partition) -> Result<Option<Stamp>> {
    let row: Option<(String, String)> = db
        .prepare_cached(&format!(
            "SELECT id,stamp FROM {} WHERE {}=?1 AND catalog=?2",
            partition.engines(),
            partition.owner_column()
        ))
        .map_err(|e| sql(e, "stamp_read"))?
        .query_row(params![partition.owner.key(), partition.catalog], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(|e| sql(e, "stamp_read"))?;
    row.map(|(id, raw)| {
        let stamp: Stamp =
            serde_json::from_str(&raw).map_err(|_| fault("record_stamp", "stamp_decode"))?;
        if stamp.engine != id {
            return Err(fault("record_stamp", "engine"));
        }
        Ok(stamp)
    })
    .transpose()
}
pub(super) fn check_stamp(db: &Connection, partition: &Partition, expected: &Stamp) -> Result<()> {
    if stamp(db, partition)?.as_ref() != Some(expected) {
        return Err(fault("record_stamp", "checkpoint"));
    }
    Ok(())
}
pub(super) fn count(db: &Connection, partition: &Partition, expected: &Stamp) -> Result<usize> {
    check_stamp(db, partition, expected)?;
    let count: i64 = db
        .query_row(
            &format!(
                "SELECT count(*) FROM {} WHERE engine_id=?1",
                partition.records()
            ),
            [&expected.engine],
            |r| r.get(0),
        )
        .map_err(|e| sql(e, "record_count"))?;
    usize::try_from(count).map_err(|_| fault("record_read", "record_count"))
}
pub(super) fn record(
    db: &Connection,
    partition: &Partition,
    expected: &Stamp,
    key: &RecordKey,
    limit: usize,
) -> Result<Vec<u8>> {
    check_stamp(db, partition, expected)?;
    blob(
        db,
        partition.records(),
        "payload",
        "engine_id=?1 AND key=?2",
        &[&expected.engine, &encode(key, "record_key")?],
        limit,
    )
}
pub(super) fn archive(
    db: &Connection,
    partition: &Partition,
    expected: &Stamp,
    limit: usize,
) -> Result<Vec<u8>> {
    let current = stamp(db, partition)?.ok_or_else(|| fault("record_missing", "engine"))?;
    if current.engine != expected.engine {
        return Err(fault("record_stamp", "archive_engine"));
    }
    let stored: String = db
        .query_row(
            &format!(
                "SELECT stamp FROM {} WHERE engine_id=?1 AND checksum=?2",
                partition.archives()
            ),
            params![expected.engine, expected.checksum],
            |r| r.get(0),
        )
        .map_err(|e| sql(e, "archive_stamp"))?;
    let stored: Stamp =
        serde_json::from_str(&stored).map_err(|_| fault("history_mismatch", "archive_stamp"))?;
    if &stored != expected {
        return Err(fault("history_mismatch", "archive_stamp"));
    }
    blob(
        db,
        partition.archives(),
        "payload",
        "engine_id=?1 AND checksum=?2",
        &[&expected.engine, &expected.checksum],
        limit,
    )
}

// Identifiers/clauses are module-private constants; source values are parameters.
// One bounded query runs in the caller's read or owner transaction. CASE prevents
// allocating oversized/non-BLOB bodies. Bind the limit so the prepared statement
// is reusable across record sizes, without caching content or skipping validation.
pub(super) fn blob(
    db: &Connection,
    table: &str,
    column: &str,
    clause: &str,
    values: &[&dyn rusqlite::ToSql],
    limit: usize,
) -> Result<Vec<u8>> {
    let limit_parameter = values.len() + 1;
    let cap = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut parameters = values.to_vec();
    parameters.push(&cap);
    let row: Option<(i64, Option<Vec<u8>>)> = db
        .prepare_cached(&format!("SELECT length({column}), CASE WHEN typeof({column})='blob' AND length({column})<=?{limit_parameter} THEN {column} ELSE NULL END FROM {table} WHERE {clause}"))
        .map_err(|e| sql(e, "blob_read"))?
        .query_row(parameters.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(|e| sql(e, "blob_read"))?;
    let (length, body) = row.ok_or_else(|| fault("record_missing", "blob"))?;
    if usize::try_from(length).ok().is_none_or(|n| n > limit) {
        return Err(fault("record_byte_limit", "blob"));
    }
    body.ok_or_else(|| fault("record_mismatch", "blob"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_blob_query_rechecks_limits_type_and_content_on_every_read() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE sample(id INTEGER PRIMARY KEY, payload); INSERT INTO sample VALUES(1,x'010203'),(2,zeroblob(1000000)),(3,'text'),(4,x'');").unwrap();
        let read = |id: i32, limit| blob(&db, "sample", "payload", "id=?1", &[&id], limit);
        assert_eq!(read(1, 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(read(1, 2).unwrap_err().code, "record_byte_limit");
        assert_eq!(read(2, 2).unwrap_err().code, "record_byte_limit");
        assert_eq!(read(3, 4).unwrap_err().code, "record_mismatch");
        assert!(read(4, 0).unwrap().is_empty());
        assert_eq!(read(5, 10).unwrap_err().code, "record_missing");
        db.execute("UPDATE sample SET payload=x'040506' WHERE id=1", [])
            .unwrap();
        assert_eq!(read(1, 3).unwrap(), vec![4, 5, 6]);
    }
}
