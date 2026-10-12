use super::{
    durable_store::SqlStore,
    record_access::{begin_host, limits},
    *,
};

#[derive(Default)]
struct Reads {
    count: usize,
    bytes: usize,
    caps: Vec<usize>,
}
struct Meter<'a> {
    store: &'a mut SqlStore,
    reads: Reads,
}
impl RecordStore for Meter<'_> {
    fn record_count(&mut self, stamp: &Stamp) -> Result<usize> {
        self.store.record_count(stamp)
    }
    fn read_record(&mut self, stamp: &Stamp, key: &RecordKey, max: usize) -> Result<Vec<u8>> {
        self.reads.count += 1;
        self.reads.caps.push(max);
        let bytes = self.store.read_record(stamp, key, max)?;
        self.reads.bytes += bytes.len();
        Ok(bytes)
    }
}
impl HistoryStore for Meter<'_> {
    fn read_archive(&mut self, stamp: &Stamp, max: usize) -> Result<Vec<u8>> {
        self.store.read_archive(stamp, max)
    }
}
impl CommitStore for Meter<'_> {
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure> {
        self.store.commit(request)
    }
}

fn export() -> ExportLimits {
    ExportLimits {
        bytes: 1_000_000,
        attempts: 1000,
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    store: SqlStore,
    host: DurableEngine,
    graph: Arc<Executable>,
    attempt: AttemptId,
    execution: ExecutionId,
}
impl Fixture {
    fn new(action: &str, cold: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        for run in ["a", "b"] {
            begin_host(&mut host, &mut store, &graph, run);
        }
        let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
        let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
        if action != "claim" {
            drop(host.claim("a", "first", attempt, &mut store).unwrap());
            if !matches!(action, "settle" | "recover" | "startup") {
                host.apply(
                    Event::Settle {
                        execution: work.execution,
                        outcome: Ok(values(41)),
                    },
                    &mut store,
                )
                .unwrap();
            }
        }
        if cold {
            host.evict_records(&mut store).unwrap();
        }
        // Leave a suffix while retaining cold consumer records for reservation.
        if action == "compact" {
            host.apply(
                Event::Pause {
                    run: "a".into(),
                    paused: false,
                },
                &mut store,
            )
            .unwrap();
        }
        Self {
            _dir: dir,
            store,
            host,
            graph,
            attempt,
            execution: work.execution,
        }
    }

    fn snapshot(&mut self) -> serde_json::Value {
        let mut value = serde_json::to_value(
            self.host
                .read_inspection("a", export(), &mut self.store)
                .unwrap(),
        )
        .unwrap();
        value["engine"] = json!(null);
        value
    }

    fn run(&mut self, action: &str, budget: RecordReadLimits) -> (Result<()>, Reads) {
        self.host.set_record_read_limits(budget).unwrap();
        let mut meter = Meter {
            store: &mut self.store,
            reads: Reads::default(),
        };
        let result = match action {
            "claim" => self
                .host
                .claim("a", "first", self.attempt, &mut meter)
                .map(drop),
            "adopt" => self.host.adopt("a", "first", self.attempt, &mut meter),
            "settle" => self
                .host
                .apply(
                    Event::Settle {
                        execution: self.execution,
                        outcome: Ok(values(41)),
                    },
                    &mut meter,
                )
                .map(drop),
            "advance" => self.host.apply(capacity(1), &mut meter).map(drop),
            "pause" => self
                .host
                .apply(
                    Event::Pause {
                        run: "a".into(),
                        paused: true,
                    },
                    &mut meter,
                )
                .map(drop),
            "cancel" => self
                .host
                .apply(
                    Event::Cancel {
                        run: "a".into(),
                        node: None,
                    },
                    &mut meter,
                )
                .map(drop),
            "recover" => self.host.apply(Event::Recover, &mut meter).map(drop),
            "inspect" => self
                .host
                .read_inspection("a", export(), &mut meter)
                .map(drop),
            "outputs" => self.host.read_outputs("a", &mut meter).map(drop),
            "adopted" => self
                .host
                .read_adopted_attempt("a", "first", &mut meter)
                .map(drop),
            "preview" => self
                .host
                .read_node_preview("a", "first", export().bytes, &mut meter)
                .map(drop),
            "inputs" => self
                .host
                .read_node_inputs("a", &[("first", "value"), ("second", "value")], &mut meter)
                .map(drop),
            "live" => self
                .host
                .read_live_inspection(
                    "a",
                    &[],
                    LiveReadLimits {
                        export: export(),
                        captures: 0,
                        capture_bytes: 2,
                    },
                    &mut meter,
                )
                .map(drop),
            "compact" => self.host.compact(&mut meter).map(drop),
            "startup" => {
                let checkpoint =
                    Checkpoint::decode(&meter.store.bytes(), limits().checkpoint).unwrap();
                DurableEngine::recover(
                    checkpoint,
                    [self.graph.clone()],
                    DurableLimits {
                        record_reads: budget,
                        ..limits()
                    },
                    &mut meter,
                )
                .map(|host| self.host = host)
            }
            _ => unreachable!(),
        };
        (result, meter.reads)
    }
}

#[test]
fn aggregate_limits_cover_owner_operations_and_refuse_atomically_at_exact_boundaries() {
    for cold in [false, true] {
        for action in [
            "claim", "adopt", "settle", "advance", "pause", "cancel", "recover", "inspect",
            "outputs", "inputs", "live", "adopted", "preview", "compact", "startup",
        ] {
            let mut reference = Fixture::new(action, cold);
            let (result, measured) = reference.run(action, record_read_limits());
            result.unwrap();
            let expected = reference.snapshot();
            // Warm compaction reserves from native rows and needs no row reads.
            if measured.count == 0 {
                assert_eq!(action, "compact");
                continue;
            }
            for dimension in ["records", "bytes", "exact"] {
                let mut fixture = Fixture::new(action, cold);
                let before = fixture.store.bytes();
                let stamp = fixture.host.stamp().clone();
                let resident = fixture.host.resident_usage().unwrap();
                let snapshot = fixture.snapshot();
                let budget = RecordReadLimits {
                    records: measured.count - usize::from(dimension == "records"),
                    bytes: measured.bytes - usize::from(dimension == "bytes"),
                };
                let (result, actual) = fixture.run(action, budget);
                if dimension == "exact" {
                    result.unwrap_or_else(|e| panic!("{cold} {action}: {e:?}"));
                    assert_eq!(actual.count, measured.count);
                    assert_eq!(actual.bytes, measured.bytes);
                    fixture
                        .host
                        .set_record_read_limits(record_read_limits())
                        .unwrap();
                    assert_eq!(fixture.snapshot(), expected, "{cold} {action}");
                } else {
                    assert_eq!(
                        result.unwrap_err().code,
                        if dimension == "records" {
                            "record_read_count_limit"
                        } else {
                            "record_read_byte_limit"
                        },
                        "{cold} {action} {dimension}"
                    );
                    assert_eq!(fixture.store.bytes(), before);
                    assert_eq!(fixture.host.stamp(), &stamp);
                    assert_eq!(fixture.host.resident_usage().unwrap(), resident);
                    assert!(!fixture.host.poisoned());
                    assert!(fixture.store.publications().is_empty());
                    fixture
                        .host
                        .set_record_read_limits(record_read_limits())
                        .unwrap();
                    assert_eq!(fixture.snapshot(), snapshot);
                    // Budget refusal is retryable without consuming IDs or effects.
                    fixture.run(action, record_read_limits()).0.unwrap();
                    assert_eq!(fixture.snapshot(), expected);
                }
            }
        }
    }
}

#[test]
fn remaining_bytes_reach_the_adapter_before_allocation_and_other_failures_survive() {
    use crate::ai::graph::read_budget::BudgetedStore;
    let mut fixture = Fixture::new("adopt", true);
    let stamp = fixture.host.stamp().clone();
    let key = RecordKey::Run("a".into());
    let row = fixture
        .store
        .read_record(&stamp, &key, limits().checkpoint.bytes)
        .unwrap();
    let mut meter = Meter {
        store: &mut fixture.store,
        reads: Reads::default(),
    };
    let mut store = BudgetedStore::new(
        &mut meter,
        RecordReadLimits {
            records: 3,
            bytes: row.len() * 2 - 1,
        },
    );
    assert_eq!(
        store
            .read_record(&stamp, &key, limits().checkpoint.bytes)
            .unwrap(),
        row
    );
    assert_eq!(
        store
            .read_record(&stamp, &key, limits().checkpoint.bytes)
            .unwrap_err()
            .code,
        "record_read_byte_limit"
    );
    assert_eq!(meter.reads.caps, vec![row.len() * 2 - 1, row.len() - 1]);
    assert_eq!(meter.reads.bytes, row.len());

    let mut store = BudgetedStore::new(&mut fixture.store, record_read_limits());
    assert_eq!(
        store.read_record(&stamp, &key, 1).unwrap_err().code,
        "record_byte_limit"
    );
    assert_eq!(
        store
            .read_record(&stamp, &RecordKey::Run("missing".into()), 1000)
            .unwrap_err()
            .code,
        "record_missing"
    );
    let mut stale = stamp;
    stale.revision += 1;
    assert_eq!(
        store.read_record(&stale, &key, 1000).unwrap_err().code,
        "record_stamp"
    );
}

#[test]
fn zero_allowance_refuses_before_storage_and_budget_updates_require_a_live_host() {
    let mut fixture = Fixture::new("adopt", false);
    for budget in [
        RecordReadLimits {
            records: 0,
            bytes: 1000,
        },
        RecordReadLimits {
            records: 1,
            bytes: 0,
        },
    ] {
        let (result, reads) = fixture.run("adopt", budget);
        assert!(result.is_err());
        assert_eq!(reads.count, 0);
    }
    fixture
        .host
        .set_record_read_limits(record_read_limits())
        .unwrap();
    fixture.store.fail_after_commit = true;
    assert!(
        fixture
            .host
            .adopt("a", "first", fixture.attempt, &mut fixture.store)
            .is_err()
    );
    assert_eq!(
        fixture
            .host
            .set_record_read_limits(record_read_limits())
            .unwrap_err()
            .code,
        "reload_required"
    );
}
