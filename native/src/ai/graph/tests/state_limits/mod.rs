use super::{durable_store::SqlStore, *};

mod admission;
mod preservation;

fn graph() -> Arc<Executable> {
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    Arc::new(registry().compile(d).unwrap())
}
fn limits(state: StateLimits) -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        state,
        checkpoint: CheckpointLimits {
            bytes: 1_000_000,
            events: 1000,
        },
        settlement_event_bytes: 4096,
        history: history_limits(),
    }
}
fn event(graph: &Executable, run: &str, scope: &str) -> Event {
    Event::Begin {
        run: run.into(),
        artifact: graph.identity().into(),
        inputs: values(40),
        scope: scope.into(),
        policy: BTreeMap::new(),
    }
}
fn start(
    host: &mut DurableEngine,
    store: &mut SqlStore,
    graph: &Executable,
    run: &str,
    scope: &str,
) -> Result<Vec<Work>> {
    store.authorize(run, scope);
    host.apply(event(graph, run, scope), store)
}
fn checkpoint(store: &SqlStore) -> Checkpoint {
    Checkpoint::decode(&store.bytes(), limits(state_limits()).checkpoint).unwrap()
}
fn attempt(host: &DurableEngine, run: &str) -> AttemptId {
    host.inspect(run).unwrap().attempts["first"]
        .last()
        .unwrap()
        .id
}
fn snapshot(host: &DurableEngine, run: &str) -> serde_json::Value {
    serde_json::to_value(
        host.inspection_snapshot(
            run,
            ExportLimits {
                bytes: 1_000_000,
                attempts: 1000,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
