use super::durable_store::SqlStore;
use super::*;

mod pages;
mod timeline;
mod validation;

fn read_limits() -> HistoricalLimits {
    HistoricalLimits {
        history: history_limits(),
        state: state_limits(),
    }
}

fn bounds() -> CheckpointLimits {
    CheckpointLimits {
        bytes: 1_000_000,
        events: 1000,
    }
}
fn export(attempts: usize) -> ExportLimits {
    ExportLimits {
        bytes: 1_000_000,
        attempts,
    }
}
fn graph() -> Arc<Executable> {
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    let mut disabled = node(Source::Constant {
        contract: contract("integer"),
        value: json!(123456789),
    });
    disabled.activation = Activation::Disabled;
    d.nodes.insert("never".into(), disabled);
    Arc::new(registry().compile(d).unwrap())
}
fn trace(graph: &Arc<Executable>) -> Engine {
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, graph, "a", 40);
    begin(&mut engine, graph, "b", 40);
    let work = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Err(unclassified("secret-code", "secret-content")),
        })
        .unwrap();
    engine
        .apply(Event::Retry {
            run: "a".into(),
            node: "first".into(),
        })
        .unwrap();
    let work = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Ok(values(41)),
        })
        .unwrap();
    adopt(&mut engine, "a", "first");
    engine.apply(demand("a", "second")).unwrap();
    let work = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Ok(values(42)),
        })
        .unwrap();
    adopt(&mut engine, "a", "second");
    engine
}
fn save(engine: &Engine, store: &mut SqlStore) -> Checkpoint {
    let checkpoint =
        Checkpoint::capture(engine, &uuid::Uuid::new_v4().to_string(), bounds()).unwrap();
    store
        .commit(CommitRequest {
            expected: None,
            next: &checkpoint,
            intent: CommitIntent::Record,
            records: RecordChanges::between(None, &engine.state),
        })
        .unwrap();
    checkpoint
}
fn compact(engine: &mut Engine, old: &Checkpoint, store: &mut SqlStore) -> Checkpoint {
    engine.rebase();
    let next = old.compacted(engine, bounds()).unwrap();
    store
        .commit(CommitRequest {
            expected: Some(old.stamp()),
            next: &next,
            intent: CommitIntent::Compact { archive: old },
            records: RecordChanges::default(),
        })
        .unwrap();
    next
}
fn header(mut value: serde_json::Value) -> serde_json::Value {
    value.as_object_mut().unwrap().remove("attempts");
    value
}

/// The inspection API accepts this capability; publication is unavailable.
struct ReadOnly<'a>(&'a mut SqlStore);
impl HistoryStore for ReadOnly<'_> {
    fn read_archive(&mut self, stamp: &Stamp, max_bytes: usize) -> Result<Vec<u8>> {
        self.0.read_archive(stamp, max_bytes)
    }
}
