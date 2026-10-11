use super::{
    durable_store::SqlStore,
    record_access::{Observed, available_host},
    *,
};

#[test]
fn one_preview_does_not_read_an_unrelated_current_producer() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, first, _) = available_host(&mut store);
    host.adopt("a", "first", first, &mut store).unwrap();
    host.apply(capacity(4), &mut store).unwrap();
    let second = host.inspect("a").unwrap().attempts["second"]
        .last()
        .unwrap()
        .id;
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: Some(RecordKey::Attempt(second)),
        commits: 0,
    };
    assert!(
        host.read_node_preview("a", "first", 100_000, &mut observed)
            .unwrap()
            .is_none()
    );
    assert_eq!(observed.reads.len(), 3);
    assert_eq!(observed.commits, 0);
    observed.fail = Some(RecordKey::Attempt(first));
    assert_eq!(
        host.read_node_preview("a", "first", 100_000, &mut observed)
            .unwrap_err()
            .code,
        "read_injected"
    );
}
