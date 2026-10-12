use super::*;

#[test]
fn workspace_identity_reads_one_row_without_materializing_product_state() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("db")).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let measured = count.clone();
    store
        .connection
        .progress_handler(
            1,
            Some(move || {
                measured.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
        .unwrap();
    let expected = store.snapshot().unwrap().learner.id;
    let projection_steps = count.swap(0, Ordering::Relaxed);
    assert_eq!(store.learner_id().unwrap(), expected);
    let identity_steps = count.load(Ordering::Relaxed);
    store
        .connection
        .progress_handler(0, None::<fn() -> bool>)
        .unwrap();
    eprintln!("workspace_snapshot_vm_steps={projection_steps} identity_vm_steps={identity_steps}");
    assert!(identity_steps < 50);
    assert!(projection_steps > identity_steps);
}

#[cfg(unix)]
#[test]
fn application_data_and_database_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("app-data");
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    prepare_private_directory(&directory).unwrap();
    let path = directory.join("skellyspeak.sqlite3");
    std::fs::write(&path, []).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let _store = Store::open(&path).unwrap();
    assert_eq!(
        std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in [&path, &path.with_extension("lock")] {
        assert_eq!(
            std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn workspace_lock_prevents_a_second_writer_until_close() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("db");
    let first = Store::open(&path).unwrap();
    assert!(Store::open(&path).is_err());
    drop(first);
    assert!(Store::open(&path).is_ok());
}
