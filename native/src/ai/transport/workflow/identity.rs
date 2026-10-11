//! Capture the conversation identity that seeds request-only persona sampling.
//! Production identity generation is unchanged; this module is capture-build only.
use super::*;

pub(crate) fn conversation_identity() -> Result<String> {
    let mut guard = locked()?;
    match guard.as_mut() {
        Some(state) => identity(state),
        None => Ok(uuid::Uuid::new_v4().to_string()),
    }
}

fn identity(state: &mut State) -> Result<String> {
    let semantic = serde_json::json!({"domain":"conversation"});
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(Kind::Identity, semantic))?)
    );
    let (recording, replay, call) = state.begin(Kind::Identity, fingerprint)?;
    let value = if let Some(record) = replay {
        match decode(&record) {
            Ok(value) => value,
            Err(_) => return Err(state.fail("replay_identity_invalid")),
        }
    } else {
        let value = uuid::Uuid::new_v4().to_string();
        if recording {
            state.push(call, serde_json::json!(value))?;
        }
        value
    };
    state.finish(call)?;
    Ok(value)
}

fn decode(record: &Record) -> Result<String> {
    if !matches!(record.kind, Kind::Identity)
        || record.cancelled_micros.is_some()
        || record.frames.len() != 1
    {
        return Err(error());
    }
    let value = record.frames[0].payload.as_str().ok_or_else(error)?;
    let uuid = uuid::Uuid::parse_str(value).map_err(|_| error())?;
    if uuid.get_version_num() != 4 || uuid.to_string() != value {
        return Err(error());
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_preserves_distinct_conversation_sampling_seeds() {
        let directory = tempfile::tempdir().unwrap();
        let mut capture = State::new(
            directory.path(),
            Mode::Capture,
            Tape {
                format: 1,
                records: vec![],
            },
        )
        .unwrap();
        // Setup before recording is part of the workspace snapshot, not its tape.
        identity(&mut capture).unwrap();
        capture.start().unwrap();
        let first = identity(&mut capture).unwrap();
        let second = identity(&mut capture).unwrap();
        assert_ne!(first, second);
        let tape = capture.stop().unwrap().unwrap();
        assert_eq!(tape.records.len(), 2);
        let mut replay = State::new(directory.path(), Mode::Replay, tape).unwrap();
        replay.start().unwrap();
        assert_eq!(identity(&mut replay).unwrap(), first);
        assert_eq!(identity(&mut replay).unwrap(), second);
        replay.stop().unwrap();
    }

    #[test]
    fn malformed_identity_invalidates_replay_without_random_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let mut capture = State::new(
            directory.path(),
            Mode::Capture,
            Tape {
                format: 1,
                records: vec![],
            },
        )
        .unwrap();
        capture.start().unwrap();
        identity(&mut capture).unwrap();
        let mut tape = capture.stop().unwrap().unwrap();
        tape.records[0].frames[0].payload = serde_json::json!("not-a-uuid");
        let mut replay = State::new(directory.path(), Mode::Replay, tape).unwrap();
        replay.start().unwrap();
        assert!(identity(&mut replay).is_err());
        assert_eq!(replay.failure_reason, Some("replay_identity_invalid"));
        assert!(replay.stop().is_err());
    }
}
