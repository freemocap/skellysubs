use super::*;

#[path = "assessment_tests.rs"]
mod assessment;
#[path = "execution_tests.rs"]
mod execution;
#[path = "graph_assessment_tests.rs"]
mod graph_assessments;
#[path = "graph_audio_tests.rs"]
mod graph_audio;
#[path = "graph_audio_delivery_tests.rs"]
mod graph_audio_delivery;
#[path = "graph_publication_tests.rs"]
mod graph_publication;
#[path = "graph_reply_role_tests.rs"]
mod graph_reply_roles;
#[path = "graph_reply_source_tests.rs"]
mod graph_reply_sources;
#[path = "graph_runtime_tests.rs"]
mod graph_runtime;
#[path = "graph_transport_tests.rs"]
mod graph_transport;
#[path = "reading_preload_tests.rs"]
mod reading_preload;
#[path = "skill_direction_tests.rs"]
mod skill_direction;
#[path = "speech_default_tests.rs"]
mod speech_default;
#[path = "speech_fidelity_tests.rs"]
mod speech_fidelity;
#[path = "speech_reliability_tests.rs"]
mod speech_reliability;
#[path = "streak_tests.rs"]
mod streaks;
#[path = "turn_owner_tests.rs"]
mod turn_owner;

fn baseline(path: &Path, extras: bool) -> Connection {
    let db = Connection::open(path).unwrap();
    db.execute_batch(BASELINE).unwrap();
    db.execute_batch(r#"INSERT INTO ai_config VALUES(1,1,'fixture','fixture',
        '{"transcription":{"model":"fixture"},"speech":{"model":"fixture"}}',
        0,'hosted',NULL,'',NULL,'{"baseUrl":"http://127.0.0.1:8765/v1","bearerAuth":true}','chat_model');"#).unwrap();
    if extras {
        db.execute_batch(OPTIONAL_BASELINE).unwrap();
    }
    let preferences = serde_json::json!({
        "theme":"dark", "appearance":{"palette":"warm"},
        "explanationLanguage":"english",
        "explanationVarietyId":"english-united-states",
        "interfaceLocale":"english", "targetVarieties":{}, "myLanguages":["spanish"],
        "textSize":95,"textSpacing":1,"highContrast":false,
        "onboarding":"completed","onboardingRequired":false,"onboardingHelp":false
    });
    db.execute(
        "INSERT INTO learner VALUES('learner',1,'Retained learner',7,?1)",
        [preferences.to_string()],
    )
    .unwrap();
    db.execute_batch(
        "INSERT INTO saved_topics VALUES('topic','保留 café');
        INSERT INTO effort_awards(id,dimension,source_id,language_id,variety_id,policy,claimed)
        VALUES('award','practice_attempts','attempt','spanish','fixture','original-policy',1);
        INSERT INTO reading_attempts VALUES('reading','{\"request_id\":\"retained\"}');
        UPDATE metadata SET revision=19;",
    )
    .unwrap();
    db
}

fn version(db: &Connection) -> i32 {
    db.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

#[test]
fn format_46_adds_empty_level_receipts_preserving_history_and_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.sqlite3");
    let db = baseline(&path, true);
    db.pragma_update(None, "user_version", 46).unwrap();
    let awards = rows(&db, "effort_awards");
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(version(&store.connection), SCHEMA_VERSION);
    assert_eq!(rows(&store.connection, "effort_awards"), awards);
    assert!(rows(&store.connection, "skill_level_events").is_empty());
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert!(rows(&reopened.connection, "skill_level_events").is_empty());
    assert_eq!(
        std::fs::read_dir(dir.path().join("migration-backups"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn level_receipt_migration_rolls_back_on_final_validation_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("db"), true);
    db.pragma_update(None, "user_version", 46).unwrap();
    let before = rows(&db, "effort_awards");
    assert!(
        run_chain(&mut db, 46, 47, &STEPS[..2], |_| Err(AppError::new(
            ErrorCode::Storage,
            "Fixture validation failure."
        )))
        .is_err()
    );
    assert_eq!(version(&db), 46);
    assert!(db.prepare("SELECT * FROM skill_level_events").is_err());
    assert_eq!(rows(&db, "effort_awards"), before);
}

fn rows(db: &Connection, table: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut statement = db
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let count = statement.column_count();
    statement
        .query_map([], |row| (0..count).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn baseline_upgrade_preserves_history_settings_and_has_a_recovery_copy() {
    for extras in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.sqlite3");
        let db = baseline(&path, extras);
        if extras {
            db.execute_batch(
                "UPDATE voice_playback SET rate=1.5;
                UPDATE microphone_selection SET device='Chosen microphone';
                INSERT INTO skill_choices VALUES('spanish',3,NULL,'[]');
                INSERT INTO inference_executions(id,task,state,metadata)
                VALUES('execution','speech','succeeded','{\"request_id\":\"request\"}');",
            )
            .unwrap();
        }
        let mut tables = vec![
            "learner",
            "saved_topics",
            "effort_awards",
            "reading_attempts",
        ];
        if extras {
            tables.extend(["voice_playback", "microphone_selection", "skill_choices"]);
        }
        let original: Vec<_> = tables.iter().map(|table| rows(&db, table)).collect();
        drop(db);
        let store = Store::open(&path).unwrap();
        assert_eq!(version(&store.connection), SCHEMA_VERSION);
        assert_eq!(store.snapshot().unwrap().learner.name, "Retained learner");
        for (table, expected) in tables.iter().zip(&original) {
            let mut actual = rows(&store.connection, table);
            if *table == "learner" {
                if let rusqlite::types::Value::Text(raw) = &mut actual[0][4] {
                    let mut value: serde_json::Value = serde_json::from_str(raw).unwrap();
                    assert_eq!(
                        value.as_object_mut().unwrap().remove("execution").unwrap(),
                        // Format 47 captured the then-current policy. Later fresh
                        // defaults must not rewrite saved learner preferences.
                        serde_json::json!({"assessment":"automatic", "replyBrief":"on_demand", "reading":"on_demand"})
                    );
                    *raw = value.to_string();
                } else {
                    panic!("preferences must remain JSON text");
                }
            }
            assert_eq!(&actual, expected, "{table}");
        }
        let backups: Vec<_> = std::fs::read_dir(dir.path().join("migration-backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(backups.len(), 1);
        let copy = Connection::open(&backups[0]).unwrap();
        assert_eq!(version(&copy), 45);
        for (table, expected) in tables.iter().zip(&original) {
            assert_eq!(&rows(&copy, table), expected);
        }
        drop(store);
        drop(Store::open(&path).unwrap());
        assert_eq!(
            std::fs::read_dir(dir.path().join("migration-backups"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn fresh_and_migrated_schemas_are_identical() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.sqlite3");
    drop(baseline(&old, true));
    let upgraded = Store::open(&old).unwrap();
    let fresh = Store::open(&dir.path().join("fresh.sqlite3")).unwrap();
    let objects = |db: &Connection| {
        db.prepare("SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?.replace("\r\n", "\n"),
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(objects(&upgraded.connection), objects(&fresh.connection));
}

#[test]
fn invalid_sources_and_backup_failure_leave_the_original_untouched() {
    for damage in [
        "PRAGMA user_version=44",
        "PRAGMA user_version=999",
        "PRAGMA application_id=42",
        "DROP TRIGGER revision_link_update",
        "ALTER TABLE reward_settings ADD COLUMN unrecognized TEXT",
        "INSERT INTO contacts VALUES('bad','missing','missing',0,1)",
        "",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.sqlite3");
        let db = baseline(&path, true);
        db.pragma_update(None, "foreign_keys", false).unwrap();
        db.execute_batch(damage).unwrap();
        if damage.is_empty() {
            std::fs::write(dir.path().join("migration-backups"), b"blocked").unwrap();
        }
        let old_version = version(&db);
        let expected = rows(&db, "learner");
        drop(db);
        assert!(Store::open(&path).is_err(), "{damage}");
        let db = Connection::open(&path).unwrap();
        assert_eq!(version(&db), old_version);
        assert_eq!(rows(&db, "learner"), expected);
    }
}

#[test]
fn invalid_product_json_rolls_back_before_startup_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.sqlite3");
    let db = baseline(&path, false);
    db.execute("UPDATE learner SET preferences='{}'", [])
        .unwrap();
    drop(db);
    let Err(error) = Store::open(&path) else {
        panic!("invalid preferences accepted")
    };
    assert_eq!(error.diagnostics.unwrap()["stage"], "validate_final");
    let db = Connection::open(&path).unwrap();
    assert_eq!(version(&db), 45);
    assert!(db.prepare("SELECT * FROM skill_choices").is_err());
}

fn add_column(db: &Connection) -> Result<()> {
    db.execute_batch("ALTER TABLE saved_topics ADD COLUMN extra TEXT;")?;
    Ok(())
}
fn second_step(db: &Connection) -> Result<()> {
    db.execute_batch("UPDATE saved_topics SET extra='filled';")?;
    Ok(())
}
fn noop(_: &Connection) -> Result<()> {
    Ok(())
}
fn fail(db: &Connection) -> Result<()> {
    db.execute_batch("UPDATE saved_topics SET extra='partial'; SELECT * FROM missing_table;")?;
    Ok(())
}

#[test]
fn multiple_steps_resume_from_each_version_without_replaying_committed_steps() {
    let steps = [
        Step {
            from: 45,
            apply: add_column,
            validate: noop,
        },
        Step {
            from: 46,
            apply: second_step,
            validate: noop,
        },
    ];
    for start in [45, 46] {
        let dir = tempfile::tempdir().unwrap();
        let mut db = baseline(&dir.path().join("db"), false);
        if start == 46 {
            add_column(&db).unwrap();
            db.pragma_update(None, "user_version", 46).unwrap();
        }
        run_chain(&mut db, start, 47, &steps, noop).unwrap();
        assert_eq!(version(&db), 47);
        assert_eq!(
            db.query_row("SELECT extra FROM saved_topics", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "filled"
        );
    }
}

#[test]
fn later_step_failure_rolls_back_ddl_data_and_version_with_useful_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("db"), false);
    let steps = [
        Step {
            from: 45,
            apply: add_column,
            validate: noop,
        },
        Step {
            from: 46,
            apply: fail,
            validate: noop,
        },
    ];
    let error = run_chain(&mut db, 45, 47, &steps, noop).unwrap_err();
    assert_eq!(version(&db), 45);
    assert!(db.prepare("SELECT extra FROM saved_topics").is_err());
    let details = error.diagnostics.unwrap();
    assert_eq!(details["step_from"], 46);
    assert!(details["cause"]["diagnostics"].is_object());
    assert!(!details.to_string().contains("保留"));
}

#[test]
fn registry_rejects_gaps_duplicates_and_version_bumps_without_a_step() {
    assert!(check_chain(STEPS, SCHEMA_VERSION).is_ok());
    assert!(check_chain(STEPS, SCHEMA_VERSION + 1).is_err());
    assert!(
        check_chain(
            &[Step {
                from: 46,
                apply: noop,
                validate: noop
            }],
            46
        )
        .is_err()
    );
    assert!(
        check_chain(
            &[
                Step {
                    from: 45,
                    apply: noop,
                    validate: noop
                },
                Step {
                    from: 45,
                    apply: noop,
                    validate: noop
                }
            ],
            47
        )
        .is_err()
    );
}

#[test]
fn recovery_copy_contains_committed_wal_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let db = baseline(&path, true);
    db.pragma_update(None, "journal_mode", "WAL").unwrap();
    db.execute("UPDATE learner SET name='Committed WAL value'", [])
        .unwrap();
    let copy = recovery_copy::create(&db, &path, 45, 46).unwrap();
    let copy = Connection::open(copy).unwrap();
    assert_eq!(rows(&db, "learner"), rows(&copy, "learner"));
}

#[test]
fn upgrade_preserves_conversation_graph_and_allows_continued_workspace_use() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    let conversation = store.snapshot().unwrap().conversations[0].id.clone();
    store
        .set_hosted_connection(1, Some("fixture-credential"), "fixture@example.invalid")
        .unwrap();
    let revision = store.snapshot().unwrap().conversations[0].revision;
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action: Action::SendMessage {
                input: crate::learning::coaching::InputEvidence::default(),
                conversation_id: conversation.clone(),
                text: "Preserved original text".into(),
                expected_revision: revision,
            },
        })
        .unwrap();
    // This fixture represents a completed conversation before its schema upgrade.
    store
        .connection
        .execute("UPDATE turns SET state='succeeded'", [])
        .unwrap();
    let tables = [
        "personas",
        "contacts",
        "conversations",
        "conversation_settings",
        "turns",
        "messages",
    ];
    let original: Vec<_> = tables
        .iter()
        .map(|table| rows(&store.connection, table))
        .collect();
    // Reconstruct the supported source format; these retained tables still use
    // their version-45 contracts, while milestone receipts were introduced later.
    store
        .connection
        .execute_batch("DROP TABLE graph_helper_requests; DROP TRIGGER graph_engine_helper_owners; DROP TABLE workspace_transcription_cache; DROP TABLE workspace_graph_consumers; DROP VIEW native_graph_audio_cache; DROP VIEW native_graph_audio_receipts; DROP TABLE workspace_graph_audio_cache; DROP TABLE workspace_graph_audio_receipts; DROP TABLE workspace_reading_cache; DROP TABLE workspace_graph_runs; DROP TABLE workspace_graph_transport_identities; DROP TABLE workspace_graph_archives; DROP TABLE workspace_graph_records; DROP TABLE workspace_graph_engines; DROP VIEW graph_conversation_runs; DROP TABLE graph_speech_requests; DROP TRIGGER graph_engine_speech_owners; DROP TABLE graph_audio_deliveries; DROP TABLE graph_audio_cache; DROP TABLE graph_audio_receipts; DROP TABLE conversation_graph_disclosures; DROP TABLE conversation_graph_assessments; DROP TABLE conversation_graph_reply_sources; DROP TRIGGER conversation_graph_publication_reserved; DROP TABLE graph_transport_identities; DROP TABLE conversation_graph_publications; DROP TABLE conversation_graph_effects; DROP TRIGGER turn_execution_conversation_fixed; DROP TABLE turn_execution_owners; DROP TABLE graph_archives; DROP TABLE graph_records; DROP TABLE graph_engines; DROP TABLE reading_dictionary; DROP TABLE reading_packages; DROP TABLE skill_level_events; UPDATE learner SET preferences=json_remove(preferences,'$.execution');")
        .unwrap();
    store.connection.execute_batch(OPTIONAL_BASELINE).unwrap();
    store
        .connection
        .pragma_update(None, "user_version", 45)
        .unwrap();
    upgrade(&mut store.connection, &path, &store.config).unwrap();
    for (table, expected) in tables.iter().zip(original) {
        let actual = rows(&store.connection, table);
        assert_eq!(actual, expected, "{table}");
    }
    drop(store);
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    assert_eq!(store.snapshot().unwrap().conversations.len(), 1);
    let revision = store.snapshot().unwrap().conversations[0].revision;
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action: Action::SendMessage {
                conversation_id: conversation.clone(),
                text: "Current inquiry".into(),
                input: crate::learning::coaching::InputEvidence::default(),
                expected_revision: revision,
            },
        })
        .unwrap();
    assert!(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .messages
            .iter()
            .any(|message| message.text == "Current inquiry")
    );
}

#[test]
fn step_validation_failure_rolls_back_and_retry_can_succeed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut db = baseline(&path, false);
    let steps = [Step {
        from: 45,
        apply: add_column,
        validate: fail,
    }];
    let error = run_chain(&mut db, 45, 46, &steps, noop).unwrap_err();
    assert_eq!(error.diagnostics.unwrap()["stage"], "validate_step");
    drop(db);
    let mut db = Connection::open(&path).unwrap();
    assert_eq!(version(&db), 45);
    assert!(db.prepare("SELECT extra FROM saved_topics").is_err());
    let steps = [Step {
        from: 45,
        apply: add_column,
        validate: noop,
    }];
    run_chain(&mut db, 45, 46, &steps, noop).unwrap();
    assert_eq!(version(&db), 46);
}

#[test]
fn requested_speech_upgrade_preserves_records_and_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("speech-requests"), true);
    run_chain(
        &mut db,
        45,
        66,
        &STEPS[..21],
        v65_graph_audio_delivery::validate,
    )
    .unwrap();
    let before = rows(&db, "messages");
    assert!(
        run_chain(&mut db, 66, 67, &STEPS[..22], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 66);
    assert!(db.prepare("SELECT * FROM graph_speech_requests").is_err());
    run_chain(
        &mut db,
        66,
        67,
        &STEPS[..22],
        v67_graph_speech_requests::validate,
    )
    .unwrap();
    assert_eq!(before, rows(&db, "messages"));
    assert!(rows(&db, "graph_speech_requests").is_empty());
    assert_eq!(
        include_str!("v67_graph_speech_requests.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/graph_speech_requests.sql").replace("\r\n", "\n")
    );
}

#[test]
fn workspace_graph_upgrade_is_additive_and_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("workspace-graphs"), true);
    run_chain(
        &mut db,
        45,
        67,
        &STEPS[..22],
        v67_graph_speech_requests::validate,
    )
    .unwrap();
    let messages = rows(&db, "messages");
    assert!(
        run_chain(&mut db, 67, 68, &STEPS[..23], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 67);
    assert!(db.prepare("SELECT * FROM workspace_graph_engines").is_err());
    run_chain(
        &mut db,
        67,
        68,
        &STEPS[..23],
        v68_workspace_graphs::validate,
    )
    .unwrap();
    assert_eq!(messages, rows(&db, "messages"));
    assert!(rows(&db, "workspace_graph_engines").is_empty());
    assert_eq!(
        include_str!("v68_workspace_graphs.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/workspace_graph_runtime.sql").replace("\r\n", "\n")
    );
}

#[test]
fn helper_owners_upgrade_atomically_without_changing_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("helper-owners"), true);
    run_chain(
        &mut db,
        45,
        68,
        &STEPS[..23],
        v68_workspace_graphs::validate,
    )
    .unwrap();
    let messages = rows(&db, "messages");
    assert!(
        run_chain(&mut db, 68, 69, &STEPS[..24], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 68);
    assert!(db.prepare("SELECT * FROM graph_helper_requests").is_err());
    v68_workspace_graphs::validate(&db).unwrap();
    run_chain(
        &mut db,
        68,
        69,
        &STEPS[..24],
        v69_graph_helper_requests::validate,
    )
    .unwrap();
    assert_eq!(messages, rows(&db, "messages"));
    assert!(rows(&db, "graph_helper_requests").is_empty());
    assert_eq!(
        include_str!("v69_graph_helper_requests.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/graph_helper_requests.sql").replace("\r\n", "\n")
    );
}

#[test]
fn cache_ownership_upgrade_is_atomic_and_preserves_graph_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("cache-ownership"), true);
    run_chain(
        &mut db,
        45,
        70,
        &STEPS[..25],
        v70_graph_assessment_owners::validate,
    )
    .unwrap();
    db.execute(
        "INSERT INTO workspace_graph_engines VALUES('engine','learner',?1,'{}',X'00')",
        ["a".repeat(64)],
    )
    .unwrap();
    db.execute_batch(
        "INSERT INTO workspace_graph_runs VALUES('run','engine','artifact','reading','{}');
        INSERT INTO inference_blobs VALUES('shared',X'010203');
        INSERT INTO workspace_reading_cache VALUES('run','source','shared',1);
        INSERT INTO inference_executions(id,task,state) VALUES('receipt','reading','succeeded');
        INSERT INTO inference_results VALUES('receipt','source','shared',0);
        INSERT INTO inference_consumers VALUES('consumer','receipt');",
    )
    .unwrap();
    let tables = [
        "workspace_graph_engines",
        "workspace_graph_runs",
        "workspace_reading_cache",
        "inference_blobs",
        "inference_cache_settings",
        "effort_awards",
    ];
    let original: Vec<_> = tables.iter().map(|table| rows(&db, table)).collect();
    assert!(
        run_chain(&mut db, 70, 71, &STEPS[..26], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 70);
    assert_eq!(rows(&db, "inference_results").len(), 1);
    run_chain(&mut db, 70, 71, &STEPS[..26], v71_cache::validate).unwrap();
    for (table, expected) in tables.iter().zip(original) {
        assert_eq!(rows(&db, table), expected, "{table}");
    }
    assert!(db.prepare("SELECT * FROM inference_executions").is_err());
    v71_cache::validate(&db).unwrap();
}

#[test]
fn motion_format_preserves_old_preferences_and_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("motion");
    let mut db = baseline(&path, true);
    run_chain(&mut db, 45, 72, &STEPS[..27], v72_streaks::validate).unwrap();
    let before = rows(&db, "learner");
    assert!(
        run_chain(&mut db, 72, 73, &STEPS[..28], |_| Err(AppError::new(
            ErrorCode::Storage,
            "Injected failure."
        )))
        .is_err()
    );
    assert_eq!(version(&db), 72);
    assert_eq!(rows(&db, "learner"), before);
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(version(&store.connection), 73);
    assert_eq!(rows(&store.connection, "learner"), before);
    assert_eq!(
        store
            .snapshot()
            .unwrap()
            .learner
            .preferences
            .appearance
            .motion,
        crate::configuration::appearance::MotionPreference::System
    );
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(rows(&store.connection, "learner"), before);
}
