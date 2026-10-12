use super::*;

#[test]
#[ignore = "manual local timing; no wall-clock threshold in CI"]
fn reading_authority_timing() {
    let (_dir, mut store) = fixture();
    for _ in 0..25 {
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: uuid::Uuid::new_v4().to_string(),
                action: Action::CreateContact {
                    language_id: "spanish".into(),
                    details: crate::partners::persona::starter("spanish").unwrap(),
                },
            })
            .unwrap();
    }
    let request = Request::capture(&store, input()).unwrap();
    for _ in 0..10 {
        request.validate_source(&store).unwrap();
    }
    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = std::time::Instant::now();
        request.validate_source(&store).unwrap();
        samples.push(start.elapsed().as_micros());
    }
    samples.sort_unstable();
    println!(
        "reading_authority_us samples={} p50={} p95={} max={} contacts=25 build=debug",
        samples.len(),
        samples[99],
        samples[189],
        samples[199]
    );
}

#[test]
fn reading_authority_does_not_project_unrelated_workspace_views() {
    let (_dir, store) = fixture();
    // Removing a view's table inside a rolled-back test transaction proves that
    // authority does not need that projection. This is not a supported workspace.
    let transaction = store.connection.unchecked_transaction().unwrap();
    transaction.execute("DROP TABLE saved_topics", []).unwrap();
    let request = Request::capture(&store, input()).unwrap();
    request.validate_source(&store).unwrap();
    request.validate_access(&store).unwrap();
    transaction.rollback().unwrap();
}

#[test]
fn reading_authority_rejects_a_different_workspace() {
    let (_dir, store) = fixture();
    let request = Request::capture(&store, input()).unwrap();
    let (_other_dir, other) = fixture();
    for result in [
        request.validate_source(&other),
        request.validate_access(&other),
    ] {
        let error = result.unwrap_err();
        assert_eq!(error.diagnostics.unwrap()["reason"], "workspace_changed");
    }
}
