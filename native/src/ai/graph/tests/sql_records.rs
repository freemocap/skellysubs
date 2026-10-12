//! Disposable SQLite adapter for the native record transaction contract.
use super::{durable_store::SqlStore, *};
use crate::ai::graph::model::fault;
use rusqlite::{OptionalExtension, Transaction};

pub(super) fn write(
    tx: &Transaction<'_>,
    rows: &RecordChanges<'_>,
    fail_after: Option<usize>,
) -> std::result::Result<Vec<RecordKey>, CommitFailure> {
    let mut written = Vec::new();
    for row in rows.iter() {
        let bytes = row.bytes(2_000_000).map_err(CommitFailure::Rejected)?;
        tx.execute("INSERT INTO graph_record VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload",
            rusqlite::params![serde_json::to_string(row.key()).unwrap(), bytes])
            .map_err(|_| CommitFailure::Rejected(unclassified("record_write", "fixture_store")))?;
        written.push(row.key().clone());
        if fail_after == Some(written.len()) {
            return Err(CommitFailure::Rejected(unclassified(
                "record_write_injected",
                "fixture_store",
            )));
        }
    }
    Ok(written)
}

pub(super) fn check_stamp(tx: &rusqlite::Connection, expected: &Stamp) -> Result<()> {
    let stamp: Option<String> = tx
        .query_row("SELECT stamp FROM checkpoint WHERE id=1", [], |r| r.get(0))
        .optional()
        .map_err(|_| fault(CoreFaultCode::RecordRead, "stamp"))?;
    if stamp != Some(serde_json::to_string(expected).unwrap()) {
        return Err(fault(CoreFaultCode::RecordStamp, "stamp"));
    }
    Ok(())
}

impl RecordStore for SqlStore {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize> {
        let tx = self
            .conn
            .transaction()
            .map_err(|_| fault(CoreFaultCode::RecordRead, "transaction"))?;
        check_stamp(&tx, expected)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM graph_record", [], |r| r.get(0))
            .map_err(|_| fault(CoreFaultCode::RecordRead, "count"))?;
        usize::try_from(count).map_err(|_| fault(CoreFaultCode::RecordRead, "count"))
    }

    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        let tx = self
            .conn
            .transaction()
            .map_err(|_| fault(CoreFaultCode::RecordRead, "transaction"))?;
        read(&tx, expected, key, max_bytes)
    }
}

pub(super) fn read(
    tx: &rusqlite::Connection,
    expected: &Stamp,
    key: &RecordKey,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    check_stamp(tx, expected)?;
    let key = serde_json::to_string(key).unwrap();
    let length: Option<i64> = tx
        .query_row(
            "SELECT length(payload) FROM graph_record WHERE key=?1",
            [&key],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| fault(CoreFaultCode::RecordRead, "length"))?;
    let length = length.ok_or_else(|| fault(CoreFaultCode::RecordMissing, "record"))?;
    if usize::try_from(length).ok().is_none_or(|n| n > max_bytes) {
        return Err(fault(CoreFaultCode::RecordByteLimit, "record"));
    }
    tx.query_row(
        "SELECT payload FROM graph_record WHERE key=?1 AND length(payload)=?2",
        rusqlite::params![key, length],
        |r| r.get(0),
    )
    .map_err(|_| fault(CoreFaultCode::RecordRead, "body"))
}
