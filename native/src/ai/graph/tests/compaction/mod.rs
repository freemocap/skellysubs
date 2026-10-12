use super::{durable_store::SqlStore, *};

mod continuation;
mod integrity;
mod node_outputs;
mod stepping;
mod transactions;

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
fn start(host: &mut DurableEngine, store: &mut SqlStore, graph: &Executable) {
    store.authorize("run", "scope");
    host.apply(
        Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: values(40),
            scope: "scope".into(),
            policy: BTreeMap::new(),
        },
        store,
    )
    .unwrap();
}
fn checkpoint(store: &SqlStore) -> Checkpoint {
    Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap()
}
fn archives(store: &SqlStore) -> usize {
    store
        .conn
        .query_row("SELECT count(*) FROM archive", [], |r| r.get::<_, u32>(0))
        .unwrap() as usize
}
fn id(host: &DurableEngine, node: &str) -> AttemptId {
    host.inspect("run").unwrap().attempts[node]
        .last()
        .unwrap()
        .id
}
fn view(host: &DurableEngine) -> serde_json::Value {
    serde_json::to_value(
        host.inspection_snapshot(
            "run",
            ExportLimits {
                bytes: 1_000_000,
                attempts: 1000,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
