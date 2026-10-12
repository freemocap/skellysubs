use super::*;
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

mod attempt_records;
mod compaction;
mod dispatch;
mod durability;
mod durable_store;
mod enlistment;
mod eviction;
mod evidence_persistence;
mod execution;
mod execution_queries;
mod history;
mod interleavings;
mod invocation;
mod projection;
mod read_budget;
mod record_access;
mod record_evidence;
mod records;
mod refinement;
mod retained_definitions;
mod scheduling_reads;
mod settlement_capacity;
mod sql_records;
mod state_limits;
mod stepping;
mod stored_records;
mod targeted_reads;
mod transition_reads;
mod validation;
mod value_types;

fn contract(name: &str) -> Contract {
    Contract::new(name, 1)
}
fn evidence_limits() -> EvidenceLimits {
    EvidenceLimits {
        observations: 16,
        bytes: 32_768,
    }
}
fn history_limits() -> HistoryLimits {
    HistoryLimits {
        bytes: 10_000_000,
        events: 10_000,
        segments: 100,
    }
}
fn record_read_limits() -> RecordReadLimits {
    RecordReadLimits {
        records: 100_000,
        bytes: 100_000_000,
    }
}
fn state_limits() -> StateLimits {
    StateLimits {
        runs: 1000,
        attempts: 10_000,
        executions: 10_000,
    }
}
fn port(optional: bool) -> Port {
    Port {
        contract: contract("integer"),
        optional,
    }
}
fn ports() -> Ports {
    BTreeMap::from([("value".into(), port(false))])
}
fn values(n: i64) -> Values {
    BTreeMap::from([("value".into(), json!(n))])
}
fn source(node: &str) -> Source {
    Source::Output {
        node: node.into(),
        port: "value".into(),
    }
}
fn node(input: Source) -> Node {
    Node {
        operation: contract("increment"),
        inputs: BTreeMap::from([("value".into(), input)]),
        after: vec![],
        guard: None,
        activation: Activation::Automatic,
    }
}
fn definition() -> Definition {
    Definition {
        contract: contract("chain"),
        inputs: ports(),
        outputs: ports(),
        nodes: BTreeMap::from([
            ("first".into(), node(Source::Input("value".into()))),
            ("second".into(), node(source("first"))),
        ]),
        results: BTreeMap::from([("value".into(), source("second"))]),
        compositions: BTreeMap::new(),
    }
}
fn registry() -> Registry {
    let mut registry = Registry::default();
    registry
        .define_type(contract("integer"), Shape::Integer)
        .unwrap();
    registry
        .define_type(contract("boolean"), Shape::Boolean)
        .unwrap();
    registry
        .register(
            Operation {
                contract: contract("increment"),
                implementation: "synthetic/increment/v1".into(),
                inputs: ports(),
                outputs: ports(),
                resource: Resource::Provider,
                reuse: Reuse::Exact,
            },
            Arc::new(|_, input| {
                Box::pin(async move {
                    Ok(values(
                        input["value"]
                            .as_i64()
                            .unwrap()
                            .checked_add(1)
                            .ok_or_else(|| unclassified("overflow", "value"))?,
                    ))
                })
            }),
        )
        .unwrap();
    registry
}
fn begin(engine: &mut Engine, graph: &Executable, run: &str, n: i64) {
    engine
        .apply(Event::Begin {
            run: run.into(),
            artifact: graph.identity().into(),
            inputs: values(n),
            scope: "test-authority-v1".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
}
fn capacity(n: usize) -> Event {
    Event::Advance(Capacity {
        local: n,
        provider: n,
    })
}
fn adopt(engine: &mut Engine, run: &str, node: &str) {
    let attempt = engine.inspect(run).unwrap().attempts[node]
        .last()
        .unwrap()
        .id;
    engine
        .apply(Event::Adopt {
            run: run.into(),
            node: node.into(),
            attempt,
        })
        .unwrap();
}
fn demand(run: &str, node: &str) -> Event {
    Event::Demand {
        run: run.into(),
        node: node.into(),
    }
}

fn advance(engine: &mut Engine, capacity: Event) -> Vec<Work> {
    let work = engine.apply(capacity).unwrap();
    for item in &work {
        engine
            .apply(Event::Dispatch {
                execution: item.execution,
            })
            .unwrap();
    }
    work
}

fn unclassified(code: &str, path: &str) -> Fault {
    Fault {
        code: code.into(),
        path: path.into(),
    }
}
