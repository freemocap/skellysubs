use super::*;
#[test]
fn speech_identity_preserves_exact_inputs_and_effective_access_scope() {
    use crate::ai::{audio::SpeechInput, connections::access::ResolvedTarget};
    use crate::model::ConnectionRoute;
    let target = ResolvedTarget {
        audio_resolution: None,
        route: ConnectionRoute::Custom,
        revision: 1,
        url: "http://localhost/v1/audio/speech".into(),
        model: "speech-model".into(),
        credential: Some("account-one".into()),
    };
    let scope = speech::scope(&target, "workspace").unwrap();
    let input = SpeechInput {
        language_tag: "en".into(),
        text: "\u{00e9}".into(),
        language: "fr".into(),
        voice: "unused".into(),
    };
    let key = speech::request_key(&scope, &input).unwrap();
    let mut tagged = input.clone();
    tagged.language_tag = "en-GB".into();
    assert_ne!(key, speech::request_key(&scope, &tagged).unwrap());
    let mut changed = input.clone();
    for text in [
        "e\u{0301}",
        "\u{00e9} ",
        "\u{0643}\u{062a}\u{0627}\u{0628}",
        "\u{4e66}",
    ] {
        changed.text = text.into();
        assert_ne!(key, speech::request_key(&scope, &changed).unwrap());
    }
    changed = input.clone();
    changed.voice = "different-unused-voice".into();
    assert_eq!(key, speech::request_key(&scope, &changed).unwrap());
    changed.language = "other-language-tag".into();
    assert_ne!(key, speech::request_key(&scope, &changed).unwrap());
    let mut changed_target = target.clone();
    changed_target.revision += 1;
    assert_eq!(scope, speech::scope(&changed_target, "workspace").unwrap());
    changed_target.model = "other-model".into();
    assert_ne!(scope, speech::scope(&changed_target, "workspace").unwrap());
    changed_target = target.clone();
    changed_target.credential = Some("account-two".into());
    assert_ne!(scope, speech::scope(&changed_target, "workspace").unwrap());
    changed_target = target.clone();
    changed_target.url = "http://localhost/other/audio/speech".into();
    assert_ne!(scope, speech::scope(&changed_target, "workspace").unwrap());
    assert_ne!(scope, speech::scope(&target, "other-workspace").unwrap());
}

#[test]
fn cache_capacity_counts_shared_blobs_once_and_evicts_least_recent_use() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE workspace_graph_runs(run_id TEXT PRIMARY KEY);")
        .unwrap();
    initialize(&db).unwrap();
    let insert = |run: &str, payload: &[u8]| {
        db.execute("INSERT INTO workspace_graph_runs VALUES(?1)", [run])
            .unwrap();
        let hash = digest(payload);
        db.execute(
            "INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)",
            rusqlite::params![hash, payload],
        )
        .unwrap();
        db.execute(
            "INSERT INTO workspace_reading_cache VALUES(?1,?1,?2,?3)",
            rusqlite::params![run, hash, tick(&db).unwrap()],
        )
        .unwrap();
    };
    insert("first", b"aaa");
    insert("second", b"bbb");
    insert("shared", b"aaa");
    assert_eq!(settings(&db).unwrap().used_bytes, 6);
    assert_eq!(settings(&db).unwrap().result_count, 3);
    db.execute(
        "UPDATE workspace_reading_cache SET last_used=?1 WHERE run_id='first'",
        [tick(&db).unwrap()],
    )
    .unwrap();
    insert("fourth", b"ccc");
    set_capacity(&db, 6).unwrap();
    let ids = db
        .prepare("SELECT run_id FROM workspace_reading_cache ORDER BY run_id")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(ids, ["first", "fourth", "shared"]);
    set_capacity(&db, 0).unwrap();
    assert_eq!(settings(&db).unwrap().used_bytes, 0);
    assert_eq!(settings(&db).unwrap().result_count, 0);
    assert!(set_capacity(&db, u64::MAX).is_err());
}
