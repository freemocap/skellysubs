//! The command coordinator transfers its already-open transaction at its final
//! commit point. This fixture exercises the existing core contract, not a second
//! graph transaction protocol or a production command adapter.
use super::{
    durable_store::{SqlStore, commit_transaction},
    *,
};
use rusqlite::{Connection, Transaction};

mod first_admission;

struct Enlisted<'a> {
    tx: Option<Transaction<'a>>,
    reject: bool,
    lose_ack: bool,
    reads: usize,
    fail_record_after: Option<usize>,
}

impl CommitStore for Enlisted<'_> {
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure> {
        commit_transaction(
            self.tx.take().unwrap(),
            request,
            self.fail_record_after,
            self.reject,
            self.lose_ack,
        )?;
        Ok(())
    }
}

impl RecordStore for Enlisted<'_> {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize> {
        let tx = self.tx.as_ref().unwrap();
        super::sql_records::check_stamp(tx, expected)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM graph_record", [], |r| r.get(0))
            .unwrap();
        Ok(usize::try_from(count).unwrap())
    }

    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        self.reads += 1;
        super::sql_records::read(self.tx.as_ref().unwrap(), expected, key, max_bytes)
    }
}

impl HistoryStore for Enlisted<'_> {
    fn read_archive(&mut self, _: &Stamp, _: usize) -> Result<Vec<u8>> {
        Err(unclassified(
            "unexpected_archive_read",
            "enlistment_fixture",
        ))
    }
}

fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        checkpoint: CheckpointLimits {
            bytes: 1_000_000,
            events: 100,
        },
        settlement_event_bytes: 4096,
        history: history_limits(),
        state: state_limits(),
    }
}

fn begin_event(graph: &Executable) -> Event {
    Event::Begin {
        run: "run".into(),
        artifact: graph.identity().into(),
        scope: "scope".into(),
        inputs: values(40),
        policy: BTreeMap::new(),
    }
}

fn owner_tables(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE command_source (id TEXT PRIMARY KEY);
        CREATE TABLE command_receipt (id TEXT PRIMARY KEY);",
    )
    .unwrap();
}

fn stage<'a>(conn: &'a mut Connection, action: &str) -> Enlisted<'a> {
    let tx = conn.transaction().unwrap();
    tx.execute("INSERT INTO command_source VALUES (?1)", [action])
        .unwrap();
    tx.execute("INSERT INTO command_receipt VALUES (?1)", [action])
        .unwrap();
    tx.execute(
        "INSERT OR IGNORE INTO authority VALUES ('run','scope',?1)",
        [serde_json::to_string(&values(40)).unwrap()],
    )
    .unwrap();
    Enlisted {
        tx: Some(tx),
        reject: false,
        lose_ack: false,
        reads: 0,
        fail_record_after: None,
    }
}

fn counts(conn: &Connection) -> (i64, i64) {
    (
        conn.query_row("SELECT count(*) FROM command_source", [], |r| r.get(0))
            .unwrap(),
        conn.query_row("SELECT count(*) FROM command_receipt", [], |r| r.get(0))
            .unwrap(),
    )
}

#[test]
fn admission_commits_domain_receipt_and_graph_together_or_rolls_everything_back() {
    for reject in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.db");
        let mut store = SqlStore::open(&path);
        owner_tables(&store.conn);
        let observer = Connection::open(&path).unwrap();
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        let before = store.bytes();
        let stamp = host.stamp().clone();
        let mut enlisted = stage(&mut store.conn, "admission");
        enlisted.reject = reject;
        assert_eq!(
            counts(&observer),
            (0, 0),
            "staged domain data is not committed"
        );
        assert_eq!(host.stamp(), &stamp);
        let result = host.apply(begin_event(&graph), &mut enlisted);
        assert!(
            enlisted.tx.is_none(),
            "commit consumed the enclosing transaction"
        );
        drop(enlisted);
        assert_eq!(result.is_err(), reject);
        assert!(!host.poisoned());
        if reject {
            assert_eq!(counts(&observer), (0, 0));
            assert_eq!(host.stamp(), &stamp);
            assert_eq!(store.bytes(), before);
            assert!(host.inspect("run").is_err());
        } else {
            assert_eq!(counts(&observer), (1, 1));
            assert_eq!(host.stamp().revision, stamp.revision + 1);
            let checkpoint = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
            let recovered =
                DurableEngine::recover(checkpoint, [graph], limits(), &mut store).unwrap();
            assert_eq!(
                recovered.inspect("run").unwrap().artifact_id,
                host.inspect("run").unwrap().artifact_id
            );
        }
    }
}

#[test]
fn validation_failure_before_commit_drops_the_owner_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    owner_tables(&store.conn);
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    let before = store.bytes();
    {
        let mut enlisted = stage(&mut store.conn, "invalid");
        let mut event = begin_event(&graph);
        if let Event::Begin { inputs, .. } = &mut event {
            *inputs = BTreeMap::new();
        }
        assert!(host.apply(event, &mut enlisted).is_err());
        assert!(enlisted.tx.is_some(), "no storage commit was attempted");
    }
    assert_eq!(counts(&store.conn), (0, 0));
    assert_eq!(store.bytes(), before);
    assert!(!host.poisoned());
}

#[test]
fn lost_ack_retains_receipt_and_graph_but_forbids_further_effects_until_reload() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    owner_tables(&store.conn);
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    {
        let mut enlisted = stage(&mut store.conn, "admission");
        enlisted.lose_ack = true;
        assert_eq!(
            host.apply(begin_event(&graph), &mut enlisted)
                .unwrap_err()
                .code,
            "acknowledgment_lost"
        );
    }
    assert_eq!(counts(&store.conn), (1, 1));
    assert!(host.poisoned());
    assert!(
        host.inspection_snapshot(
            "run",
            ExportLimits {
                bytes: 100_000,
                attempts: 100
            }
        )
        .is_err()
    );
    assert!(host.apply(capacity(1), &mut store).is_err());
    let checkpoint = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let recovered = DurableEngine::recover(checkpoint, [graph], limits(), &mut store).unwrap();
    assert!(recovered.inspect("run").is_ok());
    assert_eq!(
        counts(&store.conn),
        (1, 1),
        "recovery does not repeat command publication"
    );
}

#[test]
fn cold_dispatch_reads_inside_enlisted_transaction_and_exposes_invocation_only_after_commit() {
    for reject in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        owner_tables(&store.conn);
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        host.apply(
            begin_event(&graph),
            &mut stage(&mut store.conn, "admission"),
        )
        .unwrap();
        host.apply(capacity(1), &mut store).unwrap();
        let attempt = host.inspect("run").unwrap().attempts["first"][0].id;
        host.evict_records(&mut store).unwrap();
        let mut enlisted = stage(&mut store.conn, "dispatch");
        enlisted.reject = reject;
        let invocation = host.claim("run", "first", attempt, &mut enlisted);
        assert!(enlisted.reads > 0);
        assert!(enlisted.tx.is_none());
        drop(enlisted);
        assert_eq!(invocation.is_err(), reject);
        assert_eq!(counts(&store.conn), if reject { (1, 1) } else { (2, 2) });
        let inspection = host
            .read_inspection(
                "run",
                ExportLimits {
                    bytes: 100_000,
                    attempts: 100,
                },
                &mut store,
            )
            .unwrap();
        assert_eq!(
            inspection.nodes["first"],
            if reject {
                Disposition::Prepared
            } else {
                Disposition::Running
            }
        );
    }
}
