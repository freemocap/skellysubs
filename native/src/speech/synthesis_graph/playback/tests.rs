use super::super::tests::{adapter, values};
use super::*;
use crate::ai::graph_store::{Partition, catalog_id};
use rusqlite::Connection;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn one_artifact_covers_cache_hit_miss_and_explicit_regeneration() {
    for (hit, regenerate) in [(true, false), (false, false), (true, true)] {
        let calls = Arc::new(AtomicUsize::new(0));
        let lookups = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let reads = lookups.clone();
        let graph = Arc::new(
            compile(
                Arc::new(move |invocation, _| {
                    counted.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move {
                        Ok(Receipt {
                            id: "generated".into(),
                            engine: invocation.identity().engine.clone().unwrap(),
                            execution: serde_json::to_value(invocation.identity().execution)
                                .unwrap()
                                .to_string(),
                            digest: "a".repeat(64),
                            bytes: 100,
                        })
                    })
                }),
                Arc::new(move |_, _| {
                    reads.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async move {
                        Ok(hit.then(|| Receipt {
                            id: "cached".into(),
                            engine: "original-producer".into(),
                            execution: "9".into(),
                            digest: "b".repeat(64),
                            bytes: 100,
                        }))
                    })
                }),
            )
            .unwrap(),
        );
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE conversations(id TEXT PRIMARY KEY); INSERT INTO conversations VALUES('conversation');").unwrap();
        db.execute_batch(include_str!("../../../storage/schemas/graph_runtime.sql"))
            .unwrap();
        let partition = Partition {
            owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
            catalog: catalog_id([graph.identity()]).unwrap(),
        };
        let mut inputs = values();
        inputs.insert("regenerate".into(), serde_json::json!(regenerate));
        let mut engine = DurableEngine::create_with_run(
            [graph.clone()],
            crate::conversations::execution::graph_runtime::limits(),
            Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                scope: "owner".into(),
                inputs,
                policy: BTreeMap::new(),
            },
            &mut adapter(&mut db, &partition),
        )
        .unwrap();
        let capacity = Capacity {
            local: 4,
            provider: 4,
        };
        assert!(
            engine
                .apply(Event::Advance(capacity), &mut adapter(&mut db, &partition))
                .unwrap()
                .is_empty()
        );
        engine
            .apply(
                Event::Demand {
                    run: "run".into(),
                    node: "lookup".into(),
                },
                &mut adapter(&mut db, &partition),
            )
            .unwrap();
        for _ in 0..4 {
            let work = engine
                .apply(Event::Advance(capacity), &mut adapter(&mut db, &partition))
                .unwrap();
            if work.is_empty() {
                break;
            }
            for work in work {
                let (node, attempt) = engine
                    .inspect("run")
                    .unwrap()
                    .attempts
                    .iter()
                    .find_map(|(node, attempts)| {
                        attempts
                            .last()
                            .filter(|a| a.execution == work.execution)
                            .map(|a| (node.clone(), a.id))
                    })
                    .unwrap();
                let report = engine
                    .claim("run", &node, attempt, &mut adapter(&mut db, &partition))
                    .unwrap()
                    .execute(EvidenceLimits {
                        observations: 4,
                        bytes: 4096,
                    })
                    .await;
                engine
                    .settle_report(&report, &mut adapter(&mut db, &partition))
                    .unwrap();
                engine
                    .adopt("run", &node, attempt, &mut adapter(&mut db, &partition))
                    .unwrap();
            }
        }
        let view = engine.inspect("run").unwrap();
        assert_eq!(view.nodes.len(), 3);
        assert_eq!(view.nodes["audio"], Disposition::Adopted);
        assert_eq!(
            view.nodes["synthesize"],
            if hit && !regenerate {
                Disposition::Skipped
            } else {
                Disposition::Adopted
            }
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            usize::from(!hit || regenerate)
        );
        assert_eq!(lookups.load(Ordering::SeqCst), usize::from(!regenerate));
    }
}
