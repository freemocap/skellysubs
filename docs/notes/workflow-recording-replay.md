# Workflow recording and replay

Status: implemented Windows development capture and offline replay. Controlled
application acceptance passed on 2026-10-10. Representative learner recordings
and physical microphone/speaker acceptance remain to be performed.

## Record a workflow

Run from the repository root with Node 24, installed dependencies and Rust:

```powershell
npm run workflow:prepare
npm run workflow:record
```

Preparation builds the UI and the native executable with `workflow-capture`.
The recorder opens a fresh, isolated application with its own workspace and
WebView profile. Configure language and AI access before recording. Credentials
are not part of the bundle; do not enter credentials during capture.

1. Click **Start recording** in the development toolbar. This snapshots the
   starting workspace and reloads the view before accepting recorded actions.
2. Perform one short workflow with ordinary mouse/keyboard controls. Text and
   audio content are deliberately included. Read and pause naturally.
3. Wait for requested work and playback to finish, then click **Stop recording**.
4. The capture window closes and an isolated offline replay window opens. Leave
   that window untouched. The terminal prints **Recording verified** only after
   replay reproduces the checkpoints and consumes the provider tape completely.
5. Keep the printed recording directory. Failed/incomplete sessions are not
   accepted baselines; their local reports explain the failure.

Each invocation starts a fresh application. Configure any desired starting
conversation before Start. Ordinary application windows and data are untouched.
The toolbar exists only in this explicit development launch. Nothing is uploaded.
Bundles live under ignored `.local/workflow/`; they contain sensitive text/audio
and inherit the directory's local Windows access permissions. They are not
anonymized telemetry or a distribution format for untrusted recipients.

After changing code, rebuild and replay the same bundle:

```powershell
npm run workflow:prepare
npm run workflow:replay -- "C:\absolute\path\to\recording"
```

Avoid changing build output while recording or replaying; build identity checks
reject that condition. A changed application is allowed between runs. A changed
workload, missing target, unexpected input, request mismatch or output mismatch
fails explicitly instead of substituting a live provider response.

## What is captured and replayed

- Consistent native workspace export, referenced Drill audio and allowed browser
  presentation settings; restoration creates a new destination only.
- Ordered pointer, click, scroll, focus, keyboard, composition and exact input
  events with Unicode preserved, plus rendered checkpoints.
- Native provider requests matched by credential-free semantic fingerprints,
  response content, diagnostics, delays, cancellation and failure outcomes.
  Independent requests remain independent. Replay refuses live network calls.
- Native microphone mono f32 sample blocks and sample rate, delivered through
  the normal recording buffers during replay without opening a microphone.
- Browser media byte identities, playback states, decoded audio buffers and
  supported scrub-worklet commands. This observes delivered audio, not sound
  physically emitted by the speakers.

Recordings are bounded. Overflow, unsupported input and incomplete provider work
produce failures, never silent event loss. Imports validate shape, lengths,
SHA-256 assets and roles, and reject links, path escape and missing assets.
Provider replay has no automatic network fallback. Ordinary builds retain the
normal transport and do not install the browser recorder.

## Measurement interpretation

`latency.json` and `replay-latency.json` contain same-clock spans and p50/p95/max
with sample counts. `comparison.json` pairs explicit action/span identities and
shows unmatched observations and missing endpoints. Background/derived actions
are retained without inventing ownership. `builds.json` identifies the compared
executable/frontend content. `clock-calibration.json` retains round-trip samples
and uncertainty; unrelated clock origins must not simply be subtracted.

Replay preserves recorded human pacing and provider response delays. Reading time
is not part of action-to-frame or IPC span measurements, although it remains part
of total replay duration. This preserves cancellation and audio interactions.
There is no pause-compressed replay mode.

`native-latency.json` describes captured transport timing. Adoption and visible
completion are explicitly unknown where no causal endpoint exists. IPC completion
and the next animation frame do not by themselves prove that a particular provider
result has painted. The agreed 50 ms provider-completion-to-display target is
therefore not yet established by these reports. Repeated representative runs are
needed before treating distributions as performance evidence.

## Verification on 2026-10-10

- Strict tooling types and 27 Node tests passed, including asset corruption,
  ambiguous/mismatched requests, action identity and asynchronous ownership.
- Native library suite with capture enabled: 947 passed, 2 ignored, 0 failed.
- Real isolated desktop coach workflow captured exact Unicode input and reply,
  then replayed successfully after shutting down its controlled loopback server.
  No paid provider was contacted. Capture session:
  `0f4ff584-7a98-4d78-a9c4-efdc108f3b07`; replay:
  `15b548fb-137b-481a-99a2-0696a18f3533`.
- A copied recording with deliberately changed input was rejected.
- Synthetic audio in the actual WebView verified PCM identity and start/end
  observations. Native microphone fixtures verify sample retention and replay.
- Forty local clicks with capture off/on measured synchronous dispatch p95 of
  0.4/1.2 ms. This excludes frame wait and controller overhead and is not a claim
  about whole-application recording overhead or long audio sessions.

The remaining acceptance is representative human workflows. Physical microphone
and speaker behavior, continuous VAD segmentation timing and IME diversity have
not been established by the controlled coach test. Replay checks reject divergence.
Touch/pen, file input, drag/drop and unsupported editing surfaces fail explicitly.
Browser/mobile microphone recording is outside this Windows native harness.
Audio observation does not validate every AudioParam automation or acoustic effect.

## Implementation owners and checks

`tools/workflow-replay/` owns launch, recording controls, event collection, import,
replay and reports. `native/src/application/workflow.rs` owns isolated startup,
workspace snapshot and idle checks. `native/src/ai/transport/workflow/` owns typed
provider tapes. `native/src/speech/recording/workflow.rs` owns microphone samples.
`ui/src/platform/ipc/native.ts` exposes an optional timing callback without
capturing arguments or results. These observations do not define graph topology
or schedule application work.

```powershell
npm run workflow:check
node tools/workflow-replay/provider-smoke.ts
node tools/workflow-replay/overhead-smoke.ts
node tools/workflow-replay/audio-smoke.ts
node tools/workflow-replay/mismatch-smoke.ts "C:\path\to\synthetic\recording"
```

Desktop smoke scripts launch disposable windows. Keep their windows unobstructed
and untouched. The mismatch script is intended for the synthetic coach recording.

Final source gates: fast validation, tooling types/27 tests, generated contracts,
UI build, Clippy and feature-enabled binary checks passed. The full UI suite
initially had one Drill readiness failure under unrestricted workers; its 57-test
file passed alone and all 1,953 tests in 288 files passed with four workers.
No application change was made to hide that failure. Documentation entry-point
links and diff whitespace checks passed. Native Clippy also passed with capture
enabled. Changes remain uncommitted.

Repeated replay exposed an overly strict intermediate checkpoint: transient
connection text could change independently of the recorded interaction. Intermediate
screens remain diagnostic evidence; replay validates pointer hit targets, action
identity and final rendered/media/provider outcomes. Final replay session
`0ff41c35-dd83-414b-8bea-5159ea65c24a` passed. A subsequently modified input was
rejected (`mismatch-ac7de159-500b-41ea-acf4-f988f3054118`). Replay failure reports
include the current action and local screen, and preserve external-input failures.

## Replay maintenance and measurement tally

The prepared capture executable now lives in `.local/workflow-build/` so ordinary
native builds cannot replace it. `npm run workflow:prepare` builds and copies that
executable; `workflow:record` requires it. Ordinary native builds reject workflow
launch environment variables before opening a workspace.

Replay markers now identify the expected DOM event type. A derived focus event
cannot consume a marker reserved for a keyboard/pointer event. A regression test
reproduced the mistaken external-input failure and passes after the correction.
The complete learner session still requires replay acceptance; this focused test
alone does not establish that all pointermove interruptions have the same cause.

`npm run workflow:tally -- RECORDING VERIFIED_REPLAY...` produces content-free
per-command distributions and paired changes. Successful replay receipts carry
the recording manifest digest; the tally rejects candidates without that exact
identity. See the [efficiency audit](ai-graph-efficiency-audit.md) for the running
measurement table and the separate advisory complexity inventory.

### Quiet replay and input ordering

Use `npm run workflow:replay -- RECORDING --mute` to silence replay output, or
`npm run workflow:record -- --mute` to mute the automatic replay after an audible
capture. Direct Node equivalents accept the same arguments. The WebView2
`--mute-audio` output switch leaves application volume preferences, decoded bytes,
playback events and audio timing in place. Replay receipts record `outputMuted`.
Audible output remains the default. This is not verification of physical speakers.

Replay waits for the expected DOM hardware event after each CDP dispatch; CDP
acknowledgment alone does not mean a wheel event has reached the page. Input
markers match type, coordinates, modifiers and applicable key/button/delta fields.
Unexpected events and missing acknowledgments still fail. Replay windows ignore
OS cursor events, while automation reaches the renderer directly; capture windows
remain interactive. The replay window is click-through, so avoid clicking through
it into an application underneath. Failure reports retain bounded input coordinates
and event types as well as the action checkpoint. Cursor trajectories replay by
coordinates; semantic clicks/input retain target checks. Independent changes in
content under a moving pointer do not imply a different recorded trajectory.

## Recorder save reliability â€” 2026-10-10

Implemented after learner session `18905c08-e69c-4bcd-88c4-f0eafffaf2d1`
failed its final checkpoint. The save coordinator retains native tape before
collecting the final checkpoint. Controlled failures export collected events/assets
to `partial-recording/` and timing spans to `partial-latency.json`, with explicit
incomplete status and failure stage in `incomplete.json`. Replay import continues
to reject these bundles. A failure after validation also forces the partial export
to incomplete without changing an already complete export.

Known ResizeObserver delivery notices remain in application diagnostics but no
longer reject the recorder checkpoint. Genuine browser errors, unhandled rejections,
unfinished playback and observation failures still fail verification. Authored
checkpoint reasons are allowlisted across the browser boundary; arbitrary browser
exception content is not persisted as recorder diagnostics. Partial export failures
are reported explicitly. This does not provide crash/power-loss recovery: buffered
browser timings unavailable after a browser exit cannot be reconstructed, and an
abrupt recorder-process termination still loses in-memory events. Earlier unsaved
recordings cannot be retroactively recovered by this change.

Verification: 35 workflow tooling tests/types passed; fast gate passed. A real
isolated desktop test injected a genuine error and verified saved actions, timing
samples and provider tape with `final_checkpoint` failure:
`.local/workflow/564a7469-4b65-4ab1-83ae-8861279a5789`.
A separate desktop test injected a ResizeObserver notice, saved successfully and
completed offline replay: capture `109ca019-5c0e-4baf-b8f2-b02957419ddb`, replay
`a2686964-0ef9-4e06-8a99-10e78d1bd5c5`. These tests used the prepared app without
calling providers or changing the learner workspace. The late-export status guard
was subsequently covered by the final tooling suite. Full learner-workflow replay
remains a separate check; these tests prove the repaired save boundary.

Start a new recorder process with `npm run workflow:record -- --mute`; these scripts
are loaded at launch and need no native/UI rebuild. Configure/sign in before Start
recording. Wait for active provider work and playback to finish before Stop. Leave
the replay window alone while automatic verification runs. If verification fails,
inspect the saved complete recording or explicit partial evidence rather than
immediately repeating the entire exercise.


## Concurrent capture repair — 2026-10-10

Learner recording `50c9b0b8-8b15-4f53-bb07-8ab1bb0c3637` failed during
`native_stop`. Its partial bundle retains 2,707 actions, 1,297 timing events and
three display events. Reading assistance and microphone start both reported the
capture guard error before Stop. No provider tape was exported. The retained
logs do not identify the first invalidation; attributing this particular session
to an exact call would exceed the evidence.

A deterministic regression identified a capture defect: overlapping requests
with identical semantic fingerprints shared one active-call key and invalidated
the session. Capture now gives each invocation an independent ID and record
index. Response frames, completion and cancellation address that invocation.
Fingerprints select replay queues; identical requests consume their recorded
occurrences in admission order, including when their completion order differs.
This does not serialize provider calls or alter graph ownership. The tape format
and workspace schema are unchanged. It does not establish deterministic ordering
of otherwise indistinguishable callers across arbitrary scheduler changes.

The first native failure now retains an authored reason, mode, elapsed time,
active/recorded call counts and captured byte count in
`native-capture-failure.json`. Subsequent failures preserve the first reason.
Requests, fingerprints, credentials and response content are excluded from that
report. Replay mismatches, unsupported network work, limits and unexpected
cancellation still fail explicitly. Browser partial-save handling remains active;
this change does not reconstruct a provider tape from the failed learner session.

Verification: the overlap test failed before the change and passes
after it. Native tests cover reversed completion order, cancellation of one
identical invocation, offline replay queue consumption and preservation/redaction
of the first failure cause. `tools/workflow-replay/overlap-smoke.ts` exercises two
identical native `check_access` commands against a local fixture that waits for
both requests before answering, exports capture, closes the fixture, and repeats
the calls in an isolated offline replay. This is a focused command-path check,
not full learner-session acceptance.


Final checks passed: capture-enabled native library 954 passed / 5 ignored,
Clippy (`--features workflow-capture --lib --tests -- -D warnings`), binary
checks/build, 35 workflow tooling tests/types, and final fast gate. No UI or
workspace contract changed in this repair.

Real desktop evidence on the prepared executable:
- Identical overlapping native requests: capture
  `4c5feb1b-09e8-4c02-b771-5a9626cc102c`, offline replay
  `87ffc123-f27c-43de-ae53-8089df96a7ac`; both succeeded. The same assertion failed
  before replacement of the executable. The fixture waits for both requests to
  arrive before returning either response.
- Start/Stop controls and final checkpoint: capture
  `573a8995-b7bd-4d2b-b48a-17ac50b19c96`, replay
  `6f33d911-6336-4bd2-aa24-a3e2f62da662`; passed.
- Streamed coach response from local controlled provider: capture
  `e0b13121-e36a-4b62-a2de-3bc576f60865`, replay
  `d06322a2-84d9-40b1-a77d-d9620aef5ac0`; passed with fixture shut down for replay.

Prepared executable SHA-256:
`328abb04855e4725e01ee361e7b22c700b44dbe3506a6e3d7df41f7314d1277f`.
A fresh recorder process can now be started with `npm run workflow:record -- --mute`.
Full learner-workflow reproduction is the next user check. These focused passes do
not prove which call first invalidated the preceding learner capture.


## Focused read profiling

`node tools/workflow-replay/local-read-profile.ts <consistent-workspace-export>`
restores a disposable copy of a consistent SQLite export, launches the prepared
capture executable muted, blocks provider traffic and measures 20 serial plus 20
concurrent repetitions of workspace, language-total and skill-evidence reads.
It verifies totals against detailed evidence for every language after timing.
The original export is unchanged. Never copy an actively written SQLite file by
itself; obtain a consistent export/backup first.

Capture-build-only `local_timings_start` and `local_timings_finish` controls record
store-lock caller, wait and hold durations without content. Samples are bounded at
100,000 and overflow is explicit. Ordinary builds have no recorder. Timing records
are written after releasing the store guard. IPC timing includes dispatch and
delivery, not visible paint; frame intervals are a separate observation. Native
samples also include background application reads and are not individually
correlated to injected commands. Human reading pauses are absent from these spans.

Focused results are saved as `local-read-profile.json` with build identity. They
are usable when a full replay fails, but never constitute a full-session replay
acceptance receipt. The efficiency audit retains the current request mismatch,
workload limits and before/after measurements. Prepare a new executable after
native changes before measuring, and keep compiler/tests and other desktop
recording sessions idle during timing runs.


Responsiveness batch verification completed: 957 native tests (five ignored),
2,020 UI tests, Clippy, binary/contract checks, UI build, 37 workflow tooling tests,
fast gate and documentation links passed. Prepared combined build identity:
`a2886805242c17bc68392363ca9714480c74fe14ef9d1dd48cfdec0d43b72530`.
Two focused isolated runs verified all 21 language totals and measured concurrent
read p95 at 16-35 ms. Full-session replay reliability and roughly half-second
startup frame stalls remain unresolved; this is not whole-app responsiveness
acceptance. Exact measurements and limits are in the efficiency audit.

## Conversation identity and deterministic persona sampling

The full-session provider mismatch was traced to an input difference, not a
provider queue collision: persona projection ranks details using the conversation
UUID. Creating a conversation during replay generated a new UUID and therefore a
different sampled persona section in the system prompt. Saved execution inputs
from recording 3cd58ba4-8b39-418c-b8ce-4771fad9843b and replay
48040e84-4b0a-4b17-ba63-c778114cd959 differed in that section alone for the first
failed prose request.

Capture builds now record conversation creation identities as bounded synchronous
identity records in the native tape. Replays consume the exact UUID, validate its
shape and reject missing or malformed records. Normal builds still generate UUIDs
directly. The graph engine, prompt sampler and strict provider fingerprints are
unchanged. This captures an input to sampling rather than weakening request matching.

`node tools/workflow-replay/identity-smoke.ts` invokes actual application commands
in separate capture/replay apps, creates three conversations and compares their
IDs and SHA-256 prompt hashes. It failed against the preceding executable and
passed against the fixed one; receipt:
.local/workflow/91ce8ee4-4d7d-40e7-8846-de12d0d95bbe/identity-result.json.
The focused native workflow suite passes all 12 tests and Clippy passes with the
capture feature and warnings denied. Full native regression is pending below.

A derived copy of the existing recording is prepared at
.local/workflow-recovered-identities. Two IDs were recovered in receipt insertion
order from its retained original workspace, excluding receipts already in the
initial workspace snapshot. Its recovery-provenance.json records their source.
The original recording and its 132 provider records are unchanged. The derived
fixture has no original timing for synchronous identity creation; those two
record offsets are zero. It is for reproducibility validation, not an assertion
that every nondeterministic application input is captured. Full replay is pending.

### Replay repair verification and remaining checkpoint

Native library verification: 959 passed, five ignored, no failures. Capture Clippy
and ordinary production binary checks passed. Workflow tooling: 38 tests plus
TypeScript passed, including a regression that first failed for SVG-child selectors.

Fresh recordings now select a uniquely aria-labelled control instead of its
incidental icon child. Duplicate labels keep the exact structural selector; scroll
and wheel observations retain their original element. Recorded coordinates still
must hit the intended target. There is no guessed click or skipped action fallback.

The real controlled-provider capture/replay loop passed with the fixture server
closed before replay: capture 88c2cab3-36f6-40bc-b914-530a17ac7f12, replay
6e2dbb7d-34a0-46cb-8e97-03db9b464389. The captured app commands, streamed response,
rendered checkpoint and media checkpoint were verified. Prepared build identity:
edad3916600672bf7d94f18a304106bada06eb2a0517d672217af5a688d7b514.

Full derived replay 8377437d-f65b-404e-9a46-c2728e1730a2 passed the prior provider
mismatch: the recovered conversation exists and three prose executions succeeded.
It stopped at action-3831 (about 120 seconds of recorded actions), target-exists,
with no native capture failure. The original action-3833 IPC trace identifies this
as microphone start. Its target is a positional SVG path; that path is absent and
the old coordinates currently hit a textarea. This is not a full-session pass.
No click was redirected and no provider result was substituted. A new recording
on the current layout is required to finish the full-session checkpoint; the
labelled-control capture improvement cannot reconstruct missing original geometry.

The full fast gate currently fails because the concurrently edited activity UI
references five untranslated message occurrences (four distinct keys): Selected
operation, Response [middle dot] streaming, Graph structure, and Choose a graph.
Those files are owned by the other agent and were not changed in this batch.
The startup UI tests/build passed before those concurrent edits; current whole-tree
release readiness is not asserted. Documentation links and whitespace checks pass.
