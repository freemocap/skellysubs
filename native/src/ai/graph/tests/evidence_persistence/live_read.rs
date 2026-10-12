use super::*;

fn read_limits() -> LiveReadLimits {
    LiveReadLimits {
        export: ExportLimits {
            bytes: 100_000,
            attempts: 100,
        },
        captures: 10,
        capture_bytes: 100_000,
    }
}
fn snapshot(f: &Fixture, sequence: u64, text: &str) -> EvidenceSnapshot {
    let mut snapshot = f.prefix();
    snapshot.provisional = Some(ProvisionalCapture {
        session: "938c2c23-76a5-4c61-98bf-a6f0ccf1a97e".into(),
        sequence,
        text: text.into(),
        failure: None,
    });
    snapshot
}
fn read(f: &mut Fixture, run: &str, captures: &[EvidenceSnapshot]) -> LiveInspection {
    let view = f
        .host
        .read_live_inspection(run, captures, read_limits(), &mut f.store)
        .unwrap();
    for (node, state) in &view.graph.nodes {
        let adopted = f
            .host
            .read_adopted_attempt(run, node, &mut f.store)
            .unwrap()
            .map(|id| id.0.to_string());
        let expected = if *state == Disposition::Adopted {
            view.graph
                .attempts
                .get(node)
                .and_then(|a| a.last())
                .map(|a| a.id.clone())
        } else {
            None
        };
        assert_eq!(adopted, expected);
        if captures.is_empty() {
            let preview = f
                .host
                .read_node_preview(run, node, read_limits().export.bytes, &mut f.store)
                .unwrap();
            assert_eq!(
                serde_json::to_value(preview).unwrap(),
                serde_json::to_value(view.previews.get(node)).unwrap()
            );
        }
    }
    view
}

#[tokio::test]
async fn read_uses_exact_native_topology_and_does_not_commit_or_adopt() {
    let mut f = Fixture::new().await;
    let pending = snapshot(&f, 1, "protected source");
    let before = f.store.bytes();
    let expected = f
        .host
        .read_inspection("a", read_limits().export, &mut f.store)
        .unwrap();
    let live = read(&mut f, "a", std::slice::from_ref(&pending));
    assert_eq!(
        serde_json::to_value(&live.graph).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert!(live.graph.artifact.definition.nodes.contains_key("second"));
    let p = &live.previews["first"];
    assert!(p.live && p.uncommitted && !p.complete);
    assert_eq!(p.retained_sequence, None);
    assert_eq!(p.attempt, f.attempt.0.to_string());
    assert_eq!(f.store.bytes(), before);
    assert!(f.store.publications().is_empty());
    f.host.record_observations(&pending, &mut f.store).unwrap();
    let live = read(&mut f, "a", &[]);
    assert!(live.previews["first"].live);
    assert!(!live.previews["first"].uncommitted);
    assert_eq!(
        live.previews["first"].retained_sequence.as_deref(),
        Some("1")
    );
}

#[tokio::test]
async fn corrected_and_stale_captures_use_native_watermarks_without_prefix_inference() {
    let mut f = Fixture::new().await;
    let saved = snapshot(&f, 2, "corrected");
    f.host.record_observations(&saved, &mut f.store).unwrap();
    let stale = snapshot(&f, 1, "earlier unrelated text");
    assert_eq!(
        read(&mut f, "a", &[stale]).previews["first"].capture.text,
        "corrected"
    );
    let mut later = snapshot(&f, 9_007_199_254_740_993, "new replacement");
    let result = read(&mut f, "a", std::slice::from_ref(&later));
    assert_eq!(
        result.previews["first"].capture.sequence,
        "9007199254740993"
    );
    assert_eq!(
        result.previews["first"].retained_sequence.as_deref(),
        Some("2")
    );
    assert!(result.previews["first"].uncommitted);
    later.provisional.as_mut().unwrap().sequence = 2;
    assert_eq!(
        f.host
            .read_live_inspection("a", &[later], read_limits(), &mut f.store)
            .err()
            .unwrap()
            .code,
        "provisional_conflict"
    );
}

#[tokio::test]
async fn shared_cancellation_settlement_and_adoption_control_preview_eligibility() {
    let mut f = Fixture::new().await;
    begin_host(&mut f.host, &mut f.store, &f.graph, "b");
    f.host.apply(capacity(1), &mut f.store).unwrap();
    let pending = snapshot(&f, 1, "unsaved");
    f.host
        .apply(
            Event::Cancel {
                run: "a".into(),
                node: None,
            },
            &mut f.store,
        )
        .unwrap();
    assert!(
        read(&mut f, "a", std::slice::from_ref(&pending))
            .previews
            .is_empty()
    );
    assert!(read(&mut f, "b", std::slice::from_ref(&pending)).previews["first"].live);
    f.host.record_observations(&pending, &mut f.store).unwrap();
    let late = snapshot(&f, 2, "newer unsaved");
    let cancelled = read(&mut f, "a", std::slice::from_ref(&late));
    assert_eq!(cancelled.previews["first"].capture.text, "unsaved");
    assert!(!cancelled.previews["first"].live);
    assert_eq!(
        read(&mut f, "b", std::slice::from_ref(&late)).previews["first"]
            .capture
            .text,
        "newer unsaved"
    );
    f.report.provisional = pending.provisional;
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    let available = read(&mut f, "b", std::slice::from_ref(&late));
    assert_eq!(available.graph.nodes["first"], Disposition::Available);
    assert!(available.previews["first"].complete);
    assert!(!available.previews["first"].live);
    assert!(!available.previews["first"].uncommitted);
    assert_eq!(available.previews["first"].capture.text, "unsaved");
    let attempt = f.host.inspect("b").unwrap().attempts["first"][0].id;
    f.host.adopt("b", "first", attempt, &mut f.store).unwrap();
    assert!(!read(&mut f, "b", &[late]).previews["first"].live);
}

#[tokio::test]
async fn pause_preserves_inflight_preview_but_restart_unknown_and_poison_never_revive_it() {
    let mut f = Fixture::new().await;
    let saved = snapshot(&f, 1, "retained");
    f.host.record_observations(&saved, &mut f.store).unwrap();
    f.host
        .apply(
            Event::Pause {
                run: "a".into(),
                paused: true,
            },
            &mut f.store,
        )
        .unwrap();
    assert!(read(&mut f, "a", &[]).previews["first"].live);
    f.host.evict_records(&mut f.store).unwrap();
    assert_eq!(
        read(&mut f, "a", &[]).previews["first"].capture.text,
        "retained"
    );
    f.reopen();
    let late = snapshot(&f, 2, "late");
    let unknown = read(&mut f, "a", std::slice::from_ref(&late));
    assert_eq!(unknown.graph.nodes["first"], Disposition::Unknown);
    assert!(!unknown.previews["first"].live);
    assert_eq!(unknown.previews["first"].capture.text, "retained");
    f.store.fail_after_commit = true;
    assert!(f.host.record_observations(&late, &mut f.store).is_err());
    assert_eq!(
        f.host
            .read_live_inspection("a", &[], read_limits(), &mut f.store)
            .err()
            .unwrap()
            .code,
        "reload_required"
    );
}

#[tokio::test]
async fn input_and_output_limits_and_verified_record_reads_fail_without_partial_views() {
    let mut f = Fixture::new().await;
    let pending = snapshot(&f, 1, "protected source");
    let captures = [pending];
    let exact = serde_json::to_vec(&read(&mut f, "a", &captures))
        .unwrap()
        .len();
    let input_bytes = serde_json::to_vec(&captures).unwrap().len();
    let mut limit = read_limits();
    limit.export.bytes = exact;
    limit.capture_bytes = input_bytes;
    assert!(
        f.host
            .read_live_inspection("a", &captures, limit, &mut f.store)
            .is_ok()
    );
    for variant in 0..3 {
        let mut short = limit;
        match variant {
            0 => short.export.bytes -= 1,
            1 => short.capture_bytes -= 1,
            _ => short.captures = 0,
        }
        assert_eq!(
            f.host
                .read_live_inspection("a", &captures, short, &mut f.store)
                .err()
                .unwrap()
                .code,
            "inspection_limit"
        );
    }
    f.host
        .set_record_read_limits(RecordReadLimits {
            records: 0,
            bytes: 100_000,
        })
        .unwrap();
    assert_eq!(
        f.host
            .read_live_inspection("a", &captures, limit, &mut f.store)
            .err()
            .unwrap()
            .code,
        "record_read_count_limit"
    );
    f.host.set_record_read_limits(record_read_limits()).unwrap();
    f.store
        .conn
        .execute("UPDATE graph_record SET payload=?1", [b"{}".to_vec()])
        .unwrap();
    assert_eq!(
        f.host
            .read_live_inspection("a", &captures, limit, &mut f.store)
            .err()
            .unwrap()
            .code,
        "record_mismatch"
    );
}

#[tokio::test]
async fn foreign_duplicate_and_conflicting_captures_are_rejected_and_metadata_is_not_exported() {
    let mut f = Fixture::new().await;
    let saved = snapshot(&f, 1, "source");
    f.host.record_observations(&saved, &mut f.store).unwrap();
    for kind in 0..4 {
        let mut bad = saved.clone();
        match kind {
            0 => bad.identity.engine = Some("foreign".into()),
            1 => bad.identity.artifact = "foreign".into(),
            2 => bad.identity.operation = contract("other"),
            _ => bad.provisional.as_mut().unwrap().session = uuid::Uuid::new_v4().to_string(),
        }
        assert!(
            f.host
                .read_live_inspection("a", &[bad], read_limits(), &mut f.store)
                .is_err()
        );
    }
    assert!(
        f.host
            .read_live_inspection(
                "a",
                &[saved.clone(), saved.clone()],
                read_limits(),
                &mut f.store
            )
            .is_err()
    );
    let mut faulted = saved;
    faulted.provisional.as_mut().unwrap().failure =
        Some(unclassified("private-code-marker", "private-path-marker"));
    faulted.observations[0].redacted_reason = Some("metadata-marker".into());
    let view = read(&mut f, "a", &[faulted]);
    let encoded = serde_json::to_string(&view).unwrap();
    assert!(
        !encoded.contains("private-code-marker")
            && !encoded.contains("private-path-marker")
            && !encoded.contains("metadata-marker")
    );
    assert!(
        view.previews["first"].uncommitted,
        "failure is uncommitted even at the retained text sequence"
    );
}

#[tokio::test]
async fn retry_uses_current_attempt_identity_and_cannot_inherit_old_live_text() {
    let mut f = Fixture::new().await;
    let old = snapshot(&f, 1, "old attempt");
    f.report.provisional = old.provisional.clone();
    f.report.outcome = Err(unclassified("transport", "response"));
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    f.host
        .apply(
            Event::Retry {
                run: "a".into(),
                node: "first".into(),
            },
            &mut f.store,
        )
        .unwrap();
    f.host.apply(capacity(1), &mut f.store).unwrap();
    let view = read(&mut f, "a", &[old]);
    assert_eq!(view.graph.attempts["first"].len(), 2);
    assert!(view.previews.is_empty());
    assert_eq!(view.graph.nodes["first"], Disposition::Prepared);
}
