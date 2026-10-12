//! Verify published results through the same snapshot consumed by the UI.
use super::*;

pub(super) fn product_state(db: &Connection, turn: &str) -> String {
    db.query_row("SELECT context FROM turns WHERE id=?1", [turn], |r| {
        r.get(0)
    })
    .unwrap()
}

pub(super) fn verify_product(
    store: &mut Store,
    conversation: &str,
    turn: &str,
    graph: Arc<Executable>,
) {
    store.graph_runtime.register_artifact(graph).unwrap();
    store.graph_runtime.recover(&mut store.connection).unwrap();
    let product = store.conversation_snapshot(conversation, None).unwrap();
    assert_eq!(product.messages.len(), 2);
    for message in &product.messages {
        assert_eq!(message.translation.as_deref(), Some("Hello."));
        assert_eq!(message.translation_state.as_deref(), Some("succeeded"));
        assert!(message.word_gloss.is_some());
        assert_eq!(message.gloss_state.as_deref(), Some("succeeded"));
        assert!(
            message
                .gloss_operation_id
                .as_ref()
                .unwrap()
                .starts_with("graph:")
        );
        if message.role == "user" {
            assert!(message.conversation_feedback.is_some());
            assert_eq!(message.feedback_state.as_deref(), Some("succeeded"));
            assert!(message.coach_decision.is_some());
        } else {
            assert_eq!(message.text, "Hola.");
            assert!(message.reply_brief.is_some());
            assert!(message.reply_assistance.is_some());
            assert!(message.reply_explanations.is_some());
            assert_eq!(message.brief_state.as_deref(), Some("succeeded"));
            assert_eq!(message.suggestions_state.as_deref(), Some("succeeded"));
            assert_eq!(message.explanations_state.as_deref(), Some("succeeded"));
        }
    }
    let evidence = crate::learning::learner::progression::snapshot(store, "spanish").unwrap();
    let record = evidence["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["chat_id"] == conversation)
        .expect("published assessment appears in learner evidence");
    assert_eq!(record["status"], "complete");
    assert!(
        record["attempt_id"]
            .as_str()
            .unwrap()
            .starts_with("graph-assessment:")
    );
    assert!(evidence["profile"]["xp"].as_u64().unwrap() > 0);
    let message = product
        .messages
        .iter()
        .find(|message| message.role == "user")
        .unwrap();
    let history = store.message_history(conversation, &message.id).unwrap();
    assert!(
        history.versions[0]
            .assessments
            .iter()
            .any(|assessment| assessment.kind == "skill_assessment"
                && assessment.state == "succeeded"
                && assessment.attempt_id.is_some())
    );
    let context = crate::conversations::assessments::context(&store.connection, turn).unwrap();
    assert_eq!(context["coachValidationOmissions"], 1);
    assert_eq!(context["coachDecision"]["keptGoing"], true);
    // A receipt for a different attempt cannot disclose this feedback.
    let tx = store.connection.transaction().unwrap();
    assert!(
        crate::conversations::assessments::disclose(
            &tx,
            turn,
            "foreign-receipt",
            &json!({"keptGoing":false})
        )
        .is_err()
    );
    tx.rollback().unwrap();
    assert_eq!(
        crate::conversations::assessments::context(&store.connection, turn).unwrap()["coachDecision"],
        context["coachDecision"]
    );
}
