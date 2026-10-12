# v4.0.1 release: speech fixture timeout

The loader limitation below was subsequently reproduced in v4.0.3 CI and traced
to a missing Windows manifest. See the
[follow-up investigation](release-v4.0.3-windows-loader-2026-10-12.md).
The v4.0.4 follow-up at the end of this note supersedes the partial timeout
policy below; the original verification remains a historical record.

## Observed failure

The [v4.0.1 release run](https://github.com/freemocap/skellyspeak/actions/runs/38246914057)
failed only in `checks / native / Native / speech`. Its other validation jobs
passed. Draft creation, desktop/Android packaging and publication were skipped.
The separate iOS distribution workflow and ordinary CI on the same commit passed.

`late_transcription_and_retry_cannot_replace_a_newer_failed_take` failed because
the mock HTTP listener reported `No transcription request arrived` after five
seconds. Its request notification channel then closed, causing `RecvError` in
the test. The speech job reported 83 passed, one failed and one ignored; it did
not reach a failed assertion about newer-take ownership.

## Diagnosis and implemented mitigation

The mock begins its bounded accept wait when its readiness future is polled.
That excludes workspace fixture setup, but still includes asynchronous audio
inspection and durable admission before HTTP dispatch. Five seconds is therefore
an implicit performance requirement in a behavioral test. The hosted log does
not reveal which pre-dispatch operation consumed the interval; slow dispatch is
consistent with the failure and the successful CI run on the same commit.

Increase only the mock's accept guard to 30 seconds, retaining an explicit
failure if no request arrives. Keep the five-second socket-read and response
release guards. No production execution, provider timeout, release workflow or
version changes are involved.

Add `mock_request_allows_slow_pre_dispatch_work`, which polls readiness before
delaying HTTP dispatch for 5.1 seconds. It retains the client socket until the
server finishes. The existing setup-delay regression remains intact. The tests
stay with their existing owner; splitting this already large suite is a separate
responsibility review, outside this release repair.

## Local verification and limits

- The actual fixture functions and both fixture regression tests were extracted
  verbatim into an ignored standalone Rust test harness to bypass a local native
  executable loading failure. The new test reproduced both hosted error messages
  with the five-second guard. With the fix, both fixture tests passed.
- `npm run check:fast`: passed.
- `cargo clippy --manifest-path native/Cargo.toml --lib --tests -- -D warnings`:
  passed.
- `npm run build`: passed, with the existing large-chunk advisory.
- `npm run release:test`: 16 passed.
- Full native execution is not established by the isolated fixture tests. The
  native executable exits before the test harness starts with Windows
  `0xc0000139` (`STATUS_ENTRYPOINT_NOT_FOUND`), including outside the sandbox and
  with a clean process-local PATH. The original regression could not run locally.
- Hosted validation of this change and the skipped signed packaging jobs remain
  required. Signing/distribution on other platforms cannot be verified here.

No commit, push, version bump, tag or release was performed. The failed historical
run is unchanged. Re-run CI with the repair before preparing the next patch.

## v4.0.4 follow-up: settlement waits

The [ordinary v4.0.4 CI run](https://github.com/freemocap/skellyspeak/actions/runs/38273286795)
failed only in the speech domain: 84 passed, one failed, one ignored. The failure
was `leaving_one_or_all_consumers_does_not_abandon_submitted_recognition`, whose
three-second durable-settlement wait expired with `Elapsed(())`. The original
v4.0.1 failing test and both fixture-timing tests passed. The separate
[release workflow](https://github.com/freemocap/skellyspeak/actions/runs/38273287618)
passed every validation job, including speech, and all desktop builds; Android
packaging was still running when inspected. iOS distribution passed separately.
The ordinary CI failure is not evidence that release packaging failed.

The prior accept-timeout repair did not cover two later three-second completion
waits in the same fixture suite. The hosted error did not preserve the last
receipt/graph state, so it cannot identify which settlement stage consumed the
time. A passing independent run on the same commit supports a timing-sensitive
failure rather than a deterministic ownership assertion failure.

The suite now uses one 30-second per-phase hang guard for request acceptance,
socket I/O, response release and both durable-completion waits. Completion polls
share a helper that preserves the original assertions: succeeded receipt and,
where required, no cancelled nodes and all nodes adopted. Timeout diagnostics
include the consumer, last receipt state and inspected node dispositions, without
provider content. The suite remains with its existing owner; consolidating its
waits is scoped to this repair, not a general decomposition of the large file.
Production execution, provider timeouts and workflow gates are unchanged.

`abandoned_recognition_can_settle_after_a_slow_provider_response` drives the real
HTTP/native execution path: cancel its consumer after dispatch, begin waiting for
settlement, delay the provider response 3.1 seconds, then require durable success,
graph adoption and exactly one provider attempt. It failed under the old
three-second guard with the receipt still pending, then passed with the shared
guard. This verifies the waiting contract; it does not reconstruct the exact
hosted scheduling delay.

Final local verification:

- The new regression failed with the old three-second budget and passed with
  the common guard; the original hosted-failing test also passed.
- `cargo test --manifest-path native/Cargo.toml --lib -- --test-threads=4`:
  996 passed, five existing ignored tests, zero failures (642.73 seconds).
- Exact CI preparation: inventories all 1,001 tests successfully.
- `node tools/ci/native-tests.ts run speech`: 86 passed, one ignored, zero
  failures while the full native suite was also running, exercising contention.
- Final fast gate, strict Clippy (`--lib --tests -- -D warnings`), documentation
  links and diff whitespace: passed.

Hosted verification of the changed tests remains pending. No commit, push,
version bump, tag, release rerun or publication was performed.
