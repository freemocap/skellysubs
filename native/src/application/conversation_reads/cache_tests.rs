use super::*;

fn version(session: &str, changes: u64) -> Version {
    Version {
        session: session.into(),
        changes,
    }
}
fn value(bytes: usize) -> Arc<Prepared> {
    Arc::new(Prepared {
        revision: 7,
        fields: " ".repeat(bytes).into(),
    })
}

#[test]
fn workspace_changes_and_session_replacement_invalidate_all_pages() {
    let mut cache = Cache::default();
    let first = version("one", 10);
    let response = value(30);
    cache.insert(&first, "chat".into(), None, response.clone());
    cache.insert(&first, "chat".into(), Some(100), value(40));
    assert!(Arc::ptr_eq(
        &response,
        &cache.get(&first, "chat", None).unwrap()
    ));
    assert!(cache.get(&first, "other", None).is_none());
    assert!(cache.get(&first, "chat", Some(99)).is_none());
    let changed = version("one", 11);
    assert!(cache.get(&changed, "chat", None).is_none());
    assert!(cache.get(&changed, "chat", Some(100)).is_none());
    cache.insert(&changed, "chat".into(), None, response);
    assert!(cache.get(&version("two", 11), "chat", None).is_none());
    assert_eq!(cache.bytes, 0);
}

#[test]
fn byte_budget_evicts_least_recent_pages_and_does_not_retain_oversized_responses() {
    let mut cache = Cache::default();
    let version = version("one", 0);
    let response = value(Cache::MAX_BYTES / 4);
    for key in ["a", "b", "c"] {
        cache.insert(&version, key.into(), None, response.clone());
    }
    assert!(cache.get(&version, "a", None).is_some());
    cache.insert(&version, "d".into(), None, response);
    assert!(cache.get(&version, "b", None).is_none());
    assert!(cache.get(&version, "a", None).is_some());
    cache.insert(&version, "large".into(), None, value(Cache::MAX_BYTES));
    assert!(cache.get(&version, "large", None).is_none());
    assert!(cache.bytes <= Cache::MAX_BYTES);
    cache.insert(&version, "a".into(), None, value(1));
    assert_eq!(
        cache.bytes,
        cache.entries.iter().map(Entry::bytes).sum::<usize>()
    );
}

#[test]
fn prepared_response_is_a_json_object_without_double_encoding() {
    use tauri::ipc::{InvokeResponseBody, IpcResponse};
    let response = Prepared {
        revision: 7,
        fields: r#"{"text":"café 𠮷"}"#.into(),
    };
    let InvokeResponseBody::Json(json) = response.response().body().unwrap() else {
        panic!("conversation must remain a JSON IPC response");
    };
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["revision"], 7);
    assert_eq!(parsed["text"], "café 𠮷");
}

#[test]
fn navigation_advances_only_current_entries_without_copying_their_content() {
    let mut cache = Cache::default();
    let old = version("one", 10);
    let new = version("one", 13);
    let response = value(100);
    cache.insert(&old, "chat".into(), None, response.clone());
    cache.navigation_committed(&old, &new, 8);
    let current = cache.get(&new, "chat", None).unwrap();
    assert_eq!(current.revision, 8);
    assert!(Arc::ptr_eq(&response.fields, &current.fields));
    cache.navigation_committed(&new, &new, 6); // Idempotent old command receipt.
    assert_eq!(cache.get(&new, "chat", None).unwrap().revision, 8);
    // An unobserved content write between the cached view and navigation must
    // not bless the older projection as current.
    cache.navigation_committed(&version("one", 14), &version("one", 17), 9);
    assert!(cache.get(&version("one", 17), "chat", None).is_none());
}
