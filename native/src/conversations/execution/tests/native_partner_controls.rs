use super::*;
use crate::ai::graph::*;
use std::sync::Arc;

#[tokio::test]
async fn native_turn_retry_targets_reply_without_retrying_independent_assessment() {
    let (_dir, mut store, conversation) = setup();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    store.graph_runtime.partner.bind(
        Arc::new(|_| {
            Box::pin(async {
                Err(Fault {
                    code: "fixture_failure".into(),
                    path: "provider".into(),
                })
            })
        }),
        Arc::new(|_, _| Box::pin(async { Ok(None) })),
    );
    for _ in 0..25 {
        if let Some(claim) = store
            .graph_runtime
            .next(
                &mut store.connection,
                true,
                &store.config,
                &store.session_id,
            )
            .unwrap()
        {
            let report = claim
                .invocation
                .execute(EvidenceLimits {
                    observations: 16,
                    bytes: 65536,
                })
                .await;
            store
                .graph_runtime
                .finish(&mut store.connection, &conversation, &turn, report)
                .unwrap();
        }
    }
    let before = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(before.nodes["reply"], Disposition::Failed);
    assert_eq!(before.nodes["assessment"], Disposition::Failed);
    let receipt = apply(
        &mut store,
        Action::ControlTurn {
            turn_id: turn.clone(),
            control: TurnControl::Retry,
        },
    );
    assert_eq!(receipt.entity_id, turn);
    let after = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(after.nodes["reply"], Disposition::Ready);
    assert_eq!(after.nodes["assessment"], Disposition::Failed);
    assert_eq!(
        after.attempts["assessment"].len(),
        before.attempts["assessment"].len()
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
