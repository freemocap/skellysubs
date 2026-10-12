use super::*;
use crate::{
    ai::{connections::access::ResolvedTarget, graph::Contract},
    language::source_graph::SourceText,
    speech::{alignment::CharacterAlignment, synthesis_graph::Settings},
};

fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE conversations(id TEXT PRIMARY KEY); INSERT INTO conversations VALUES('conversation');").unwrap();
    db.execute_batch(include_str!("../../storage/schemas/graph_runtime.sql"))
        .unwrap();
    db.execute_batch(
        "CREATE TABLE learner(id TEXT PRIMARY KEY); INSERT INTO learner VALUES('workspace');",
    )
    .unwrap();
    db.execute_batch(include_str!(
        "../../storage/schemas/workspace_graph_runtime.sql"
    ))
    .unwrap();
    results::initialize(&db).unwrap();
    db.execute(
        "INSERT INTO workspace_graph_engines VALUES('workspace-engine','workspace',?1,'{}',X'00')",
        ["b".repeat(64)],
    )
    .unwrap();
    db.execute(
        "INSERT INTO graph_engines VALUES('engine','conversation',?1,'{}',X'00')",
        ["a".repeat(64)],
    )
    .unwrap();
    db
}
fn request() -> Request {
    Request {
        source: SourceText {
            id: "message".into(),
            text: "cafe\u{301} 日本語".into(),
        },
        settings: Settings {
            target: ResolvedTarget {
                audio_resolution: None,
                route: crate::model::ConnectionRoute::Custom,
                revision: 1,
                url: "http://127.0.0.1:8765/v1/audio/speech".into(),
                model: "captured".into(),
                credential: Some("reference".into()),
            },
            install_id: "install".into(),
            language_tag: "es".into(),
            language: "Spanish".into(),
            voice: "voice".into(),
        },
    }
}
fn identity(execution: u64) -> InvocationIdentity {
    InvocationIdentity {
        engine: Some("engine".into()),
        execution: serde_json::from_value(serde_json::json!(execution)).unwrap(),
        artifact: "artifact".into(),
        operation: Contract::new("speech.synthesize", 1),
    }
}
fn alignment() -> SpeechAlignment {
    let lane = CharacterAlignment {
        characters: vec![request().source.text],
        starts: vec![0.0],
        ends: vec![0.1],
    };
    SpeechAlignment {
        source_text: request().source.text,
        original: Some(lane.clone()),
        normalized: Some(lane),
    }
}
#[test]
fn native_audio_receipts_survive_eviction_without_legacy_executions_or_resynthesis() {
    let mut db = database();
    let receipt = {
        let tx = db.transaction().unwrap();
        let receipt = retain(&tx, &identity(1), &request(), &wav(), Some(&alignment())).unwrap();
        verify(&tx, &receipt).unwrap();
        let cached = lookup(&tx, &request()).unwrap().unwrap();
        assert_eq!(cached.wav, wav());
        assert_eq!(cached.alignment, Some(alignment()));
        assert_eq!(cached.receipt.id, receipt.id);
        tx.commit().unwrap();
        receipt
    };
    assert_eq!(results::settings(&db).unwrap().result_count, 1);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    results::set_capacity(&db, 0).unwrap();
    results::recover(&db).unwrap();
    let tx = db.transaction().unwrap();
    verify(&tx, &receipt).unwrap();
    assert!(lookup(&tx, &request()).unwrap().is_none());
    assert_eq!(
        tx.query_row(
            "SELECT alignment FROM graph_audio_receipts WHERE id=?1",
            [&receipt.id],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        serde_json::to_string(&alignment()).unwrap()
    );
    assert_eq!(results::settings(&tx).unwrap().used_bytes, 0);
}
#[test]
fn native_audio_rolls_back_and_rejects_mismatch_corruption_and_duplicate_producers() {
    let mut db = database();
    let expected = reference(&identity(1), &wav()).unwrap();
    {
        let tx = db.transaction().unwrap();
        retain(&tx, &identity(1), &request(), &wav(), None).unwrap();
        assert!(retain(&tx, &identity(1), &request(), &wav(), None).is_err());
    }
    let tx = db.transaction().unwrap();
    assert!(verify(&tx, &expected).is_err());
    let mut wrong = alignment();
    wrong.source_text.push('!');
    assert!(retain(&tx, &identity(1), &request(), &wav(), Some(&wrong)).is_err());
    let receipt = retain(&tx, &identity(1), &request(), &wav(), Some(&alignment())).unwrap();
    let mut forged = receipt.clone();
    forged.engine = "foreign".into();
    assert!(verify(&tx, &forged).is_err());
    assert!(
        tx.execute("UPDATE graph_audio_receipts SET audio_bytes=3", [])
            .is_err()
    );
    tx.execute("UPDATE inference_blobs SET payload=X'010203'", [])
        .unwrap();
    assert!(read(&tx, &receipt.id).is_err());
}
#[test]
fn reading_and_audio_share_one_capacity_and_keep_audio_receipts() {
    let mut db = database();
    // Cache-policy fixture: its graph run is independent of the audio producer.
    db.execute("INSERT INTO workspace_graph_runs(run_id,engine_id,artifact_id,kind,context) VALUES('reading','workspace-engine','artifact','reading','{}')", []).unwrap();
    let payload = [7u8; 50];
    let digest = results::digest(&payload);
    db.execute(
        "INSERT INTO inference_blobs(digest,payload) VALUES(?1,?2)",
        params![digest, payload.as_slice()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO workspace_reading_cache VALUES('reading','source',?1,?2)",
        params![digest, results::tick(&db).unwrap()],
    )
    .unwrap();
    let receipt = {
        let tx = db.transaction().unwrap();
        let receipt = retain(&tx, &identity(1), &request(), &wav(), None).unwrap();
        tx.commit().unwrap();
        receipt
    };
    let capacity = results::settings(&db).unwrap().used_bytes - 50;
    results::set_capacity(&db, capacity).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM workspace_reading_cache", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let tx = db.transaction().unwrap();
    assert!(read(&tx, &receipt.id).unwrap().is_some());
    verify(&tx, &receipt).unwrap();
}

fn wav() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut bytes,
        hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..2400 {
        writer.write_sample(1_i16).unwrap();
    }
    writer.finalize().unwrap();
    bytes.into_inner()
}

#[test]
fn workspace_audio_uses_shared_integrity_rollback_and_eviction_rules() {
    let mut db = database();
    let mut owner = identity(1);
    owner.engine = Some("workspace-engine".into());
    let expected = reference(&owner, &wav()).unwrap();
    {
        let tx = db.transaction().unwrap();
        retain(&tx, &owner, &request(), &wav(), Some(&alignment())).unwrap();
        assert!(retain(&tx, &owner, &request(), &wav(), None).is_err());
        // Dropping the enclosing settlement rolls back receipt and cache together.
    }
    {
        let tx = db.transaction().unwrap();
        assert!(verify(&tx, &expected).is_err());
        assert!(lookup(&tx, &request()).unwrap().is_none());
        let receipt = retain(&tx, &owner, &request(), &wav(), Some(&alignment())).unwrap();
        assert_eq!(receipt.id, expected.id);
        assert_eq!(
            lookup(&tx, &request()).unwrap().unwrap().receipt.engine,
            "workspace-engine"
        );
        let mut wrong = receipt.clone();
        wrong.engine = "engine".into();
        assert!(verify(&tx, &wrong).is_err());
        assert!(
            tx.execute(
                "UPDATE workspace_graph_audio_receipts SET audio_bytes=1",
                []
            )
            .is_err()
        );
        tx.commit().unwrap();
    }
    assert_eq!(results::settings(&db).unwrap().result_count, 1);
    results::set_capacity(&db, 0).unwrap();
    results::recover(&db).unwrap();
    let tx = db.transaction().unwrap();
    verify(&tx, &expected).unwrap();
    assert!(read(&tx, &expected.id).unwrap().is_none());
    assert_eq!(results::settings(&tx).unwrap().used_bytes, 0);
    assert_eq!(
        tx.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn workspace_text_and_transcript_projections_share_lru_without_erasing_owners() {
    let db = database();
    for (run, kind, payload) in [
        ("reading", "translation", vec![1u8; 50]),
        ("recording", "transcription", vec![2u8; 50]),
    ] {
        db.execute(
            "INSERT INTO workspace_graph_runs VALUES(?1,'workspace-engine',?2,?3,'{}')",
            params![run, "c".repeat(64), kind],
        )
        .unwrap();
        let digest = results::digest(&payload);
        db.execute(
            "INSERT INTO inference_blobs VALUES(?1,?2)",
            params![digest, payload],
        )
        .unwrap();
        let table = if run == "reading" {
            "workspace_reading_cache"
        } else {
            "workspace_transcription_cache"
        };
        db.execute(
            &format!("INSERT INTO {table} VALUES(?1,'source',?2,?3)"),
            params![run, digest, results::tick(&db).unwrap()],
        )
        .unwrap();
    }
    assert_eq!(results::settings(&db).unwrap().result_count, 2);
    results::set_capacity(&db, 75).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM workspace_reading_cache", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM workspace_transcription_cache",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    results::set_capacity(&db, 0).unwrap();
    assert_eq!(results::settings(&db).unwrap().used_bytes, 0);
    assert_eq!(
        db.query_row("SELECT count(*) FROM workspace_graph_runs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert!(
        !db.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap()
    );
}
