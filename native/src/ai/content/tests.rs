use super::*;
use std::sync::{Arc, Mutex};
fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE graph_engines(id TEXT PRIMARY KEY); CREATE TABLE workspace_graph_engines(id TEXT PRIMARY KEY); CREATE TABLE transcription_attempts(id TEXT PRIMARY KEY); CREATE TABLE workspace_graph_runs(run_id TEXT PRIMARY KEY); INSERT INTO graph_engines VALUES('engine'); INSERT INTO transcription_attempts VALUES('recording');").unwrap();
    crate::ai::results::initialize(&db).unwrap();
    db
}
fn owner() -> Owner {
    Owner::Execution {
        engine: "engine".into(),
        execution: "42".into(),
    }
}
fn save(db: &Connection, id: &str, bytes: &[u8]) {
    write(
        db,
        &owner(),
        Write::Begin {
            id: id.into(),
            kind: "text.response".into(),
            media_type: "application/json".into(),
        },
    )
    .unwrap();
    write(
        db,
        &owner(),
        Write::Append {
            id: id.into(),
            bytes: bytes.into(),
        },
    )
    .unwrap();
    write(
        db,
        &owner(),
        Write::Finish {
            id: id.into(),
            complete: true,
            digest: crate::ai::results::digest(bytes),
            credentials_removed: false,
        },
    )
    .unwrap();
}
#[test]
fn original_bytes_survive_detached_read_and_eviction_leaves_provenance() {
    let db = database();
    let source = " {\"text\":\"保留 é\"} ";
    save(&db, "a", source.as_bytes());
    let prepared = prepare(&db, "a").unwrap();
    crate::ai::results::set_capacity(&db, 0).unwrap();
    assert_eq!(prepared.finish().unwrap().text.as_deref(), Some(source));
    let evicted = read(&db, "a").unwrap();
    assert_eq!(evicted.record.availability, "evicted");
    assert!(evicted.text.is_none());
    assert_eq!(
        evicted.record.digest,
        Some(crate::ai::results::digest(source.as_bytes()))
    );
    assert_eq!(for_execution(&db, "engine", "42").unwrap().len(), 1);
}
#[test]
fn interrupted_capture_is_explicit_and_corrupt_bytes_fail() {
    let db = database();
    write(
        &db,
        &owner(),
        Write::Begin {
            id: "a".into(),
            kind: "response".into(),
            media_type: "text/plain".into(),
        },
    )
    .unwrap();
    write(
        &db,
        &owner(),
        Write::Append {
            id: "a".into(),
            bytes: b"partial".to_vec(),
        },
    )
    .unwrap();
    crate::ai::results::recover(&db).unwrap();
    assert_eq!(read(&db, "a").unwrap().record.state, "interrupted");
    db.execute("UPDATE inference_blobs SET payload=x'00'", [])
        .unwrap();
    assert!(read(&db, "a").is_err());
}
#[tokio::test]
async fn tee_preserves_content_and_masks_only_actual_credentials_across_chunks() {
    let db = Arc::new(Mutex::new(database()));
    let owned = db.clone();
    let writer: capture::Writer =
        Arc::new(move |event| write(&owned.lock().unwrap(), &owner(), event));
    capture::scope(writer, async {
        capture::request(
            "text",
            "application/json",
            b"prompt transcript credential words",
            "secret-token",
        )
        .unwrap();
        capture::chunk(b"raw secret-").unwrap();
        capture::chunk(b"token result").unwrap();
        capture::complete().unwrap();
    })
    .await;
    let db = db.lock().unwrap();
    let rows = for_execution(&db, "engine", "42").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        read(&db, &rows[0].id).unwrap().text.as_deref(),
        Some("prompt transcript credential words")
    );
    let response = read(&db, &rows[1].id).unwrap();
    assert_eq!(
        response.text.as_deref(),
        Some("raw <credential removed> result")
    );
    assert!(response.record.credentials_removed);
    assert_eq!(response.record.state, "complete");
}
#[tokio::test]
async fn failed_scope_preserves_partial_response_and_recording_is_idempotent() {
    let db = Arc::new(Mutex::new(database()));
    let owned = db.clone();
    let writer: capture::Writer =
        Arc::new(move |event| write(&owned.lock().unwrap(), &owner(), event));
    capture::scope(writer, async {
        capture::request("text", "text/plain", b"request", "").unwrap();
        capture::chunk(b"unfinished").unwrap();
    })
    .await;
    let db = db.lock().unwrap();
    let rows = for_execution(&db, "engine", "42").unwrap();
    let result = read(&db, &rows[1].id).unwrap();
    assert_eq!(result.record.state, "partial");
    assert_eq!(result.text.as_deref(), Some("unfinished"));
    save_recording(&db, "recording", b"WAV bytes").unwrap();
    save_recording(&db, "recording", b"WAV bytes").unwrap();
    assert!(save_recording(&db, "recording", b"different").is_err());
    assert_eq!(for_recording(&db, "recording").unwrap().len(), 1);
}

#[test]
fn contributing_recordings_survive_message_acceptance_and_restart() {
    use crate::model::{Action, Command};
    use crate::storage::store::Store;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.sqlite3");
    let mut store = Store::open(&path).unwrap();
    store.prepare_chat().unwrap();
    store
        .set_hosted_connection(1, Some("fixture-credential"), "fixture@example.invalid")
        .unwrap();
    let conversation = store.snapshot().unwrap().conversations[0].clone();
    let sources = vec!["first-take".to_owned(), "second-take".to_owned()];
    for source in &sources {
        store.connection.execute("INSERT INTO transcription_attempts(id,conversation_id,route,model,profile_revision,state) VALUES(?1,?2,'hosted','fixture',1,'succeeded')",params![source,conversation.id]).unwrap();
        save_recording(&store.connection, source, b"original WAV source").unwrap();
    }
    crate::speech::recording::transcription::validate_sources(
        &store.connection,
        &conversation.id,
        &sources,
    )
    .unwrap();
    assert!(
        crate::speech::recording::transcription::validate_sources(
            &store.connection,
            "another-conversation",
            &sources
        )
        .is_err()
    );
    assert!(
        crate::speech::recording::transcription::validate_sources(
            &store.connection,
            &conversation.id,
            &[sources[0].clone(), sources[0].clone()]
        )
        .is_err()
    );
    let receipt = store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: uuid::Uuid::new_v4().to_string(),
            action: Action::SendMessage {
                conversation_id: conversation.id.clone(),
                text: "Edited combined transcript".into(),
                expected_revision: conversation.revision,
                input: crate::learning::coaching::InputEvidence {
                    modality: "speech_transcript".into(),
                    recording_ids: Some(sources.clone()),
                    ..Default::default()
                },
            },
        })
        .unwrap();
    let turn = receipt.entity_id;
    drop(store);
    let store = Store::open(&path).unwrap();
    let view = store.conversation_snapshot(&conversation.id, None).unwrap();
    assert_eq!(
        view.turns
            .iter()
            .find(|item| item.id == turn)
            .unwrap()
            .recording_ids
            .as_ref(),
        Some(&sources)
    );
    assert_eq!(
        for_recording(&store.connection, &sources[0]).unwrap().len(),
        1
    );
}

pub(crate) fn capture_events() -> (capture::Writer, Arc<Mutex<Vec<Write>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    (
        Arc::new(move |event| {
            sink.lock().unwrap().push(event);
            Ok(())
        }),
        events,
    )
}
pub(crate) fn captured(events: &Mutex<Vec<Write>>, kind: &str) -> Vec<u8> {
    let events = events.lock().unwrap();
    let id = events
        .iter()
        .find_map(|event| match event {
            Write::Begin {
                id, kind: found, ..
            } if found == kind => Some(id),
            _ => None,
        })
        .expect("original record");
    assert!(
        events
            .iter()
            .any(|event| matches!(event,Write::Finish{id:found,complete:true,..} if found==id))
    );
    events
        .iter()
        .filter_map(|event| match event {
            Write::Append { id: found, bytes } if found == id => Some(bytes.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}
