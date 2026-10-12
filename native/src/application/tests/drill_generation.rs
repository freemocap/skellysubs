use super::*;
use crate::application::test_server::structured_server;
use crate::drill::generation::DrillGenerationInput;
use serde_json::json;
fn input() -> DrillGenerationInput {
    DrillGenerationInput {
        skill_target: None,
        language: "spanish".into(),
        variety: None,
        explanation: "english".into(),
        explanation_variety: None,
        topic: Some("PRIVATE-TOPIC".into()),
        count: 4,
        difficulty: model::Difficulty::Beginner,
        length: crate::drill::generation::DrillLength::ShortPhrase,
    }
}

#[tokio::test]
async fn skill_target_reaches_generation_and_survives_acceptance_without_xp() {
    use crate::drill::{previews::DrillSource, skill_focus::DrillSkillTarget};
    let (url, worker) = structured_server(|content| {
        let data: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(data["skillFocus"]["skill"]["id"], "time_events");
        assert!(
            !data["skillFocus"]["skill"]["language_guidance"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert_eq!(data["length"], "sentence");
        json!({"candidates":[candidate("Ayer preparé la cena.")]}).to_string()
    });
    let (_dir, app) = app(&url);
    let mut request = input();
    request.variety = Some("spanish-spain".into());
    request.length = crate::drill::generation::DrillLength::Sentence;
    request.skill_target = Some(DrillSkillTarget::Skill {
        skill_id: "time_events".into(),
    });
    let id = reserve(&app, request).unwrap();
    let captured = previews::input(&app.lock().unwrap().connection, &id).unwrap();
    assert_eq!(captured.skill_focus.unwrap().skill.id, "time_events");
    let preview = run(&app, &id).await.unwrap();
    crate::ai::inspection::verify_workspace_reads(&app.lock().unwrap());
    {
        let store = app.lock().unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM workspace_graph_runs WHERE run_id=?1 AND kind='drill'",
                    [&id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            crate::ai::workspace_graph::receipt(&store.connection, &id)
                .unwrap()
                .unwrap()["state"],
            "succeeded"
        );
    }

    worker.join().unwrap();
    let items = app
        .lock()
        .unwrap()
        .accept_drill_items(&id, &[preview.candidates[0].candidate_id.clone()])
        .unwrap();
    let DrillSource::Generated {
        skill_focus: Some(focus),
        ..
    } = &items[0].source
    else {
        panic!("Missing captured skill target")
    };
    assert_eq!(focus.skill.id, "time_events");
    let store = app.lock().unwrap();
    let effort = crate::learning::effort::read(&store.connection, "spanish").unwrap();
    assert_eq!(effort.explorations, 1);
    assert!(effort.recent[0].conversation_id.is_none());
    store.drill_preview(&id).unwrap();
    assert_eq!(
        crate::learning::effort::read(&store.connection, "spanish")
            .unwrap()
            .explorations,
        1
    );
    let saved = store.drill_items("spanish").unwrap();
    assert_eq!(
        serde_json::to_value(&saved[0].source).unwrap()["skillFocus"]["skill"]["id"],
        "time_events"
    );
    assert_eq!(
        crate::learning::learner::progression::snapshot(&store, "spanish").unwrap()["profile"]["xp"],
        0
    );
}

#[test]
fn coach_skill_selection_is_captured_and_invalid_targets_do_not_reserve_work() {
    use crate::{
        drill::skill_focus::DrillSkillTarget, learning::recommendations::RecommendationMode,
    };
    let (_dir, app) = app("http://127.0.0.1:9/v1");
    let mut request = input();
    request.variety = Some("spanish-spain".into());
    for mode in [RecommendationMode::Explore, RecommendationMode::CoachChoice] {
        request.skill_target = Some(DrillSkillTarget::Coach { mode });
        let id = reserve(&app, request.clone()).unwrap();
        let captured = previews::input(&app.lock().unwrap().connection, &id).unwrap();
        let focus = captured.skill_focus.unwrap();
        let selection = focus.recommendation.unwrap();
        assert_eq!(selection.requested, mode);
        assert_eq!(selection.selected, RecommendationMode::Explore);
        assert_eq!(selection.skill.experience, 0);
        assert_eq!(selection.skill.skill_id, focus.skill.id);
    }
    request.skill_target = Some(DrillSkillTarget::Coach {
        mode: RecommendationMode::ContinuePracticing,
    });
    assert!(
        reserve(&app, request.clone())
            .unwrap_err()
            .message
            .contains("No retry effort")
    );
    request.skill_target = Some(DrillSkillTarget::Skill {
        skill_id: "not_a_skill".into(),
    });
    assert!(reserve(&app, request).is_err());
    let count: i64 = app
        .lock()
        .unwrap()
        .connection
        .query_row("SELECT count(*) FROM drill_previews", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
}
fn app(base: &str) -> (tempfile::TempDir, Arc<Application>) {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::start(&dir.path().join("generation.sqlite3"), None);
    app.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))",[base]).unwrap();
    (dir, app)
}
fn candidate(text: &str) -> serde_json::Value {
    json!({"text":text,"translation":null,"reported":{"difficulty":"beginner","tags":["unverified"]}})
}
#[tokio::test]
async fn preview_acceptance_is_durable_partial_idempotent_and_never_repeats_inference() {
    let (url, worker) = structured_server(|_| {
        json!({"candidates":[candidate("Uno."),candidate("Dos."),candidate("Uno."),candidate("")]})
            .to_string()
    });
    let (dir, app) = app(&url);
    let id = reserve(&app, input()).unwrap();
    assert!(
        app.lock()
            .unwrap()
            .drill_items("spanish")
            .unwrap()
            .is_empty()
    );
    let preview = run(&app, &id).await.unwrap();
    {
        let store = app.lock().unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM workspace_graph_runs WHERE run_id=?1 AND kind='drill'",
                    [&id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            crate::ai::workspace_graph::receipt(&store.connection, &id)
                .unwrap()
                .unwrap()["state"],
            "succeeded"
        );
    }

    let payload = worker.join().unwrap();
    let prompt = payload["items"][0]["request"]["messages"][0]["content"]
        .as_str()
        .unwrap();
    let expected = {
        let store = app.lock().unwrap();
        crate::configuration::difficulty::instruction(
            store.config.conversation_prompt(),
            "Spanish",
            &model::Difficulty::Beginner,
        )
    };
    // Exact shared difficulty body (target display label may include region).
    assert!(prompt.contains(expected.split_once(". ").unwrap().1));
    assert_eq!(preview.candidates.len(), 2);
    assert_eq!(preview.shortfall.unwrap().produced, 2);
    assert_eq!(
        preview.requested.unwrap().difficulty,
        model::Difficulty::Beginner
    );
    assert_eq!(
        preview.candidates[0].reported.difficulty.as_deref(),
        Some("beginner")
    );
    assert!(preview.candidates[0].verified.scope_matches_request);
    let ids: Vec<_> = preview
        .candidates
        .iter()
        .map(|c| c.candidate_id.clone())
        .collect();
    assert!(
        app.lock()
            .unwrap()
            .accept_drill_items(&id, &[ids[0].clone(), "forged".into()])
            .is_err()
    );
    assert!(
        app.lock()
            .unwrap()
            .drill_items("spanish")
            .unwrap()
            .is_empty()
    );
    let first = app
        .lock()
        .unwrap()
        .accept_drill_items(&id, &ids[..1])
        .unwrap();
    let all = app.lock().unwrap().accept_drill_items(&id, &ids).unwrap();
    assert_eq!(first[0].id, all[0].id);
    assert_eq!(app.lock().unwrap().profile().unwrap().global.attempts, 1);
    assert!(
        app.lock()
            .unwrap()
            .accept_drill_items(&id, &[ids[1].clone(), "forged-text".into()])
            .is_err()
    );
    assert_eq!(app.lock().unwrap().drill_items("spanish").unwrap().len(), 2);
    let activity =
        generation_receipts::activity_for(&app.lock().unwrap().connection, "drill").unwrap();
    assert_eq!(activity.attempts.len(), 1);
    assert_eq!(activity.attempts[0].state, "succeeded");
    assert_eq!(activity.attempts[0].finish_reason.as_deref(), Some("stop"));
    assert_eq!(
        activity.attempts[0].actual_model.as_deref(),
        Some("actual-fast")
    );
    let receipt = serde_json::to_string(&activity).unwrap();
    assert!(!receipt.contains("PRIVATE-TOPIC"));
    assert!(!receipt.contains("Uno."));
    assert!(
        generation_receipts::activity(&app.lock().unwrap().connection)
            .unwrap()
            .attempts
            .is_empty()
    );
    drop(app);
    let store = Store::open(&dir.path().join("generation.sqlite3")).unwrap();
    let mut store = store;
    assert_eq!(
        store.accept_drill_items(&id, &ids).unwrap()[0].id,
        first[0].id
    );
    store.delete_drill_item(&first[0].id).unwrap();
    assert!(store.accept_drill_items(&id, &ids[..1]).is_err());
    assert_eq!(store.drill_items("spanish").unwrap().len(), 1);
}
#[tokio::test]
async fn invalid_provider_output_keeps_receipt_usage_and_does_not_create_candidates() {
    let (url, worker) = structured_server(|_| "not JSON PRIVATE-RESPONSE".into());
    let (_dir, app) = app(&url);
    let mut request = input();
    request.topic = None;
    let id = reserve(&app, request).unwrap();
    assert!(run(&app, &id).await.is_err());
    worker.join().unwrap();
    let store = app.lock().unwrap();
    let activity = generation_receipts::activity_for(&store.connection, "drill").unwrap();
    assert_eq!(activity.attempts[0].state, "failed");
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        crate::ai::workspace_graph::receipt(&store.connection, &id)
            .unwrap()
            .unwrap()["state"],
        "failed"
    );
    assert_eq!(activity.attempts[0].input_tokens, Some(21));
    assert!(activity.attempts[0].diagnostics.is_some());
    assert!(store.drill_preview(&id).is_err());
    assert!(store.drill_items("spanish").unwrap().is_empty());
    assert_eq!(store.profile().unwrap().global.attempts, 1);
}
#[tokio::test]
async fn cancellation_before_dispatch_never_runs_or_publishes() {
    let (_dir, app) = app("http://127.0.0.1:1/v1");
    let id = reserve(&app, input()).unwrap();
    let request = app.generations.cancel(&id).unwrap().unwrap();
    generation_receipts::cancel(&mut app.lock().unwrap(), &request).unwrap();
    assert!(run(&app, &id).await.is_err());
    let store = app.lock().unwrap();
    assert!(store.drill_preview(&id).is_err());
    assert_eq!(store.profile().unwrap().global.attempts, 0);
}

#[tokio::test]
async fn cancellation_during_inference_preserves_receipt_but_rejects_late_candidates() {
    let (_dir, app) = app("http://127.0.0.1:1/v1");
    let request_id = Arc::new(Mutex::new(String::new()));
    let owned = app.clone();
    let owned_id = request_id.clone();
    let (url, worker) = structured_server(move |_| {
        let id = owned_id.lock().unwrap().clone();
        let request = owned.generations.cancel_for(&id, "drill").unwrap().unwrap();
        generation_receipts::cancel(&mut owned.lock().unwrap(), &request).unwrap();
        json!({"candidates":[candidate("Late.")]}).to_string()
    });
    app.lock()
        .unwrap()
        .connection
        .execute(
            "UPDATE ai_config SET custom_config=json_set(custom_config,'$.baseUrl',?1)",
            [url],
        )
        .unwrap();
    let id = reserve(&app, input()).unwrap();
    *request_id.lock().unwrap() = id.clone();
    assert!(run(&app, &id).await.is_err());
    worker.join().unwrap();
    let store = app.lock().unwrap();
    assert!(store.drill_preview(&id).is_err());
    let receipt = generation_receipts::activity_for(&store.connection, "drill")
        .unwrap()
        .attempts
        .remove(0);
    assert_eq!(receipt.state, "unknown");
    assert!(receipt.dispatched_at.is_some());
    let source = receipt.diagnostics.unwrap()["sourceExecution"].clone();
    assert_eq!(source["state"], "succeeded");
    assert_eq!(source["response"]["inputTokens"], 21);
    assert_eq!(store.profile().unwrap().global.attempts, 1);
    assert!(store.drill_items("spanish").unwrap().is_empty());
}

#[tokio::test]
async fn failed_candidate_publication_rolls_back_and_keeps_provider_metadata() {
    let (url, worker) =
        structured_server(|_| json!({"candidates":[candidate("Stored?")]}).to_string());
    let (_dir, app) = app(&url);
    let id = reserve(&app, input()).unwrap();
    app.lock().unwrap().connection.execute_batch("CREATE TRIGGER block_candidates BEFORE INSERT ON drill_candidates BEGIN SELECT RAISE(ABORT,'fixture candidate storage failure'); END").unwrap();
    assert!(run(&app, &id).await.is_err());
    worker.join().unwrap();
    let store = app.lock().unwrap();
    assert!(store.drill_preview(&id).is_err());
    let receipt = generation_receipts::activity_for(&store.connection, "drill")
        .unwrap()
        .attempts
        .remove(0);
    assert_eq!(receipt.state, "failed");
    assert_eq!(receipt.input_tokens, Some(21));
    assert_eq!(
        receipt.diagnostics.as_ref().unwrap()["sourceExecution"]["state"],
        "succeeded"
    );
    assert_eq!(receipt.provider_id.as_deref(), Some("structured-receipt"));
    assert_eq!(
        store
            .connection
            .query_row::<i64, _, _>("SELECT count(*) FROM drill_candidates", [], |r| r.get(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn lengths_reach_provider_enforce_bounds_and_survive_acceptance() {
    use crate::drill::{generation::DrillLength, previews::DrillSource};
    for (length, text) in [
        (DrillLength::Word, "猫"),
        (DrillLength::ShortPhrase, "猫のそば"),
        (DrillLength::Sentence, "猫がいます。"),
        (DrillLength::SeveralSentences, "猫がいます。犬もいます。"),
    ] {
        // Supplementary-plane characters occupy two UTF-16 units. This would
        // incorrectly pass a scalar-count-only bound for every preset.
        let oversized = "𠮷".repeat(length.max_units() / 2 + 1);
        let (url, worker) = structured_server(move |_| {
            json!({"candidates":[candidate(text),candidate(&oversized)]}).to_string()
        });
        let (_dir, app) = app(&url);
        let mut requested = input();
        requested.language = "japanese".into();
        requested.topic = None;
        requested.length = length;
        requested.difficulty = model::Difficulty::Advanced;
        requested.count = 2;
        let budget = requested.output_budget();
        let id = reserve(&app, requested).unwrap();
        let preview = run(&app, &id).await.unwrap();
        let payload = worker.join().unwrap();
        assert_eq!(payload["items"][0]["request"]["max_tokens"], budget);
        let messages = &payload["items"][0]["request"]["messages"];
        let sent: serde_json::Value =
            serde_json::from_str(messages[1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(sent["length"], serde_json::to_value(length).unwrap());
        assert_eq!(sent["maxUtf16Units"], length.max_units());
        assert_eq!(sent["count"], 2);
        assert!(sent["topic"].is_null());
        assert!(
            messages[0]["content"]
                .as_str()
                .unwrap()
                .contains("Advanced difficulty level")
        );
        assert_eq!(preview.candidates.len(), 1);
        assert_eq!(preview.candidates[0].text, text);
        assert_eq!(preview.shortfall.unwrap().produced, 1);
        let mut store = app.lock().unwrap();
        assert_eq!(
            store.drill_preview(&id).unwrap().requested.unwrap().length,
            length
        );
        let items = store
            .accept_drill_items(&id, &[preview.candidates[0].candidate_id.clone()])
            .unwrap();
        let DrillSource::Generated {
            length: saved,
            difficulty,
            ..
        } = &items[0].source
        else {
            panic!("generated provenance missing");
        };
        assert_eq!(*saved, length);
        assert_eq!(*difficulty, model::Difficulty::Advanced);
        assert_eq!(store.profile().unwrap().global.attempts, 1);
    }
}

#[test]
fn generation_rejects_missing_or_unknown_length_before_reservation() {
    let mut value = serde_json::to_value(input()).unwrap();
    value["length"] = json!("unbounded");
    assert!(serde_json::from_value::<DrillGenerationInput>(value.clone()).is_err());
    value.as_object_mut().unwrap().remove("length");
    assert!(serde_json::from_value::<DrillGenerationInput>(value).is_err());
}

#[test]
fn output_budget_scales_with_length_and_count_within_shared_provider_bounds() {
    use crate::drill::generation::DrillLength;
    let mut request = input();
    request.count = 1;
    request.length = DrillLength::Word;
    let word = request.output_budget();
    request.count = 20;
    let words = request.output_budget();
    request.length = DrillLength::SeveralSentences;
    let paragraphs = request.output_budget();
    assert!(word < words && words < paragraphs);
    assert!((2048..=32768).contains(&word));
    assert!((2048..=32768).contains(&paragraphs));
}

#[path = "proposal_lifecycle.rs"]
mod proposal_lifecycle;
