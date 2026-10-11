//! Deterministic experience/effort projections over retained presence observations.
use crate::model::*;
use crate::storage::store::Store;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::sync::Arc;

pub fn snapshot(store: &Store, target: &str) -> Result<Value> {
    store.config.language(target)?;
    let mut result = project(
        &store.connection,
        &store.config,
        &store.session_id,
        target,
        Some(&store.graph_runtime),
    )?;
    let raw: String = store
        .connection
        .query_row("SELECT preferences FROM learner", [], |r| r.get(0))?;
    let preferences: Preferences = serde_json::from_str(&raw)?;
    // Guide selection is shared across varieties; only rendering depends on variety.
    let editions = store
        .config
        .skills_for_language(target)?
        .into_iter()
        .map(|skill| {
            store.config.guide_source(target, &skill.id)?;
            Ok((
                skill.id.clone(),
                store
                    .config
                    .guide_edition(target, &preferences.explanation_language, &skill.id)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut guides = vec![];
    for variety in store.config.language(target)?.varieties {
        let skills: serde_json::Map<String, Value> = editions
            .iter()
            .map(|(id, edition)| {
                (
                    id.clone(),
                    json!(
                        edition
                            .as_ref()
                            .map(|edition| edition.markdown(&variety.id))
                    ),
                )
            })
            .collect();
        guides.push(json!({"id":variety.id,"name":variety.name,"skills":skills}));
    }
    result["guides"] = json!(guides);
    result["guide_explanation_language"] = json!(preferences.explanation_language);
    Ok(result)
}
pub(crate) fn snapshot_db(
    db: &Connection,
    registry: &crate::configuration::Registry,
    session: &str,
    target: &str,
) -> Result<Value> {
    project(db, registry, session, target, None)
}

fn project(
    db: &Connection,
    registry: &crate::configuration::Registry,
    session: &str,
    target: &str,
    runtime: Option<&crate::conversations::execution::graph_runtime::Runtime>,
) -> Result<Value> {
    let construct_hash = crate::learning::coaching::construct_hash(registry);
    let catalog_version = crate::learning::coaching::version_from_hash(&construct_hash);
    let learner: String = db.query_row("SELECT id FROM learner", [], |r| r.get(0))?;
    let choice: Option<(i32, Option<String>, String)> = db
        .query_row(
            "SELECT revision,focus,excluded FROM skill_choices WHERE language_id=?1",
            [target],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (revision, focus, excluded) = choice.unwrap_or((0, None, "[]".into()));
    let excluded: Vec<String> = serde_json::from_str(&excluded)?;
    // Accepted observations and credit are domain records. Live node status is
    // supplied only for the evidence view; transaction-time credit projection
    // does not read an engine while its adoption is being committed.
    let rows=db.prepare("SELECT t.id,t.conversation_id,m.sequence,m.text,t.model,t.route,t.context,CAST(strftime('%s',m.created_at) AS INTEGER),m.id FROM turns t JOIN conversations c ON c.id=t.conversation_id JOIN messages m ON m.turn_id=t.id AND m.role='user' WHERE c.language_id=?1 AND (json_type(t.context,'$.skillAssessment')='object' OR (?2 AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=t.id AND o.executor='graph' AND o.channel='persona_reply'))) ORDER BY m.created_at,t.id")?.query_map(params![target,runtime.is_some()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i32>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,i64>(7)?,r.get::<_,String>(8)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut records = vec![];
    for (turn, chat, sequence, source, model, route, context, time, message) in rows {
        let context: Value = serde_json::from_str(&context)?;
        let assessment = context.get("skillAssessment").filter(|a| a.is_object());
        let status = if assessment.is_none() {
            runtime
                .map(|runtime| {
                    runtime.source_status(
                        db,
                        &turn,
                        &crate::language::source_graph::SourceText {
                            id: message,
                            text: source.clone(),
                        },
                    )
                })
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .rev()
                .find(|item| {
                    item.operation
                        == crate::learning::coaching::assessment_graph::operation_contract()
                })
        } else {
            None
        };
        let state = status.as_ref().and_then(|item| item.state.as_deref());
        if assessment.is_none() && state.is_none() {
            continue;
        }
        let attempt = context
            .get("skillAssessmentAttempt")
            .and_then(Value::as_str)
            .unwrap_or(&turn);
        let judgments: Vec<Value> = assessment.and_then(|a| a["answers"].as_object()).map(|answers| answers.iter().map(|(id, answer)| {
            let evidence = &context["skillAttribution"]["skills"][id];
            let spans = evidence["spans"].as_array().cloned().unwrap_or_default();
            let quotes: Vec<_> = spans.iter().map(|s|s["quote"].clone()).collect();
            json!({"skill_id":id,"presence":assessment.unwrap()["presence"][id],"evidence_kind":if spans.is_empty(){"whole_message"}else{"quoted"},"answer":answer,"quotes":quotes,"spans":spans,"rationale":"","attribution_reason":evidence["reason"]})
        }).collect()).unwrap_or_default();
        let attribution_state = context
            .get("skillAttribution")
            .filter(|a| a.is_object())
            .map(|_| "succeeded");

        records.push(json!({"attribution_state":attribution_state,"attribution_error":context["skill_attributionError"],"attribution_attempt":context["skillAttributionAttempt"],"reward_credits":crate::learning::rewards::credits(&context)?,"attempt_id":attempt,"session_id":session,"turn_id":sequence,"message_id":sequence,"replaces_message_id":db.query_row("SELECT m.sequence FROM turns t JOIN messages m ON m.turn_id=t.replaces_turn_id AND m.role='user' WHERE t.id=?1",[&turn],|r|r.get::<_,i32>(0)).optional()?,"chat_id":chat,"learner_id":learner,"target":target,"variety":context["practiceSettings"]["varietyId"],"native":context["translationLanguage"],"source":source,"input":context["input"],"support_step":Value::Null,"at_secs":time,"model":assessment.and_then(|a|a.get("model")).cloned().unwrap_or(json!(model)),"provider_mode":assessment.and_then(|a|a.get("providerMode")).cloned().unwrap_or(json!(route)),"catalog_version":context["catalogVersion"],"construct_registry_hash":context["constructRegistryHash"],"mapping_error":if context["constructRegistryHash"]!=construct_hash{json!("This observation uses a different construct registry. Its evidence is retained; current credit is unavailable.")}else if assessment.is_some() && context.get("gamePolicy").is_none(){json!("This observation predates the durable reward policy. Its evidence is retained; start a new exchange to earn current rewards.")}else{Value::Null},"assessment_adapter":assessment.and_then(|a|a.get("adapter")).cloned().unwrap_or(json!("jev_choice")),"decision_policy":assessment.and_then(|a|a.get("policy")).cloned().unwrap_or(Value::Null),"prompt_version":assessment.and_then(|a|a.get("promptVersion")).cloned().unwrap_or_else(||if context.get("skillAssessment").is_some(){context["skillAssessmentPromptVersion"].clone()}else{context["coachFeedbackPromptVersion"].clone()}),"status":if assessment.is_some(){"complete"}else if !matches!(state,Some("ready"|"running"|"waiting_dependencies"|"held")){"failed"}else{"pending"},"assessment":if assessment.is_some(){json!({"judgments":judgments})}else{Value::Null},"error":if assessment.is_none() && !matches!(state,Some("ready"|"running"|"waiting_dependencies"|"held")) {status.as_ref().and_then(|item| item.error.as_ref()).map(|error|json!(error)).unwrap_or_else(||json!(format!("Skill presence unavailable: {}.",state.unwrap_or("unknown"))))} else {Value::Null}}));
    }
    let source_catalog = registry.practice_catalog(target)?;
    let catalog = source_catalog;
    let mut skills = vec![];
    let mut eligible_credits = vec![];
    for record in &records {
        if record["construct_registry_hash"] != construct_hash
            || excluded.iter().any(|id| record["attempt_id"] == *id)
        {
            continue;
        }
        eligible_credits.extend(
            record["reward_credits"]
                .as_array()
                .ok_or_else(|| AppError::new(ErrorCode::Storage, "Missing reward ledger."))?
                .iter()
                .cloned(),
        );
    }
    let levels = super::skill_levels::project(&catalog, &eligible_credits)?;
    let mut credits = vec![];
    for node in catalog
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["kind"] == "skill")
    {
        let mut experience = 0_u64;
        let mut effort = 0_u64;
        let mut skill_xp = 0_u64;
        for credit in &eligible_credits {
            if credit["skill_id"] != node["id"] {
                continue;
            }
            skill_xp += credit["xp"]
                .as_u64()
                .ok_or_else(|| AppError::new(ErrorCode::Storage, "Invalid reward XP."))?;
            experience += credit["experience"]
                .as_u64()
                .ok_or_else(|| AppError::new(ErrorCode::Storage, "Missing experience credit."))?;
            effort += credit["effort"]
                .as_u64()
                .ok_or_else(|| AppError::new(ErrorCode::Storage, "Missing effort credit."))?;
            credits.push(credit.clone());
        }
        skills.push(json!({"skill_id":node["id"],"experience":experience,"effort":effort,"xp":skill_xp,"checked":experience>0,"star":false}));
    }

    let branches:Vec<_>=catalog.as_array().unwrap().iter().filter(|n|n["kind"]=="skill").map(|n|json!({"skill_id":n["id"],"available":catalog.as_array().unwrap().iter().any(|p|p["id"]==n["parent"]&&p["kind"]=="root")||skills.iter().any(|p|p["skill_id"]==n["parent"]&&p["star"]==true)})).collect();
    let recommended = skills
        .iter()
        .filter(|s| {
            s["star"] != true
                && branches
                    .iter()
                    .any(|b| b["skill_id"] == s["skill_id"] && b["available"] == true)
        })
        .min_by_key(|s| s["experience"].as_u64().unwrap())
        .unwrap_or(&skills[0])["skill_id"]
        .clone();
    let xp: u64 = skills
        .iter()
        .map(|s| s["xp"].as_u64().unwrap())
        .sum::<u64>();
    let count: i64 = db.query_row(
        "SELECT count(*) FROM conversations WHERE language_id=?1",
        [target],
        |r| r.get(0),
    )?;
    let mut result = json!({"catalog":catalog,"catalog_version":catalog_version,"construct_registry_hash":construct_hash,"learner_id":learner,"target":target,"conversation_count":count,"records":records,"profile":{"levels":levels,"rules_version":3,"choices":{"version":1,"revision":revision,"learner_id":learner,"target":target,"focus":focus,"excluded_attempts":excluded},"xp":xp,"skills":skills,"branches":branches,"credits":credits,"recommended_focus":recommended,"active_focus":focus.map(Value::String).unwrap_or(recommended)}});
    result["profile"]["pendingLevelEvents"] = serde_json::to_value(
        crate::learning::rewards::skill_level_events::pending(db, &result)?,
    )?;
    Ok(result)
}
#[tauri::command]
pub(crate) async fn get_skill_evidence(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
) -> Result<Value> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = state.lock()?;
        snapshot(&store, &target)
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(
            &cause,
            "skill_evidence_read",
            AppError::new(ErrorCode::Internal, "Could not read skill evidence."),
        )
    })?
}
/// One row per catalog language: its XP, saved conversations and lifetime effort
/// units, for comparing languages without loading every evidence snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LanguageTotals {
    pub target: String,
    pub name: String,
    pub native_name: String,
    pub language_tag: Option<String>,
    pub xp: u32,
    pub conversations: u32,
    pub partner_understood: u32,
    pub revisions_sent: u32,
    pub practice_attempts: u32,
    pub explorations: u32,
    pub bot: u32,
}
#[tauri::command]
pub(crate) async fn get_language_totals(
    state: tauri::State<'_, Arc<crate::application::Application>>,
) -> Result<Vec<LanguageTotals>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = state.lock()?;
        language_totals(&store)
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(
            &cause,
            "language_totals_read",
            AppError::new(ErrorCode::Internal, "Could not read language totals."),
        )
    })?
}
fn language_totals(store: &Store) -> Result<Vec<LanguageTotals>> {
    super::language_totals::read(store)
}
#[tauri::command]
pub(crate) fn get_practice_overview(
    state: tauri::State<'_, Arc<crate::application::Application>>,
) -> Result<Value> {
    let store = state.lock()?;
    let mut languages = vec![];
    for language in store.snapshot()?.languages {
        languages.push(json!({"name":language.name,"endonym":language.native_name,"snapshot":snapshot(&store,&language.id)?}));
    }
    Ok(json!({"languages":languages}))
}
#[tauri::command]
pub(crate) fn save_skill_profile(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
    expected_revision: i32,
    choices: Value,
) -> Result<Value> {
    let store = state.lock()?;
    let current = snapshot(&store, &target)?;
    if current["profile"]["choices"]["revision"] != expected_revision
        || choices["target"] != target
        || choices["learner_id"] != current["learner_id"]
    {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Skill profile changed. Reload before editing.",
        ));
    }
    let focus = choices["focus"].as_str();
    if !choices["focus"].is_null()
        && focus.is_none_or(|f| {
            !current["catalog"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["id"] == f && n["kind"] == "skill")
        })
    {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Unknown practice skill.",
        ));
    }
    let excluded: Vec<String> = serde_json::from_value(choices["excluded_attempts"].clone())?;
    if excluded.iter().any(|id| {
        !current["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["attempt_id"] == *id)
    }) {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Unknown evidence record.",
        ));
    }
    store.connection.execute("INSERT INTO skill_choices VALUES(?1,?2,?3,?4) ON CONFLICT(language_id) DO UPDATE SET revision=excluded.revision,focus=excluded.focus,excluded=excluded.excluded",params![target,expected_revision+1,focus,serde_json::to_string(&excluded)?])?;
    snapshot(&store, &target)
}

/// Freeze the learner-selected or transparent recommended focus into each turn.
pub(crate) fn capture_focus(
    db: &Connection,
    registry: &crate::configuration::Registry,
    session: &str,
    target: &str,
) -> Result<Value> {
    let snapshot = snapshot_db(db, registry, session, target)?;
    focus_from_snapshot(registry, &snapshot)
}
pub(crate) fn focus_from_snapshot(
    _registry: &crate::configuration::Registry,
    snapshot: &Value,
) -> Result<Value> {
    let focus = &snapshot["profile"]["active_focus"];
    if focus.is_null() {
        return Ok(Value::Null);
    }
    let node = snapshot["catalog"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == *focus)
        .ok_or_else(|| AppError::new(ErrorCode::Storage, "Unknown active practice focus."))?;
    let chosen = snapshot["profile"]["choices"]["focus"].is_string();
    Ok(
        json!({"id":focus,"label":node["label"],"opportunity":node["description"],"source":if chosen {"learner"} else {"recommended"},"reason":if chosen {"Chosen by the learner."} else {"Skill with the least recorded experience."}}),
    )
}
#[cfg(test)]
mod language_totals_tests {
    use super::*;
    #[test]
    fn every_language_reports_its_own_xp_and_effort() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("totals.sqlite3")).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO effort_awards(id,dimension,source_id,language_id,variety_id,conversation_id,policy) VALUES('a','practice_attempts','take','spanish','spanish-spain',NULL,'test')",
                [],
            )
            .unwrap();
        let rows = language_totals(&store).unwrap();
        assert_eq!(rows.len(), store.snapshot().unwrap().languages.len());
        let spanish = rows.iter().find(|row| row.target == "spanish").unwrap();
        assert_eq!(spanish.language_tag.as_deref(), Some("es"));
        assert_eq!((spanish.practice_attempts, spanish.xp), (1, 0));
        assert!(
            rows.iter()
                .filter(|row| row.target != "spanish")
                .all(|row| row.practice_attempts == 0)
        );
    }
}
