//! Recomputable learner state. Never writes evidence or changes XP.
//! Logistic update follows [@pelanek2016]; recall shape follows [@settles_meeder2016].
//! Weights, half-life multipliers and uncertainty are uncalibrated product heuristics.
use crate::configuration::Registry;
use crate::model::*;
use crate::storage::store::Store;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructState {
    pub construct_id: String,
    pub variety_id: String,
    pub rating: f64,
    pub uncertainty: f64,
    #[ts(type = "number")]
    pub last_seen: i64,
    pub half_life_days: f64,
    pub n: u32,
    pub independent_n: u32,
    pub effective_n: f64,
    pub recall: f64,
    #[ts(type = "number")]
    pub due_at: i64,
    pub due: bool,
    pub insufficient_evidence: bool,
    pub evidence_attempt_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LearnerState {
    pub learner_id: String,
    pub language_id: String,
    #[ts(type = "number")]
    pub as_of_secs: i64,
    pub config_hash: String,
    pub construct_registry_hash: String,
    pub estimator_hash: String,
    pub estimator_version: u32,
    pub calibration: String,
    #[ts(type = "unknown")]
    pub choices: Value,
    #[ts(type = "unknown[]")]
    pub observations: Vec<Value>,
    pub constructs: Vec<ConstructState>,
}
fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| invalid(&format!("Learner evidence lacks {key}.")))
}
pub fn fold(registry: &Registry, evidence: &Value, as_of_secs: i64) -> Result<LearnerState> {
    if as_of_secs < 0 {
        return Err(invalid("Learner-state time must be nonnegative."));
    }
    let language = string(evidence, "target")?;
    registry.language(language)?;
    let learner = string(evidence, "learner_id")?;
    let records = evidence["records"]
        .as_array()
        .ok_or_else(|| invalid("Missing learner observations."))?;
    let choices = &evidence["profile"]["choices"];
    let excluded = choices["excluded_attempts"]
        .as_array()
        .ok_or_else(|| invalid("Missing evidence exclusions."))?;
    let policy = registry.estimator_policy();
    let hash = crate::learning::coaching::construct_hash(registry);
    let mut sorted: Vec<_> = records.iter().collect();
    sorted.sort_by_key(|r| {
        (
            r["at_secs"].as_i64(),
            r["chat_id"].as_str(),
            r["message_id"].as_i64(),
            r["attempt_id"].as_str(),
        )
    });
    let mut states: BTreeMap<(String, String), ConstructState> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for record in sorted {
        if record["status"] != "complete" || record["construct_registry_hash"] != hash {
            continue;
        }
        if string(record, "learner_id")? != learner || string(record, "target")? != language {
            return Err(invalid("Learner evidence crosses its owner or language."));
        }
        let attempt = string(record, "attempt_id")?;
        if excluded.iter().any(|id| id.as_str() == Some(attempt)) {
            continue;
        }
        let at = record["at_secs"]
            .as_i64()
            .filter(|at| *at >= 0)
            .ok_or_else(|| invalid("Invalid observation time."))?;
        if at > as_of_secs {
            continue;
        }
        let variety = record["variety"].as_str().unwrap_or("");
        let judgments = record["assessment"]["judgments"]
            .as_array()
            .ok_or_else(|| invalid("Complete evidence lacks judgments."))?;
        for item in judgments {
            // Presence is experience evidence, never a success grade for this estimator.
            if item.get("presence").is_some() {
                continue;
            }
            let outcome = match string(item, "outcome")? {
                "demonstrated" => 1.0,
                "partial" => 0.5,
                "not_demonstrated" => 0.0,
                "not_observed" | "uncertain" => continue,
                _ => return Err(invalid("Unknown observation outcome.")),
            };
            let construct = string(item, "skill_id")?;
            registry.skill_definition(construct)?;
            let source = string(record, "source")?;
            let quotes = item["quotes"]
                .as_array()
                .ok_or_else(|| invalid("Missing evidence quotes."))?;
            let whole = item["evidence_kind"] == "whole_message"
                && record["assessment_adapter"] == "jev_choice";
            if (whole && (!quotes.is_empty() || source.trim().is_empty()))
                || (!whole
                    && (quotes.is_empty()
                        || quotes.iter().any(|q| {
                            q.as_str()
                                .is_none_or(|q| q.trim().is_empty() || !source.contains(q))
                        })))
            {
                return Err(invalid("Invalid learner-state source quote."));
            }
            // Retries or replayed identical wording cannot manufacture independent evidence.
            let wording = source.split_whitespace().collect::<Vec<_>>().join(" ");
            if !seen.insert((variety.to_owned(), construct.to_owned(), wording)) {
                continue;
            }
            let step = item["support_step"]
                .as_str()
                .or(record["support_step"].as_str());
            let step = step.unwrap_or(
                if record["input"]["suggestion"] == true || record["input"]["scaffold"] == true {
                    "suggestion"
                } else if record["input"]["revision"] == true {
                    "revision"
                } else {
                    "none"
                },
            );
            let weight = *policy
                .support
                .get(step)
                .ok_or_else(|| invalid("Unknown evidence support step."))?;
            let state = states
                .entry((variety.into(), construct.into()))
                .or_insert_with(|| ConstructState {
                    construct_id: construct.into(),
                    variety_id: variety.into(),
                    rating: policy.initial_rating,
                    uncertainty: 1.0,
                    last_seen: at,
                    half_life_days: policy.initial_half_life_days,
                    n: 0,
                    independent_n: 0,
                    effective_n: 0.0,
                    recall: 1.0,
                    due_at: at,
                    due: false,
                    insufficient_evidence: true,
                    evidence_attempt_ids: vec![],
                });
            let expected = 1.0 / (1.0 + (-state.rating).exp());
            state.rating += policy.learning_rate * weight * (outcome - expected);
            state.half_life_days = (state.half_life_days
                * (policy.success_growth.powf(outcome)
                    * policy.failure_shrink.powf(1.0 - outcome))
                .powf(weight))
            .clamp(policy.min_half_life_days, policy.max_half_life_days);
            state.n += 1;
            state.independent_n += u32::from(step == "none");
            state.effective_n += weight;
            state.uncertainty = 1.0 / (1.0 + state.effective_n).sqrt();
            state.last_seen = at;
            state.evidence_attempt_ids.push(attempt.into());
        }
    }
    for state in states.values_mut() {
        let elapsed = (as_of_secs - state.last_seen) as f64 / 86400.0;
        state.recall = 2.0_f64.powf(-elapsed / state.half_life_days);
        let interval = -policy.due_recall.log2() * state.half_life_days * 86400.0;
        state.due_at = state.last_seen.saturating_add(interval.ceil() as i64);
        state.due = as_of_secs >= state.due_at;
        state.insufficient_evidence = state.independent_n < policy.minimum_independent_observations;
    }
    Ok(LearnerState {
        learner_id: learner.into(),
        language_id: language.into(),
        as_of_secs,
        config_hash: registry.hash().into(),
        construct_registry_hash: hash,
        estimator_hash: registry.estimator_hash(),
        estimator_version: policy.version,
        calibration: "uncalibrated_product_heuristic".into(),
        choices: choices.clone(),
        observations: records.clone(),
        constructs: states.into_values().collect(),
    })
}
pub fn snapshot(store: &Store, target: &str, at: i64) -> Result<LearnerState> {
    fold(
        &store.config,
        &crate::learning::learner::progression::snapshot(store, target)?,
        at,
    )
}
/// Partner selection scopes both records and credited counts. Exclusion choices
/// remain language-wide so changing a filter cannot erase other exclusions.
fn profile(store: &Store, target: &str, persona_id: Option<&str>, _at: i64) -> Result<Value> {
    let mut evidence = crate::learning::learner::progression::snapshot(store, target)?;
    let learner = string(&evidence, "learner_id")?;
    let partners=store.connection.prepare("SELECT p.id,json_extract(p.details,'$.name'),r.archived FROM personas p JOIN contacts r ON r.persona_id=p.id AND r.learner_id=p.learner_id WHERE p.language_id=?1 AND p.learner_id=?2 AND EXISTS(SELECT 1 FROM conversations c WHERE c.contact_id=r.id AND c.language_id=?1) ORDER BY p.id")?.query_map(rusqlite::params![target,learner],|r|Ok(serde_json::json!({"personaId":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"archived":r.get::<_,bool>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    if let Some(persona) = persona_id {
        if !partners.iter().any(|p| p["personaId"] == persona) {
            return Err(invalid(
                "The selected partner does not belong to this learner and language, or has no retained conversations.",
            ));
        }
        let chats=store.connection.prepare("SELECT c.id FROM conversations c JOIN contacts r ON r.id=c.contact_id WHERE r.persona_id=?1 AND r.learner_id=?2 AND c.language_id=?3")?.query_map(rusqlite::params![persona,learner,target],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<BTreeSet<_>>>()?;
        let records = evidence["records"]
            .as_array_mut()
            .ok_or_else(|| invalid("Missing learner observations."))?;
        let mut scoped = Vec::new();
        for record in records.drain(..) {
            if chats.contains(string(&record, "chat_id")?) {
                scoped.push(record);
            }
        }
        *records = scoped;
    }
    // Counts and source records must have the same partner scope.
    let attempts: BTreeSet<String> = evidence["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["attempt_id"].as_str().unwrap().to_owned())
        .collect();
    let credits: Vec<Value> = evidence["profile"]["credits"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| attempts.contains(c["attempt_id"].as_str().unwrap()))
        .cloned()
        .collect();
    for skill in evidence["profile"]["skills"].as_array_mut().unwrap() {
        let own: Vec<_> = credits
            .iter()
            .filter(|c| c["skill_id"] == skill["skill_id"])
            .collect();
        for field in ["experience", "effort", "xp"] {
            skill[field] =
                serde_json::json!(own.iter().map(|c| c[field].as_u64().unwrap()).sum::<u64>());
        }
        skill["checked"] = serde_json::json!(skill["experience"].as_u64().unwrap() > 0);
        skill["star"] = serde_json::json!(false);
    }
    evidence["profile"]["xp"] = serde_json::json!(
        credits
            .iter()
            .map(|c| c["xp"].as_u64().unwrap())
            .sum::<u64>()
    );
    evidence["profile"]["levels"] = serde_json::to_value(super::skill_levels::project(
        &evidence["catalog"],
        &credits,
    )?)?;
    evidence["profile"]["credits"] = serde_json::json!(credits);
    if persona_id.is_some() {
        evidence["profile"]["pendingLevelEvents"] = serde_json::json!([]);
    }
    Ok(
        serde_json::json!({"evidence":evidence,"partners":partners,"scope":{"languageId":target,"personaId":persona_id}}),
    )
}
/// Evidence and counts share one locked read so exclusions cannot race the UI.
#[tauri::command]
pub(crate) fn get_learner_profile(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
    persona_id: Option<String>,
) -> Result<Value> {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| invalid("System clock precedes epoch."))?
        .as_secs() as i64;
    let store = state.lock()?;
    profile(&store, &target, persona_id.as_deref(), at)
}
#[tauri::command]
pub(crate) fn get_learner_state(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
) -> Result<LearnerState> {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| invalid("System clock precedes epoch."))?
        .as_secs() as i64;
    {
        let store = state.lock()?;
        snapshot(&store, &target, at)
    }
}
#[tauri::command]
pub(crate) fn export_learner_state(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
) -> Result<String> {
    let snapshot = crate::learning::learner::progression::snapshot(&*state.lock()?, &target)?;
    serde_yaml_ng::to_string(&snapshot)
        .map_err(|e| invalid(&format!("Learner-state export failed: {e}")))
}

/// Save a fresh native projection, never a frontend-supplied assessment.
#[tauri::command]
pub(crate) fn save_learner_state(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
) -> Result<String> {
    use tauri::Manager;
    let model = crate::learning::learner::progression::snapshot(&*state.lock()?, &target)?;
    let downloads = app.path().download_dir().map_err(|error| {
        AppError::new(
            ErrorCode::Storage,
            format!("Could not locate Downloads: {error}"),
        )
    })?;
    let path = downloads.join(format!(
        "skellyspeak-learning-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    write_export(&path, &model)?;
    Ok(path.display().to_string())
}

fn write_export(path: &std::path::Path, model: &impl serde::Serialize) -> Result<()> {
    use std::io::Write;
    let bytes = serde_yaml_ng::to_string(model).map_err(|error| {
        AppError::new(
            ErrorCode::Storage,
            format!("Could not serialize learning evidence: {error}"),
        )
    })?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            AppError::new(
                ErrorCode::Storage,
                format!("Could not create learning evidence file: {error}"),
            )
        })?;
    file.write_all(bytes.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            AppError::new(
                ErrorCode::Storage,
                format!("Learning evidence file could not be completed: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn record(registry: &Registry, id: &str, outcome: &str, step: &str, at: i64) -> Value {
        json!({"attempt_id":id,"learner_id":"l","target":"spanish","variety":"","chat_id":"c","message_id":at,"source":format!("¿Cómo estás {id}?"),"at_secs":at,"status":"complete","construct_registry_hash":crate::learning::coaching::construct_hash(registry),"input":{},"support_step":step,"assessment":{"judgments":[{"skill_id":"information_exchange","outcome":outcome,"quotes":["¿Cómo estás"]}]}})
    }
    fn evidence(records: Vec<Value>) -> Value {
        json!({"target":"spanish","learner_id":"l","records":records,"profile":{"choices":{"excluded_attempts":[]}}})
    }
    #[test]
    fn export_round_trips_owned_evidence_and_never_overwrites_a_file() {
        let r = Registry::bundled().unwrap();
        let data = evidence(vec![record(&r, "a", "demonstrated", "none", 1)]);
        let state = fold(&r, &data, 10).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.yaml");
        write_export(&path, &state).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let restored: LearnerState = serde_yaml_ng::from_str(&saved).unwrap();
        assert_eq!(restored.learner_id, state.learner_id);
        assert_eq!(restored.language_id, state.language_id);
        assert_eq!(restored.observations, state.observations);
        assert_eq!(restored.choices, state.choices);
        assert_eq!(restored.estimator_hash, state.estimator_hash);
        assert_eq!(restored.constructs[0].evidence_attempt_ids, vec!["a"]);
        assert!(write_export(&path, &state).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
        assert!(write_export(&directory.path().join("missing/learning.yaml"), &state).is_err());
    }
    #[test]
    fn ignores_absence_uncertainty_exclusions_and_old_registry_without_inventing_state() {
        let r = Registry::bundled().unwrap();
        let mut data = evidence(vec![
            record(&r, "a", "uncertain", "none", 1),
            record(&r, "b", "not_observed", "none", 2),
            record(&r, "c", "demonstrated", "none", 3),
            record(&r, "d", "demonstrated", "none", 4),
        ]);
        data["profile"]["choices"]["excluded_attempts"] = json!(["c"]);
        data["records"][3]["construct_registry_hash"] = json!("old");
        let view = fold(&r, &data, 10).unwrap();
        assert!(view.constructs.is_empty());
        assert_eq!(view.observations.len(), 4);
    }
    #[test]
    fn support_reduces_update_and_due_never_reduces_rating() {
        let r = Registry::bundled().unwrap();
        let direct = fold(
            &r,
            &evidence(vec![record(&r, "a", "demonstrated", "none", 1)]),
            1,
        )
        .unwrap();
        let assisted = fold(
            &r,
            &evidence(vec![record(&r, "a", "demonstrated", "explicit", 1)]),
            1,
        )
        .unwrap();
        assert!(direct.constructs[0].rating > assisted.constructs[0].rating);
        assert_eq!(assisted.constructs[0].independent_n, 0);
        let later = fold(
            &r,
            &evidence(vec![record(&r, "a", "demonstrated", "none", 1)]),
            864000,
        )
        .unwrap();
        assert!(later.constructs[0].due);
        assert_eq!(later.constructs[0].rating, direct.constructs[0].rating);
        assert!(later.constructs[0].insufficient_evidence);
    }
    #[test]
    fn failed_opportunity_is_negative_and_replay_does_not_inflate_evidence() {
        let r = Registry::bundled().unwrap();
        let item = record(&r, "a", "not_demonstrated", "none", 1);
        let value = evidence(vec![item.clone(), item]);
        let state = fold(&r, &value, 10).unwrap();
        assert!(state.constructs[0].rating < 0.0);
        assert_eq!(state.constructs[0].n, 1);
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            serde_json::to_value(fold(&r, &value, 10).unwrap()).unwrap()
        );
        let exported = serde_yaml_ng::to_string(&state).unwrap();
        let restored: LearnerState = serde_yaml_ng::from_str(&exported).unwrap();
        assert_eq!(restored.constructs[0].n, 1);
    }
    #[test]
    fn owner_source_and_support_errors_are_not_silent_fallbacks() {
        let r = Registry::bundled().unwrap();
        for key in ["learner_id", "support_step", "source"] {
            let mut item = record(&r, "a", "demonstrated", "none", 1);
            item[key] = json!("invalid");
            assert!(fold(&r, &evidence(vec![item]), 10).is_err());
        }
    }
    #[test]
    fn varieties_are_separate_and_future_evidence_is_not_used() {
        let r = Registry::bundled().unwrap();
        let a = record(&r, "a", "demonstrated", "none", 1);
        let mut b = record(&r, "b", "demonstrated", "none", 2);
        b["variety"] = json!("spanish-spain");
        let data = evidence(vec![record(&r, "c", "demonstrated", "none", 30), b, a]);
        let state = fold(&r, &data, 10).unwrap();
        assert_eq!(state.constructs.len(), 2);
        assert!(state.constructs.iter().all(|s| s.n == 1));
    }
    fn partner_fixture() -> (tempfile::TempDir, Store, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
        store.prepare_chat().unwrap();
        let snapshot = store.snapshot().unwrap();
        let conversation = snapshot.conversations[0].id.clone();
        let persona = snapshot.personas[0].id.clone();
        store.connection.execute("INSERT INTO personas SELECT 'second-persona',learner_id,language_id,1,details FROM personas WHERE id=?1",[&persona]).unwrap();
        store.connection.execute("INSERT INTO contacts SELECT 'second-contact',learner_id,'second-persona',0,1 FROM contacts WHERE persona_id=?1",[&persona]).unwrap();
        store.connection.execute("INSERT INTO conversations(id,contact_id,language_id,title,archived,revision,last_used) VALUES('second-chat','second-contact','spanish','Second',0,1,0)",[]).unwrap();
        for (turn, chat, source, _outcome) in [
            (
                "first-turn",
                conversation.as_str(),
                "¿Dónde está la estación?",
                "demonstrated",
            ),
            (
                "second-turn",
                "second-chat",
                "¿Dónde estación?",
                "not_demonstrated",
            ),
        ] {
            let context = json!({"constructRegistryHash":crate::learning::coaching::construct_hash(&store.config),"catalogVersion":crate::learning::coaching::version_for(&store.config),"translationLanguage":"english","practiceSettings":{"varietyId":"spanish-spain"},"input":{},"skillAssessmentAttempt":turn,"skillAssessment":{"adapter":"jev_choice","answers":{"information_exchange":{"choice":"direct"}}}});
            store.connection.execute("INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES(?1,?2,'succeeded',0,1,'fixture','custom','fixture',?3)",rusqlite::params![turn,chat,context.to_string()]).unwrap();
            store.connection.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text,created_at) VALUES(?1,?2,?1,1,'user',?3,'2020-01-01T00:00:00Z')",rusqlite::params![turn,chat,source]).unwrap();
        }
        (dir, store, persona, conversation)
    }
    #[test]
    fn partner_scope_filters_sources_before_fold_and_leaves_global_exports_unchanged() {
        let (_dir, store, persona, chat) = partner_fixture();
        let at = 2_000_000_000;
        let export_before =
            serde_yaml_ng::to_string(&snapshot(&store, "spanish", at).unwrap()).unwrap();
        let all = profile(&store, "spanish", None, at).unwrap();
        let first = profile(&store, "spanish", Some(&persona), at).unwrap();
        let second = profile(&store, "spanish", Some("second-persona"), at).unwrap();
        assert!(all.get("model").is_none());
        assert_eq!(first["evidence"]["records"].as_array().unwrap().len(), 1);
        assert_eq!(second["evidence"]["records"].as_array().unwrap().len(), 1);
        assert_eq!(first["evidence"]["records"][0]["chat_id"], chat);
        assert_eq!(
            first["evidence"]["records"][0]["source"],
            "¿Dónde está la estación?"
        );
        assert_eq!(
            second["evidence"]["records"][0]["source"],
            "¿Dónde estación?"
        );
        assert_eq!(first["evidence"]["profile"], all["evidence"]["profile"]);
        assert_eq!(all["partners"].as_array().unwrap().len(), 2);
        assert_eq!(
            first["scope"],
            json!({"languageId":"spanish","personaId":persona})
        );
        assert_eq!(all["scope"]["personaId"], Value::Null);
        assert_eq!(
            serde_yaml_ng::to_string(&snapshot(&store, "spanish", at).unwrap()).unwrap(),
            export_before
        );
    }
    #[test]
    fn partner_counts_and_exports_follow_eligible_credit_scope() {
        let (_dir, store, persona, _) = partner_fixture();
        for turn in ["first-turn", "second-turn"] {
            let event = json!({"id":turn,"attemptId":turn,"constructId":"information_exchange",
                "kind":"experience","tier":1,"xp":1,"experience":1,"effort":0,
                "quote":"","support":"not_weighted","difficulty":"not_weighted",
                "novelty":"not_weighted","policyHash":"experience-effort-1","atSecs":1,"claimed":false});
            store.connection.execute("UPDATE turns SET context=json_set(context,'$.rewardEvents',json(?1),'$.gamePolicy',json('{}')) WHERE id=?2",
                rusqlite::params![json!([event]).to_string(),turn]).unwrap();
        }
        let all = profile(&store, "spanish", None, 0).unwrap();
        let scoped = profile(&store, "spanish", Some(&persona), 0).unwrap();
        assert_eq!(all["evidence"]["profile"]["xp"], 2);
        assert_eq!(scoped["evidence"]["profile"]["xp"], 1);
        let points = |value: &Value| {
            value["evidence"]["profile"]["levels"]["skills"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["skillId"] == "information_exchange")
                .unwrap()["points"]
                .clone()
        };
        assert_eq!(points(&all), 2);
        assert_eq!(points(&scoped), 1);
        assert_eq!(
            scoped["evidence"]["profile"]["pendingLevelEvents"],
            json!([])
        );
        assert_eq!(
            scoped["evidence"]["profile"]["credits"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        store.connection.execute("INSERT INTO skill_choices(language_id,revision,focus,excluded) VALUES('spanish',1,NULL,'[\"first-turn\"]')",[]).unwrap();
        assert_eq!(
            profile(&store, "spanish", Some(&persona), 0).unwrap()["evidence"]["profile"]["xp"],
            0
        );
        store
            .connection
            .execute("UPDATE skill_choices SET excluded='[]',revision=2", [])
            .unwrap();
        assert_eq!(
            profile(&store, "spanish", Some(&persona), 0).unwrap()["evidence"]["profile"]["xp"],
            1
        );
        let exported = crate::learning::learner::progression::snapshot(&store, "spanish").unwrap();
        assert_eq!(exported["profile"]["xp"], 2);
        assert!(exported.get("model").is_none());
    }
    #[test]
    fn partner_exclusion_restore_and_archival_preserve_identity() {
        let (_dir, store, persona, _) = partner_fixture();
        let at = 2_000_000_000;
        let initial = profile(&store, "spanish", Some(&persona), at).unwrap();
        store.connection.execute("INSERT INTO skill_choices(language_id,revision,focus,excluded) VALUES('spanish',1,NULL,'[\"first-turn\"]')",[]).unwrap();
        let excluded = profile(&store, "spanish", Some(&persona), at).unwrap();
        assert_eq!(
            excluded["evidence"]["profile"]["choices"]["excluded_attempts"],
            serde_json::json!(["first-turn"])
        );
        assert_eq!(
            profile(&store, "spanish", Some("second-persona"), at).unwrap()["evidence"]["records"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        store
            .connection
            .execute("UPDATE skill_choices SET excluded='[]',revision=2", [])
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE contacts SET archived=1 WHERE persona_id=?1",
                [&persona],
            )
            .unwrap();
        store.connection.execute("UPDATE personas SET details=json_set(details,'$.name','Renamed partner') WHERE id=?1",[&persona]).unwrap();
        let restored = profile(&store, "spanish", Some(&persona), at).unwrap();
        assert_eq!(
            restored["evidence"]["profile"]["xp"],
            initial["evidence"]["profile"]["xp"]
        );
        let selected = restored["partners"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["personaId"] == persona)
            .unwrap();
        assert_eq!(selected["name"], "Renamed partner");
        assert_eq!(selected["archived"], true);
    }
    #[test]
    fn invalid_partner_or_language_fails_instead_of_showing_global_estimates() {
        let (_dir, store, persona, _) = partner_fixture();
        for (language, selected) in [
            ("spanish", "missing"),
            ("spanish", ""),
            ("arabic", persona.as_str()),
            ("missing", persona.as_str()),
        ] {
            assert!(profile(&store, language, Some(selected), 2_000_000_000).is_err());
        }
    }
}
