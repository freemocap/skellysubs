use super::*;
use rusqlite::{Connection, OptionalExtension};

pub(super) struct SqlStore {
    pub conn: Connection,
    pub fail_before_commit: bool,
    pub fail_after_commit: bool,
    pub fail_record_after: Option<usize>,
    pub last_record_writes: Vec<RecordKey>,
}
impl SqlStore {
    pub fn open(path: &std::path::Path) -> Self {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS checkpoint (id INTEGER PRIMARY KEY CHECK(id=1), stamp TEXT NOT NULL, payload BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS archive (engine TEXT, checksum TEXT, payload BLOB NOT NULL, PRIMARY KEY(engine,checksum));
            CREATE TABLE IF NOT EXISTS authority (run TEXT PRIMARY KEY, scope TEXT NOT NULL, input TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS publication (engine TEXT, run TEXT, node TEXT, attempt TEXT, value TEXT, PRIMARY KEY(engine,run,node,attempt));").unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS graph_record (key TEXT PRIMARY KEY, payload BLOB NOT NULL);").unwrap();
        Self {
            conn,
            fail_before_commit: false,
            fail_after_commit: false,
            fail_record_after: None,
            last_record_writes: Vec::new(),
        }
    }
    pub fn authorize(&self, run: &str, scope: &str) {
        self.conn.execute("INSERT INTO authority VALUES (?1,?2,?3) ON CONFLICT(run) DO UPDATE SET scope=excluded.scope", rusqlite::params![run,scope,serde_json::to_string(&values(40)).unwrap()]).unwrap();
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.conn
            .query_row("SELECT payload FROM checkpoint WHERE id=1", [], |r| {
                r.get(0)
            })
            .unwrap()
    }
    pub fn publications(&self) -> Vec<String> {
        self.conn
            .prepare("SELECT value FROM publication ORDER BY run,node,attempt")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }
}
fn rejected(code: &str) -> CommitFailure {
    CommitFailure::Rejected(unclassified(code, "fixture_store"))
}
impl CommitStore for SqlStore {
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure> {
        let tx = self
            .conn
            .transaction()
            .map_err(|_| rejected("storage_begin"))?;
        self.last_record_writes = commit_transaction(
            tx,
            request,
            self.fail_record_after,
            self.fail_before_commit,
            self.fail_after_commit,
        )?;
        Ok(())
    }
}

/// Consumes the actual owner transaction, including changes staged before the
/// graph transition. Success means COMMIT, never release of a savepoint.
pub(super) fn commit_transaction(
    tx: rusqlite::Transaction<'_>,
    request: CommitRequest<'_>,
    fail_record_after: Option<usize>,
    fail_before_commit: bool,
    fail_after_commit: bool,
) -> std::result::Result<Vec<RecordKey>, CommitFailure> {
    let previous: Option<String> = tx
        .query_row("SELECT stamp FROM checkpoint WHERE id=1", [], |r| r.get(0))
        .optional()
        .map_err(|_| rejected("storage_read"))?;
    let expected = request.expected.map(|s| serde_json::to_string(s).unwrap());
    if previous != expected {
        return Err(rejected("stale_checkpoint"));
    }
    let authority = match &request.intent {
        CommitIntent::Begin { authority: a, .. }
        | CommitIntent::Dispatch { authority: a, .. }
        | CommitIntent::Adopt { authority: a, .. } => Some(a),
        CommitIntent::Record | CommitIntent::Compact { .. } => None,
    };
    if let Some(a) = authority {
        let current: Option<String> = tx
            .query_row("SELECT scope FROM authority WHERE run=?1", [a.run], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|_| rejected("authority_read"))?;
        if current.as_deref() != Some(a.scope) {
            return Err(rejected("authority_changed"));
        }
        if let CommitIntent::Begin { inputs, .. } = &request.intent {
            let current: String = tx
                .query_row("SELECT input FROM authority WHERE run=?1", [a.run], |r| {
                    r.get(0)
                })
                .map_err(|_| rejected("source_read"))?;
            let expected: Values =
                serde_json::from_str(&current).map_err(|_| rejected("source_decode"))?;
            if **inputs != expected {
                return Err(rejected("source_changed"));
            }
        }
    }
    if let CommitIntent::Adopt {
        authority,
        attempt,
        values,
        ..
    } = &request.intent
    {
        tx.execute(
            "INSERT INTO publication VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![
                request.next.stamp().engine,
                authority.run,
                authority.node,
                attempt.0.to_string(),
                serde_json::to_string(values).unwrap()
            ],
        )
        .map_err(|_| rejected("publication_failed"))?;
    }
    if let CommitIntent::Compact { archive } = request.intent {
        if request.expected != Some(archive.stamp())
            || request.next.archive_parent() != Some(archive.stamp())
        {
            return Err(rejected("archive_reference"));
        }
        let existing: Option<Vec<u8>> = tx
            .query_row(
                "SELECT payload FROM archive WHERE engine=?1 AND checksum=?2",
                rusqlite::params![archive.stamp().engine, archive.stamp().checksum],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| rejected("archive_read"))?;
        if let Some(existing) = existing {
            if existing != archive.bytes() {
                return Err(rejected("archive_conflict"));
            }
        } else {
            tx.execute(
                "INSERT INTO archive VALUES (?1,?2,?3)",
                rusqlite::params![
                    archive.stamp().engine,
                    archive.stamp().checksum,
                    archive.bytes()
                ],
            )
            .map_err(|_| rejected("archive_write"))?;
        }
    }
    tx.execute("INSERT INTO checkpoint VALUES (1,?1,?2) ON CONFLICT(id) DO UPDATE SET stamp=excluded.stamp,payload=excluded.payload",rusqlite::params![serde_json::to_string(request.next.stamp()).unwrap(),request.next.bytes()]).map_err(|_|rejected("checkpoint_write"))?;
    let writes = super::sql_records::write(&tx, &request.records, fail_record_after)?;
    if fail_before_commit {
        return Err(rejected("injected_rollback"));
    }
    tx.commit().map_err(|_| {
        CommitFailure::Indeterminate(unclassified("commit_uncertain", "fixture_store"))
    })?;
    if fail_after_commit {
        return Err(CommitFailure::Indeterminate(unclassified(
            "acknowledgment_lost",
            "fixture_store",
        )));
    }
    Ok(writes)
}
impl HistoryStore for SqlStore {
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        let length: Option<i64> = self
            .conn
            .query_row(
                "SELECT length(payload) FROM archive WHERE engine=?1 AND checksum=?2",
                rusqlite::params![expected.engine, expected.checksum],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| unclassified("archive_read", "fixture_store"))?;
        let length = length.ok_or_else(|| unclassified("archive_missing", "fixture_store"))?;
        if usize::try_from(length).ok().is_none_or(|n| n > max_bytes) {
            return Err(unclassified("history_limit", "checkpoint"));
        }
        // SQL bounds the returned BLOB too; a changed row between reads cannot
        // force an allocation above the caller's declared read limit.
        self.conn.query_row(
            "SELECT payload FROM archive WHERE engine=?1 AND checksum=?2 AND length(payload)=?3",
            rusqlite::params![expected.engine, expected.checksum, length], |r| r.get(0),
        ).map_err(|_| unclassified("archive_read", "fixture_store"))
    }
}
