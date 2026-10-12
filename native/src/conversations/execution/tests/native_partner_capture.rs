use super::*;
use crate::ai::graph::{Activation, Values};
use crate::language::source_graph::SourceText;
use crate::learning::coaching::feedback_graph;

#[test]
fn finalized_turn_projects_exact_sources_prompts_routes_and_activation_into_native_inputs() {
    let (_dir, mut store, conversation) = setup();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let (raw, install): (String, String) = store
        .connection
        .query_row(
            "SELECT t.context,l.id FROM turns t CROSS JOIN learner l WHERE t.id=?1",
            [&turn],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let mut captured: serde_json::Value = serde_json::from_str(&raw).unwrap();
    captured["fastModel"] = serde_json::json!("captured-fast");
    captured["target"]["model"] = serde_json::json!("captured-standard");
    let source = store
        .connection
        .query_row(
            "SELECT id,text FROM messages WHERE turn_id=?1 AND role='user'",
            [&turn],
            |r| {
                Ok(SourceText {
                    id: r.get(0)?,
                    text: r.get(1)?,
                })
            },
        )
        .unwrap();
    let values = partner_graph::captured_turn::inputs(
        context::Kind::Reply,
        &captured,
        Some(source.clone()),
        "reserved-partner-source".into(),
        install,
    )
    .unwrap();
    assert_eq!(values["reply_source_id"], "reserved-partner-source");
    for prefix in ["", "reading_", "brief_", "attribution_"] {
        assert_eq!(values[&format!("{prefix}target")]["model"], "captured-fast");
    }
    for prefix in ["gloss_", "assistance_", "feedback_"] {
        assert_eq!(
            values[&format!("{prefix}target")]["model"],
            "captured-standard"
        );
    }
    assert_eq!(
        values["assessment_target"]["model"],
        crate::learning::coaching::assessment_adapter::MODEL
    );
    let mut feedback: Values = values
        .iter()
        .filter_map(|(name, value)| {
            name.strip_prefix("feedback_")
                .map(|name| (name.to_owned(), value.clone()))
        })
        .collect();
    feedback.insert("source".into(), serde_json::json!(source));
    let request = feedback_graph::decode(&feedback).unwrap();
    let native = request
        .text_request("attempt".into(), "operation".into())
        .unwrap();
    let previous = crate::learning::coaching::prompt(
        &store.connection,
        &turn,
        crate::learning::coaching::FEEDBACK,
        &captured,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(native.messages).unwrap(),
        serde_json::to_value(previous).unwrap()
    );
    let automatic = partner_graph::captured_turn::policy(context::Kind::Reply, &captured).unwrap();
    assert!(
        automatic
            .iter()
            .filter(|(node, _)| node.as_str() != "speech/lookup")
            .map(|(_, mode)| mode)
            .all(|mode| *mode == Activation::Automatic)
    );
    assert_eq!(automatic["speech/lookup"], Activation::OnDemand);
    assert!(!values.contains_key("speech"));
    captured["executionPreferences"] = serde_json::json!({"assessment":"on_demand","replyBrief":"on_demand","reading":"on_demand"});
    let requested = partner_graph::captured_turn::policy(context::Kind::Reply, &captured).unwrap();
    assert!(requested.values().all(|mode| *mode == Activation::OnDemand));
    assert!(!requested.contains_key("attribution"));
    let opening = partner_graph::captured_turn::policy(context::Kind::Opening, &captured).unwrap();
    assert!(!opening.contains_key("assessment"));
    assert!(!opening.contains_key("learner_gloss"));
}
