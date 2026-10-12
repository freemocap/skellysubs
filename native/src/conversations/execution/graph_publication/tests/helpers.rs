use super::*;
use crate::{
    ai::{connections::access::ResolvedTarget, transport::provider::Completion},
    language::{source_graph::SourceText, translation_graph},
};
use std::sync::Arc;

fn adapter<'a>(
    db: &'a mut Connection,
    partition: &Partition,
    work: Option<&'a Work>,
    reject: bool,
) -> TransactionStore<'a, impl FnMut(&Connection, &CommitRequest<'_>) -> graph::Result<()> + 'a> {
    TransactionStore::new(
        db.transaction().unwrap(),
        partition.clone(),
        100_000,
        move |db: &Connection, request: &CommitRequest<'_>| {
            let result = (|| -> crate::model::Result<()> {
                match &request.intent {
                    CommitIntent::Begin { authority, .. } => {
                        db.execute("INSERT INTO turn_execution_owners VALUES('turn','graph','persona_reply',?1,?2,?3)",
                            params![request.next.stamp().engine,authority.run,authority.artifact])?;
                    }
                    CommitIntent::Adopt { .. } => {
                        assert!(publish_helper(db, request, work.unwrap(), |_, _| {
                            if reject {
                                Err(AppError::new(ErrorCode::Conflict, "revoked"))
                            } else {
                                Ok(())
                            }
                        })?);
                    }
                    _ => (),
                }
                Ok(())
            })();
            result.map_err(|_| Fault {
                code: "helper_publication_rejected".into(),
                path: "owner".into(),
            })
        },
    )
}

#[tokio::test]
async fn translation_adoption_projects_actual_source_role_and_rejects_stale_or_foreign_sources() {
    for role in ["user", "assistant"] {
        let mut db = db();
        if role == "assistant" {
            db.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('reply','conversation','turn',2,'assistant','Réponse 日本語')",[]).unwrap();
        }
        let (source_id, text) = if role == "user" {
            ("user", "Question")
        } else {
            ("reply", "Réponse 日本語")
        };
        let target = ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Hosted,
            revision: 1,
            url: format!("{}/v1/operations", crate::ai::hosted::ORIGIN),
            model: "fixture".into(),
            credential: Some("reference".into()),
        };
        let graph = Arc::new(translation_graph::compile(Arc::new(|_,request| Box::pin(async move {
            Ok(Completion {text:serde_json::json!({"source":request.source.text,"translation":"Translated café"}).to_string(),
                finish_reason:"stop".into(),actual_model:"fixture".into(),provider_id:"provider".into(),input_tokens:Some(1),output_tokens:Some(2),diagnostics:None})
        }))).unwrap());
        let partition = Partition {
            owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
            catalog: catalog_id([graph.identity()]).unwrap(),
        };
        let mut engine = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                scope: "scope".into(),
                inputs: translation_graph::capture(
                    SourceText {
                        id: source_id.into(),
                        text: text.into(),
                    },
                    translation_graph::Languages {
                        source: "french".into(),
                        destination: "english".into(),
                        destination_writing: vec![],
                    },
                    &target,
                    "fixture",
                )
                .unwrap(),
                policy: std::collections::BTreeMap::from([(
                    "translate".into(),
                    Activation::Automatic,
                )]),
            },
            &mut adapter(&mut db, &partition, None, false),
        )
        .unwrap();
        engine
            .apply(
                Event::Advance(Capacity {
                    local: 0,
                    provider: 1,
                }),
                &mut adapter(&mut db, &partition, None, false),
            )
            .unwrap();
        let attempt = engine.inspect("run").unwrap().attempts["translate"]
            .last()
            .unwrap()
            .id;
        let report = engine
            .claim(
                "run",
                "translate",
                attempt,
                &mut adapter(&mut db, &partition, None, false),
            )
            .unwrap()
            .execute(EvidenceLimits {
                observations: 10,
                bytes: 8192,
            })
            .await;
        engine
            .apply(
                Event::SettleObserved(report),
                &mut adapter(&mut db, &partition, None, false),
            )
            .unwrap();
        let execution = engine.inspect("run").unwrap().attempts["translate"]
            .last()
            .unwrap()
            .execution;
        let work = engine
            .read_execution_work(
                execution,
                &mut ReadStore::new(&mut db, partition.clone()).unwrap(),
            )
            .unwrap();
        let before = engine.stamp().clone();
        let original: String = db
            .query_row("SELECT context FROM turns", [], |r| r.get(0))
            .unwrap();
        assert!(
            engine
                .adopt(
                    "run",
                    "translate",
                    attempt,
                    &mut adapter(&mut db, &partition, Some(&work), true)
                )
                .is_err()
        );
        let mut foreign = work.clone();
        foreign.inputs.get_mut("source").unwrap()["id"] = serde_json::json!("another-source");
        assert!(
            engine
                .adopt(
                    "run",
                    "translate",
                    attempt,
                    &mut adapter(&mut db, &partition, Some(&foreign), false)
                )
                .is_err()
        );
        db.execute("DELETE FROM messages WHERE id=?1", [source_id])
            .unwrap();
        assert!(
            engine
                .adopt(
                    "run",
                    "translate",
                    attempt,
                    &mut adapter(&mut db, &partition, Some(&work), false)
                )
                .is_err()
        );
        db.execute(
            "INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES(?1,'conversation','turn',?2,?3,?4)",
            params![source_id, if role == "user" {1} else {2}, role, text],
        )
        .unwrap();
        db.execute("UPDATE conversations SET archived=1", [])
            .unwrap();
        assert!(
            engine
                .adopt(
                    "run",
                    "translate",
                    attempt,
                    &mut adapter(&mut db, &partition, Some(&work), false)
                )
                .is_err()
        );
        db.execute("UPDATE conversations SET archived=0", [])
            .unwrap();
        assert_eq!(engine.stamp(), &before);
        assert_eq!(
            db.query_row("SELECT context FROM turns", [], |r| r.get::<_, String>(0))
                .unwrap(),
            original
        );
        engine
            .adopt(
                "run",
                "translate",
                attempt,
                &mut adapter(&mut db, &partition, Some(&work), false),
            )
            .unwrap();
        let saved: String = db
            .query_row("SELECT context FROM turns", [], |r| r.get(0))
            .unwrap();
        let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            saved[if role == "user" {
                "userTranslation"
            } else {
                "translation"
            }],
            "Translated café"
        );
        assert!(
            saved
                .get(if role == "user" {
                    "translation"
                } else {
                    "userTranslation"
                })
                .is_none()
        );
        assert_eq!(count(&db, "operations"), 0);
        assert_eq!(count(&db, "attempts"), 0);
    }
}
