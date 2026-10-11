use super::*;

#[test]
fn cantonese_app_language_seeds_explanations_and_survives_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let contact = contact(&mut store);
    let learner = store.snapshot().unwrap().learner;
    let mut preferences = learner.preferences;
    preferences.interface_locale = "cantonese".into();
    preferences.explanation_language = "cantonese".into();
    preferences.explanation_variety_id = "cantonese-hong-kong".into();
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: learner.revision,
            name: learner.name,
            preferences,
        },
    );
    let chat = conversation(&mut store, &contact, "Cantonese explanations");
    assert_eq!(chat.settings.explanation_language, "cantonese");
    assert_eq!(chat.settings.explanation_variety_id, "cantonese-hong-kong");
    drop(store);
    let snapshot = Store::open(&path).unwrap().snapshot().unwrap();
    assert_eq!(snapshot.learner.preferences.interface_locale, "cantonese");
    assert_eq!(
        snapshot.learner.preferences.explanation_language,
        "cantonese"
    );
    assert_eq!(snapshot.conversations[0].settings, chat.settings);
}

#[test]
fn voice_defaults_are_persistent_and_opt_out_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("skellyspeak.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    let conversation = store.snapshot().unwrap().conversations.remove(0);
    assert!(conversation.settings.auto_send && conversation.settings.read_aloud);
    let mut settings = conversation.settings;
    settings.auto_send = false;
    settings.read_aloud = false;
    apply(
        &mut store,
        Action::UpdateSettings {
            conversation_id: conversation.id,
            expected_revision: conversation.settings_revision,
            settings,
        },
    );
    drop(store);
    let store = Store::open(&path).unwrap();
    let settings = &store.snapshot().unwrap().conversations[0].settings;
    assert!(!settings.auto_send && !settings.read_aloud);
    assert_eq!(settings.speech_voice, "alloy");
}

#[test]
fn settings_are_independent_copies_of_last_opened_conversation_and_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("skellyspeak.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let contact = contact(&mut store);
    let first = conversation(&mut store, &contact, "Weekend plans");
    let mut settings = first.settings.clone();
    settings.difficulty = Difficulty::Advanced;
    settings.translation = false;
    apply(
        &mut store,
        Action::UpdateSettings {
            conversation_id: first.id.clone(),
            expected_revision: 1,
            settings: settings.clone(),
        },
    );
    let second = conversation(&mut store, &contact, "Kitchen stories");
    assert_eq!(second.settings, settings);
    settings.difficulty = Difficulty::Beginner;
    apply(
        &mut store,
        Action::UpdateSettings {
            conversation_id: second.id.clone(),
            expected_revision: 1,
            settings,
        },
    );
    apply(
        &mut store,
        Action::OpenConversation {
            conversation_id: first.id.clone(),
        },
    );
    let third = conversation(&mut store, &contact, "Train journey");
    assert_eq!(third.settings.difficulty, Difficulty::Advanced);
    drop(store);
    let snapshot = Store::open(&path).unwrap().snapshot().unwrap();
    // The contact's first conversation plus the three created here.
    assert_eq!(snapshot.conversations.len(), 4);
    assert_eq!(
        snapshot
            .conversations
            .iter()
            .find(|c| c.id == second.id)
            .unwrap()
            .settings
            .difficulty,
        Difficulty::Beginner
    );
    assert_eq!(snapshot.language_profiles.len(), 1);
}

#[test]
fn language_preferences_seed_new_conversations_without_rewriting_existing_ones() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let contact = contact(&mut store);
    let first = conversation(&mut store, &contact, "First");
    let learner = store.snapshot().unwrap().learner;
    let mut preferences = learner.preferences;
    preferences
        .target_varieties
        .insert("spanish".into(), "spanish-mexico".into());
    preferences.explanation_language = "english".into();
    preferences.explanation_variety_id = "english-united-kingdom".into();
    preferences.interface_locale = "german".into();
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: learner.revision,
            name: learner.name,
            preferences,
        },
    );
    let second = conversation(&mut store, &contact, "Second");
    assert_eq!(second.settings.variety_id, "spanish-mexico");
    assert_eq!(
        second.settings.explanation_variety_id,
        "english-united-kingdom"
    );
    drop(store);
    let snapshot = Store::open(&path).unwrap().snapshot().unwrap();
    assert_eq!(snapshot.learner.preferences.interface_locale, "german");
    assert_eq!(
        snapshot
            .conversations
            .iter()
            .find(|c| c.id == first.id)
            .unwrap()
            .settings,
        first.settings
    );
    assert_eq!(
        snapshot
            .conversations
            .iter()
            .find(|c| c.id == second.id)
            .unwrap()
            .settings,
        second.settings
    );
}

#[test]
fn preferences_survive_restart_and_old_session_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("db");
    let mut store = Store::open(&path).unwrap();
    let old = command(
        &store,
        Action::CreateContact {
            language_id: "spanish".into(),
            details: crate::partners::persona::starter("spanish").unwrap(),
        },
    );
    let mut preferences = store.snapshot().unwrap().learner.preferences;
    preferences.text_size = 120;
    preferences.onboarding = OnboardingStatus::Skipped;
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: 1,
            name: "Jon".into(),
            preferences,
        },
    );
    drop(store);
    let mut reopened = Store::open(&path).unwrap();
    let learner = reopened.snapshot().unwrap().learner;
    assert_eq!(learner.preferences.text_size, 120);
    assert_eq!(learner.name, "Jon");
    assert_eq!(learner.preferences.onboarding, OnboardingStatus::Skipped);
    assert_eq!(
        reopened.execute(old).unwrap_err().code,
        ErrorCode::SessionExpired
    );
}

#[test]
fn wire_contract_rejects_unknown_settings_and_language_mutation_fields() {
    let settings = languages::defaults("spanish", "english").unwrap();
    let mut json = serde_json::to_value(settings).unwrap();
    json["languageDifficulty"] = serde_json::json!("advanced");
    assert!(serde_json::from_value::<PracticeSettings>(json).is_err());
    let action = serde_json::json!({"kind":"updatePersona", "personaId":"id", "expectedRevision":1,
        "details":crate::partners::persona::starter("spanish").unwrap(), "languageId":"french"});
    assert!(serde_json::from_value::<Action>(action).is_err());
}

#[test]
fn unavailable_saved_explanation_language_is_refused_on_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.explanationLanguage','unknown_language')",[]).unwrap();
    assert!(store.snapshot().is_err());
}

#[test]
fn appearance_survives_restart_without_changing_conversation_settings() {
    use crate::configuration::appearance::SurfacePalette;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("workspace.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    let before = store.snapshot().unwrap();
    let mut preferences = before.learner.preferences.clone();
    preferences.appearance.palette = SurfacePalette::Warm;
    preferences.appearance.motion = crate::configuration::appearance::MotionPreference::Full;
    preferences.text_size = 160;
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: before.learner.revision,
            name: before.learner.name,
            preferences: preferences.clone(),
        },
    );
    drop(store);
    let after = Store::open(&path).unwrap().snapshot().unwrap();
    assert_eq!(after.learner.preferences, preferences);
    assert_eq!(
        after.conversations[0].settings,
        before.conversations[0].settings
    );
}

#[test]
fn my_languages_survive_restart_without_creating_or_removing_conversations() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("my-languages.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    let before = store.snapshot().unwrap();
    let mut preferences = before.learner.preferences.clone();
    preferences.my_languages = vec!["spanish".into(), "arabic".into()];
    preferences
        .target_varieties
        .insert("arabic".into(), "arabic-levantine".into());
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: before.learner.revision,
            name: before.learner.name.clone(),
            preferences,
        },
    );
    drop(store);
    let mut store = Store::open(&path).unwrap();
    let added = store.snapshot().unwrap();
    assert_eq!(
        added.learner.preferences.my_languages,
        vec!["spanish", "arabic"]
    );
    assert_eq!(
        added.learner.preferences.target_varieties["arabic"],
        "arabic-levantine"
    );
    assert_eq!(added.conversations.len(), before.conversations.len());
    assert_eq!(added.conversations[0].id, before.conversations[0].id);
    assert_eq!(
        added.conversations[0].settings,
        before.conversations[0].settings
    );
    let mut preferences = added.learner.preferences;
    preferences.my_languages.clear();
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: added.learner.revision,
            name: added.learner.name,
            preferences,
        },
    );
    drop(store);
    let snapshot = Store::open(&path).unwrap().snapshot().unwrap();
    assert!(snapshot.learner.preferences.my_languages.is_empty());
    assert_eq!(snapshot.conversations[0].id, before.conversations[0].id);
    assert_eq!(
        snapshot.conversations[0].settings,
        before.conversations[0].settings
    );
    assert_eq!(
        snapshot.learner.preferences.target_varieties["arabic"],
        "arabic-levantine"
    );
}

#[test]
fn my_languages_reject_unknown_and_duplicate_languages() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = Store::open(&directory.path().join("invalid.sqlite3")).unwrap();
    let learner = store.snapshot().unwrap().learner;
    for languages in [vec!["missing"], vec!["spanish", "spanish"]] {
        let mut preferences = learner.preferences.clone();
        preferences.my_languages = languages.into_iter().map(String::from).collect();
        let cmd = command(
            &store,
            Action::UpdateLearner {
                expected_revision: learner.revision,
                name: learner.name.clone(),
                preferences,
            },
        );
        assert!(store.execute(cmd).is_err());
        assert!(
            store
                .snapshot()
                .unwrap()
                .learner
                .preferences
                .my_languages
                .is_empty()
        );
    }
}

#[test]
fn onboarding_choices_and_help_dismissal_survive_restart_without_creating_a_chat() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("setup.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let learner = store.snapshot().unwrap().learner;
    assert!(learner.preferences.onboarding_required);
    let mut preferences = learner.preferences;
    preferences.interface_locale = "spanish".into();
    preferences.my_languages.push("spanish".into());
    preferences
        .target_varieties
        .insert("spanish".into(), "spanish-mexico".into());
    preferences.onboarding_language = Some("spanish".into());
    preferences.onboarding = OnboardingStatus::InProgress;
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: learner.revision,
            name: learner.name,
            preferences,
        },
    );
    assert!(store.snapshot().unwrap().conversations.is_empty());
    drop(store);
    let mut store = Store::open(&path).unwrap();
    let learner = store.snapshot().unwrap().learner;
    assert_eq!(learner.preferences.interface_locale, "spanish");
    assert!(matches!(
        learner.preferences.onboarding,
        OnboardingStatus::InProgress
    ));
    assert_eq!(
        learner.preferences.onboarding_language.as_deref(),
        Some("spanish")
    );
    let mut preferences = learner.preferences;
    preferences.onboarding_required = false;
    preferences.onboarding_language = None;
    preferences.onboarding = OnboardingStatus::Skipped;
    preferences.onboarding_help = false;
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: learner.revision,
            name: learner.name,
            preferences,
        },
    );
    drop(store);
    let store = Store::open(&path).unwrap();
    let preferences = store.snapshot().unwrap().learner.preferences;
    assert!(!preferences.onboarding_required);
    assert!(!preferences.onboarding_help);
    assert!(matches!(preferences.onboarding, OnboardingStatus::Skipped));
}

#[test]
fn an_old_not_started_flag_does_not_force_setup() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("setup.sqlite3")).unwrap();
    let mut value = serde_json::to_value(store.snapshot().unwrap().learner.preferences).unwrap();
    for key in ["onboardingRequired", "onboardingLanguage", "onboardingHelp"] {
        value.as_object_mut().unwrap().remove(key);
    }
    let preferences: Preferences = serde_json::from_value(value).unwrap();
    assert!(matches!(
        preferences.onboarding,
        OnboardingStatus::NotStarted
    ));
    assert!(!preferences.onboarding_required);
    assert!(!preferences.onboarding_help);
}

#[test]
fn script_scale_preferences_survive_restart_and_reject_invalid_values() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("script-scales.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    let before = store.snapshot().unwrap();
    let mut preferences = before.learner.preferences.clone();
    preferences.script_scales = Some([("arabic".into(), 2.0)].into());
    apply(
        &mut store,
        Action::UpdateLearner {
            expected_revision: before.learner.revision,
            name: before.learner.name,
            preferences,
        },
    );
    drop(store);
    let store = Store::open(&path).unwrap();
    let saved = store.snapshot().unwrap();
    assert_eq!(
        saved.learner.preferences.script_scales.as_ref().unwrap()["arabic"],
        2.0
    );
    assert_eq!(saved.conversations[0].id, before.conversations[0].id);
    for (language, scale) in [("arabic", 0.0), ("arabic", 3.1), ("missing", 1.0)] {
        let mut preferences = saved.learner.preferences.clone();
        preferences.script_scales = Some([(language.into(), scale)].into());
        assert!(
            crate::configuration::Registry::bundled()
                .unwrap()
                .validate_preferences(&preferences)
                .is_err()
        );
    }
}
