use super::*;

#[test]
fn every_bundled_entry_is_available_without_provider_execution() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("workspace")).unwrap();
    for (_, json) in BUNDLED {
        let package: Package = serde_json::from_str(json).unwrap();
        for entry in package.entries {
            let query = SavedGlossQuery {
                scope: entry.scope,
                surfaces: vec![entry.text],
            };
            let found = sources(&store, &query).unwrap();
            let source = found
                .iter()
                .find(|s| {
                    s.source_id
                        == format!("dictionary/{}/{}/{}", package.id, package.version, entry.id)
                })
                .unwrap();
            assert_eq!(
                source.segments[0].gloss.as_deref(),
                Some(entry.gloss.as_str())
            );
            assert_eq!(source.dictionary, Some(true));
            assert_eq!(source.provenance.as_ref().unwrap().sources, entry.sources);
            assert!(source.operation_id.is_none() && source.attempt_id.is_none());
        }
    }
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
}

#[test]
fn lookup_preserves_unicode_and_variety_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("workspace")).unwrap();
    let mut package: serde_json::Value = serde_json::from_str(&fixture("unicode")).unwrap();
    package["entries"][0]["text"] = "بَيْت".into();
    package["entries"][0]["scope"] = serde_json::json!({"language":"arabic","variety":"arabic-levantine","explanation":"english","explanationVariety":"english-united-states"});
    install(&mut store.connection, &store.config, &package.to_string()).unwrap();
    let mut query = SavedGlossQuery {
        scope: serde_json::from_value(package["entries"][0]["scope"].clone()).unwrap(),
        surfaces: vec!["بَيْت".into()],
    };
    let found = sources(&store, &query).unwrap();
    let source = found
        .iter()
        .find(|s| s.source_id.contains("/test/"))
        .unwrap();
    assert_eq!(source.text, "بَيْت");
    assert_eq!(source.segments[0].end, "بَيْت".encode_utf16().count() as u32);
    query.surfaces = vec!["بيت".into()];
    assert!(
        !sources(&store, &query)
            .unwrap()
            .iter()
            .any(|s| s.source_id.contains("/test/"))
    );
    query.surfaces = vec!["بَيْت".into()];
    query.scope.variety = Some("arabic-modern-standard".into());
    assert!(
        !sources(&store, &query)
            .unwrap()
            .iter()
            .any(|s| s.source_id.contains("/test/"))
    );
}

fn fixture(version: &str) -> String {
    serde_json::json!({
        "schemaVersion":1,"id":"test","version":version,"license":"original test fixture",
        "review":"source_checked","sources":["https://example.org/fixture"],
        "entries":[{"id":"water","scope":{"language":"english","variety":"english-united-states",
        "explanation":"spanish","explanationVariety":"spanish-spain"},"text":"water","gloss":"agua",
        "romanization":null,"pronunciation":null,"sense":"drinking liquid","sources":["https://example.org/fixture"]}]
    }).to_string()
}

#[test]
fn imports_are_atomic_idempotent_and_do_not_impersonate_inference() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let count = |db: &Connection| {
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    let before = count(&store.connection);
    install(&mut store.connection, &store.config, &fixture("1")).unwrap();
    let changes = store.connection.total_changes();
    install(&mut store.connection, &store.config, &fixture("1")).unwrap();
    assert_eq!(store.connection.total_changes(), changes);
    assert_eq!(count(&store.connection), before);
    let query = SavedGlossQuery {
        scope: ReadingScope {
            language: "english".into(),
            variety: Some("english-united-states".into()),
            explanation: "spanish".into(),
            explanation_variety: Some("spanish-spain".into()),
        },
        surfaces: vec!["water".into()],
    };
    let found = sources(&store, &query).unwrap();
    assert!(
        found
            .iter()
            .any(|s| s.source_id == "dictionary/test/1/water"
                && s.operation_id.is_none()
                && s.dictionary == Some(true))
    );
    // A failure after deleting old rows must roll back both manifest and records.
    store.connection.execute_batch("CREATE TEMP TRIGGER reject_preload BEFORE INSERT ON reading_dictionary WHEN NEW.package_id='test' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    assert!(install(&mut store.connection, &store.config, &fixture("2")).is_err());
    assert!(
        sources(&store, &query)
            .unwrap()
            .iter()
            .any(|s| s.source_id == "dictionary/test/1/water")
    );
    store
        .connection
        .execute_batch("DROP TRIGGER reject_preload;")
        .unwrap();
    install(&mut store.connection, &store.config, &fixture("2")).unwrap();
    assert!(
        sources(&store, &query)
            .unwrap()
            .iter()
            .any(|s| s.source_id == "dictionary/test/2/water")
    );
    assert!(
        install(
            &mut store.connection,
            &store.config,
            &fixture("2").replace("agua", "AGUA")
        )
        .is_err()
    );
    // Imports are outside ordinary inference cache pruning.
    crate::ai::results::set_capacity(&store.connection, 0).unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    assert!(
        sources(&store, &query)
            .unwrap()
            .iter()
            .any(|s| s.source_id == "dictionary/test/2/water")
    );
    let wrong_case = SavedGlossQuery {
        surfaces: vec!["Water".into()],
        ..query
    };
    assert!(
        !sources(&store, &wrong_case)
            .unwrap()
            .iter()
            .any(|s| s.source_id.contains("/test/"))
    );
}

#[test]
fn malformed_packages_do_not_replace_installed_content() {
    let config = Registry::bundled().unwrap();
    let mut db = Connection::open_in_memory().unwrap();
    db.execute_batch(include_str!(
        "../../../storage/schemas/reading_preloads.sql"
    ))
    .unwrap();
    install(&mut db, &config, &fixture("1")).unwrap();
    for bad in [
        fixture("2").replace("source_checked", "draft"),
        fixture("2").replace("spanish-spain", "invalid-variety"),
        fixture("2").replace("\"text\":\"water\"", "\"text\":\"\""),
    ] {
        assert!(install(&mut db, &config, &bad).is_err());
    }
    assert_eq!(
        db.query_row(
            "SELECT version FROM reading_packages WHERE id='test'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "1"
    );
}

#[test]
fn large_dictionary_uses_scope_surface_index() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(include_str!(
        "../../../storage/schemas/reading_preloads.sql"
    ))
    .unwrap();
    db.execute_batch("INSERT INTO reading_packages VALUES('large','1','digest','fixture','source_checked');
        WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<420000)
        INSERT INTO reading_dictionary SELECT 'large',CAST(x AS TEXT),'source','variety','explanation','variety','word-'||x,'{}' FROM n;").unwrap();
    let sql = "SELECT d.payload FROM reading_dictionary d JOIN reading_packages p ON p.id=d.package_id WHERE d.language='source' AND d.variety='variety' AND d.explanation='explanation' AND d.explanation_variety='variety' AND d.surface IN (SELECT value FROM json_each('[\"word-420000\"]'))";
    let plan = db
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
        .join("\n");
    assert!(plan.contains("reading_dictionary_scope_surface"), "{plan}");
    assert!(!plan.contains("SCAN d"), "{plan}");
    assert_eq!(
        db.query_row(sql, [], |r| r.get::<_, String>(0)).unwrap(),
        "{}"
    );
}
