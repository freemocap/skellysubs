//! SQLite persistence for native graphs. Owners provide authority/publication;
//! this module never decides readiness or constructs a second operation model.
use super::graph::*;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};

mod detached;
mod reads;
pub use detached::DetachedHistory;
#[cfg(test)]
mod tests;
pub use reads::{BorrowedReadStore, ReadStore};

#[derive(Clone)]
pub struct Partition {
    pub owner: Owner,
    pub catalog: String,
}

/// Product ownership selects storage, never execution semantics or topology.
#[derive(Clone)]
pub enum Owner {
    Conversation(String),
    Workspace(String),
}
impl Owner {
    pub fn key(&self) -> &str {
        match self {
            Self::Conversation(id) | Self::Workspace(id) => id,
        }
    }
}
impl Partition {
    fn engines(&self) -> &'static str {
        match self.owner {
            Owner::Conversation(_) => "graph_engines",
            Owner::Workspace(_) => "workspace_graph_engines",
        }
    }
    fn records(&self) -> &'static str {
        match self.owner {
            Owner::Conversation(_) => "graph_records",
            Owner::Workspace(_) => "workspace_graph_records",
        }
    }
    fn archives(&self) -> &'static str {
        match self.owner {
            Owner::Conversation(_) => "graph_archives",
            Owner::Workspace(_) => "workspace_graph_archives",
        }
    }
    fn owner_column(&self) -> &'static str {
        match self.owner {
            Owner::Conversation(_) => "conversation_id",
            Owner::Workspace(_) => "workspace_id",
        }
    }
}

/// Exact sorted artifact set, independent of registration order or app version.
pub fn catalog_id<'a>(artifacts: impl IntoIterator<Item = &'a str>) -> Result<String> {
    let mut ids: Vec<_> = artifacts.into_iter().collect();
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(fault("duplicate_artifact", "catalog"));
    }
    let bytes = serde_json::to_vec(&ids).map_err(|_| fault("serialization", "catalog"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Owns the command's real transaction. The owner callback checks current
/// authority and stages domain publication/revision in that transaction. It must
/// not commit, dispatch, perform external effects or acknowledge success itself.
/// Discard this adapter after an error, including pre-commit graph validation.
/// The caller holds workspace serialization through commit and host replacement.
/// The borrowed native request supplies the exact engine identity, including
/// first admission before that engine's row has been inserted.
pub struct TransactionStore<'a, F> {
    tx: Option<Transaction<'a>>,
    partition: Partition,
    record_bytes: usize,
    owner: F,
}

impl<'a, F> TransactionStore<'a, F> {
    pub fn new(tx: Transaction<'a>, partition: Partition, record_bytes: usize, owner: F) -> Self {
        Self {
            tx: Some(tx),
            partition,
            record_bytes,
            owner,
        }
    }

    fn db(&self) -> Result<&Connection> {
        self.tx
            .as_deref()
            .ok_or_else(|| fault("transaction_closed", "graph_store"))
    }
}

impl<F> RecordStore for TransactionStore<'_, F> {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize> {
        reads::count(self.db()?, &self.partition, expected)
    }
    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        reads::record(self.db()?, &self.partition, expected, key, max_bytes)
    }
}
impl<F> HistoryStore for TransactionStore<'_, F> {
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        reads::archive(self.db()?, &self.partition, expected, max_bytes)
    }
}

impl<F> CommitStore for TransactionStore<'_, F>
where
    F: FnMut(&Connection, &CommitRequest<'_>) -> Result<()>,
{
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure> {
        let tx = self
            .tx
            .take()
            .ok_or_else(|| CommitFailure::Rejected(fault("transaction_closed", "graph_store")))?;
        let staged = (|| {
            let foreign_keys: bool = tx
                .pragma_query_value(None, "foreign_keys", |r| r.get(0))
                .map_err(|e| sql(e, "foreign_keys"))?;
            if !foreign_keys {
                return Err(fault("graph_storage_foreign_keys_required", "transaction"));
            }
            if catalog_id(request.next.artifact_ids())? != self.partition.catalog {
                return Err(fault("artifact_mismatch", "catalog"));
            }
            match request.expected {
                Some(expected) => {
                    reads::check_stamp(&tx, &self.partition, expected)?;
                    if request.next.stamp().engine != expected.engine {
                        return Err(fault("checkpoint_identity", "engine"));
                    }
                }
                None => {
                    if reads::stamp(&tx, &self.partition)?.is_some() {
                        return Err(fault("record_stamp", "create"));
                    }
                }
            }
            (self.owner)(&tx, &request)?;
            let stamp = encode(request.next.stamp(), "stamp")?;
            if request.expected.is_none() {
                tx.execute(
                    &format!(
                        "INSERT INTO {}(id,{},catalog,stamp,checkpoint) VALUES(?1,?2,?3,?4,?5)",
                        self.partition.engines(),
                        self.partition.owner_column()
                    ),
                    params![
                        request.next.stamp().engine,
                        self.partition.owner.key(),
                        self.partition.catalog,
                        stamp,
                        request.next.bytes()
                    ],
                )
                .map_err(|e| sql(e, "create"))?;
            } else {
                let changed = tx.execute(&format!("UPDATE {} SET stamp=?1,checkpoint=?2 WHERE id=?3 AND {}=?4 AND catalog=?5", self.partition.engines(), self.partition.owner_column()),
                    params![stamp, request.next.bytes(), request.next.stamp().engine, self.partition.owner.key(), self.partition.catalog])
                    .map_err(|e| sql(e, "checkpoint"))?;
                if changed != 1 {
                    return Err(fault("record_stamp", "checkpoint"));
                }
            }
            if let CommitIntent::Compact { archive } = request.intent {
                if request.expected != Some(archive.stamp())
                    || request.next.archive_parent() != Some(archive.stamp())
                {
                    return Err(fault("history_mismatch", "archive_parent"));
                }
                retain_archive(&tx, &self.partition, archive)?;
            }
            for row in request.records.iter() {
                let key = encode(row.key(), "record_key")?;
                let payload = row.bytes(self.record_bytes)?;
                tx.execute(&format!("INSERT INTO {}(engine_id,key,payload) VALUES(?1,?2,?3) ON CONFLICT(engine_id,key) DO UPDATE SET payload=excluded.payload", self.partition.records()),
                    params![request.next.stamp().engine, key, payload]).map_err(|e| sql(e, "record_write"))?;
            }
            Ok(())
        })();
        if let Err(cause) = staged {
            return match tx.rollback() {
                Ok(()) => Err(CommitFailure::Rejected(cause)),
                Err(e) => Err(CommitFailure::Indeterminate(sql(e, "rollback"))),
            };
        }
        tx.commit()
            .map_err(|e| CommitFailure::Indeterminate(sql(e, "commit")))
    }
}

fn retain_archive(db: &Connection, partition: &Partition, archive: &Checkpoint) -> Result<()> {
    let stamp = encode(archive.stamp(), "archive_stamp")?;
    let existing: Option<String> = db
        .query_row(
            &format!(
                "SELECT stamp FROM {} WHERE engine_id=?1 AND checksum=?2",
                partition.archives()
            ),
            params![archive.stamp().engine, archive.stamp().checksum],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| sql(e, "archive_stamp"))?;
    if let Some(existing) = existing {
        let body = reads::blob(
            db,
            partition.archives(),
            "payload",
            "engine_id=?1 AND checksum=?2",
            &[&archive.stamp().engine, &archive.stamp().checksum],
            archive.bytes().len(),
        )?;
        if existing != stamp || body != archive.bytes() {
            return Err(fault("history_mismatch", "archive"));
        }
    } else {
        db.execute(
            &format!("INSERT INTO {} VALUES(?1,?2,?3,?4)", partition.archives()),
            params![
                archive.stamp().engine,
                archive.stamp().checksum,
                stamp,
                archive.bytes()
            ],
        )
        .map_err(|e| sql(e, "archive_write"))?;
    }
    Ok(())
}

fn encode(value: &impl serde::Serialize, path: &str) -> Result<String> {
    serde_json::to_string(value).map_err(|_| fault("serialization", path))
}
fn fault(code: &str, path: &str) -> Fault {
    Fault {
        code: code.into(),
        path: path.into(),
    }
}
pub(crate) fn sql(error: rusqlite::Error, stage: &str) -> Fault {
    // Retain bounded SQLite codes, never SQL text, source values or raw errors.
    let code = match error.sqlite_error() {
        Some(e) => format!(
            "graph_storage_sqlite_{}_{}",
            e.extended_code & 0xff,
            e.extended_code
        ),
        None => match error {
            rusqlite::Error::QueryReturnedNoRows => "graph_storage_missing_row".into(),
            rusqlite::Error::InvalidColumnType(index, _, actual) => {
                format!("graph_storage_column_type_{index}_{actual:?}")
            }
            rusqlite::Error::IntegralValueOutOfRange(index, _) => {
                format!("graph_storage_integer_range_{index}")
            }
            _ => "graph_storage_decode_details_omitted".into(),
        },
    };
    Fault {
        code,
        path: stage.into(),
    }
}
