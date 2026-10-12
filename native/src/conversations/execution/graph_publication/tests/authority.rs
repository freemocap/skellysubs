use super::*;
use crate::conversations::execution::{graph_authority, prose};

#[tokio::test]
async fn revoked_dispatch_never_claims_producer_or_creates_wire_identity() {
    for change in [
        "UPDATE ai_config SET hosted_credential_id='replacement'",
        "UPDATE ai_config SET hosted_credential_id=NULL",
        "UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.bearerAuth',json('false'))",
        "UPDATE conversations SET archived=1",
        "UPDATE contacts SET archived=1",
        "UPDATE turns SET state='invalidated'",
        "UPDATE turns SET paused=1",
        "UPDATE turns SET refusal_hold='{}'",
        "UPDATE ai_config SET paused=1",
        "DELETE FROM messages WHERE id='user'",
    ] {
        let (mut db, partition, mut host, attempt) = prepared("Reply").await;
        let before = host.stamp().clone();
        db.execute(change, []).unwrap();
        let mut error = None;
        assert!(
            host.claim(
                "run",
                "reply",
                attempt,
                &mut store(&mut db, &partition, &mut error, false, 100_000)
            )
            .is_err(),
            "{change}"
        );
        assert!(
            error.is_some(),
            "full domain failure must survive the graph fault"
        );
        assert_eq!(host.stamp(), &before);
        assert_eq!(
            host.inspect("run").unwrap().nodes["reply"],
            Disposition::Prepared
        );
        assert_eq!(count(&db, "graph_transport_identities"), 0);
        assert_eq!(count(&db, "conversation_graph_publications"), 0);
    }
}

#[tokio::test]
async fn available_result_is_checked_again_at_adoption_without_treating_pause_as_revocation() {
    for change in [
        "none",
        "paused",
        "credential",
        "source",
        "scope",
        "artifact",
        "engine",
    ] {
        let (mut db, partition, mut host, attempt) = available("Reply").await;
        match change {
            "paused" => {
                db.execute_batch("UPDATE ai_config SET paused=1; UPDATE turns SET paused=1;")
                    .unwrap();
            }
            "credential" => {
                db.execute(
                    "UPDATE ai_config SET hosted_credential_id='replacement'",
                    [],
                )
                .unwrap();
            }
            "source" => {
                db.execute("DELETE FROM messages WHERE id='user'", [])
                    .unwrap();
            }
            _ => (),
        }
        // The fixture's target/source IDs are the same inputs admitted above;
        // production must read them from the original native producer record.
        let captured = prose::decode(&inputs("Reply")).unwrap();
        let mut original_error = None;
        let callback = |db: &Connection, request: &CommitRequest<'_>| {
            publish_reply(db, request, |db, authority| {
                let altered = Authority {
                    run: authority.run,
                    node: authority.node,
                    artifact: if change == "artifact" {
                        "other"
                    } else {
                        authority.artifact
                    },
                    scope: if change == "scope" {
                        "other"
                    } else {
                        authority.scope
                    },
                };
                graph_authority::check_owner(
                    db,
                    if change == "engine" {
                        "other"
                    } else {
                        &request.next.stamp().engine
                    },
                    &altered,
                    &captured,
                    graph_authority::Phase::Adoption,
                )
            })
            .map_err(|error| {
                original_error = Some(error);
                Fault {
                    code: "fixture_authority_rejected".into(),
                    path: "authority".into(),
                }
            })
        };
        let mut adapter =
            TransactionStore::new(db.transaction().unwrap(), partition, 100_000, callback);
        let result = host.adopt("run", "reply", attempt, &mut adapter);
        drop(adapter);
        let permitted = matches!(change, "none" | "paused");
        assert_eq!(result.is_ok(), permitted, "{change}");
        assert_eq!(original_error.is_none(), permitted);
        assert_eq!(
            count(&db, "conversation_graph_publications"),
            i64::from(permitted)
        );
        assert_eq!(count(&db, "effort_awards"), i64::from(permitted));
    }
}

#[tokio::test]
async fn owner_checks_require_transaction_and_preserve_captured_model_selection() {
    let (mut db, _, host, _) = prepared("Reply").await;
    let captured = prose::decode(&inputs("Reply")).unwrap();
    let graph = executable();
    let authority = Authority {
        run: "run",
        node: Some("reply"),
        artifact: graph.identity(),
        scope: "captured-authority",
    };
    assert!(
        graph_authority::check_owner(
            &db,
            &host.stamp().engine,
            &authority,
            &captured,
            graph_authority::Phase::Dispatch
        )
        .is_err()
    );
    let tx = db.transaction().unwrap();
    // Destination authority compares route/URL/credential, as in the existing
    // scheduler. A selected model is captured work, not an instruction to reroute.
    tx.execute(
        "UPDATE ai_config SET standard_model='new-default',revision=revision+1",
        [],
    )
    .unwrap();
    graph_authority::check_owner(
        &tx,
        &host.stamp().engine,
        &authority,
        &captured,
        graph_authority::Phase::Dispatch,
    )
    .unwrap();
    assert_eq!(captured.target.model, "fixture");
    tx.rollback().unwrap();
}
