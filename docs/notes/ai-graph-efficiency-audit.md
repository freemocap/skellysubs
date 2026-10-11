# AI workflow efficiency audit

Date: 2026-10-10. Stage 11 source investigation and completed focused corrections.
Initial findings, implementation evidence and measurements are separated below. Governing checklist:
[production integration](ai-graph-production-integration.md).

## Scope and result

Reviewed conversation admission/dispatch/publication, store ownership, conversation
and activity subscriptions, saved result inspection, reading, playback and Drill
audio delivery. The findings below are confirmed source mechanisms. Their ranking
is provisional until traces distinguish frequency, lock wait, service time and
visible delay. This is not an exhaustive audit of every application command.

Provider calls are dispatched concurrently outside the store guard. Local graph
operations release the provider permit. The problem is not a universal serial
provider pipeline: synchronous shared-store work, repeated projections and polling
can delay otherwise independent work. No deadlock or exact cause of the reported
freeze has been established.

## Ranked inventory

### Linear implementation checklist

- [x] **1. Reading validation:** replace repeated full-workspace snapshots with narrow identity reads.
- [x] **2. Scheduler claims:** remove unnecessary retained-run scanning and inspection.
- [x] **3. Store contention:** reduce expensive work holding the shared store lock.
- [x] **4. Conversation projections:** reduce repeated queries and graph inspections.
- [x] **5. Subscriptions:** remove duplicate conversation/activity subscriptions and unnecessary refreshes.
- [x] **6. Polling:** reduce avoidable delays between completed work and visible results.
- [x] **7. Attempt inspection:** avoid repeatedly reconstructing unchanged execution details.
- [x] **8. Activity rendering:** avoid unnecessary graph layout and collapsed-detail work.
- [x] **9. Admission budgets:** avoid reconstructing history to calculate outstanding work.
- [x] **10. Audio delivery:** remove unnecessary waits before playback.
- [x] **11. Helper requests and errors:** remove redundant preflight reads and delays displaying failures.
- [x] **12. Final verification:** run regression gates, consolidate measurements and prepare the app for a responsiveness check.

Paths below are repository-relative. Each row identifies both evidence and the
measurement required before claiming a performance improvement.

| Priority | Confirmed mechanism and source | Measurement / candidate repair |
| --- | --- | --- |
| 1 | `native/src/language/reading/mod.rs::validate_source` calls `store.snapshot()` for learner identity, then non-speech access validation calls it again. `application/native_reading.rs` invokes source validation every 10 ms while waiting. | Count snapshots and lock occupancy per pending request. Replace full projection with an authoritative narrow identity read; wake on relevant changes while retaining cancellation and final authority checks. |
| 2 | `conversations/execution/graph_runtime/execution.rs::next` starts each claim by enumerating retained conversation runs; it exports inspection before checking active state. The host calls `next` repeatedly to drain work. | Record scanned runs, exported bytes and claim time with increasing retained history. Derive ready-work indexing from authoritative runtime state; preserve reactivation, recovery, single-step and independent branches. |
| 3 | `application/state.rs` owns one synchronous store mutex. Snapshot/history reads, scheduler claims and publication use it. `commands/workspace.rs::watch_conversation` also performs projection directly in its async command. | Separate lock wait from lock-held CPU/SQL/serialization time. Narrow expensive projections and remove synchronous blocking from async workers; moving work to a blocking worker alone does not eliminate contention. |
| 4 | `conversations/execution/snapshots.rs` uses the workspace-wide revision, projects up to 100 messages and many turns, and includes graph inspection in turn views. Owner/status/evidence SQL is repeated inside these projections. | Count queries, graph inspections and IPC bytes per changed message. Introduce appropriately scoped invalidation and bounded projections without inventing a second runtime interpretation. |
| 5 | `useConversationSnapshot.ts` and `activity/useConversationActivity.ts` independently watch full conversation snapshots. The activity watcher awaits `readWorkspace()` before publishing the received snapshot. Both refresh already-loaded history as revisions change. | Count active watchers and repeated page reads; measure snapshot arrival-to-render separately from workspace refresh. Share suitable subscriptions and avoid reloading unchanged history. Preserve edits/deletions in loaded history. |
| 6 | `application/scheduler.rs` sleeps 100 ms after dispatch. `watch_conversation` samples every 150 ms and forces a snapshot after 20 seconds. Changed streaming observations can bump the workspace revision every 250 ms. | Trace provider completion → settlement → adoption → projection → paint. A polling interval can itself exceed the agreed 50 ms target on paths that wait for the next tick. Use completion/change notifications with recovery checks, not an additional scheduling model. |
| 7 | `ai/inspection.rs::attempt` reconstructs historical inspection at the requested revision. `NativeAttemptDetails.tsx` reloads when graph revision changes. First inspection credit calls the same inspection reader again in `learning/effort/bot.rs`. | Measure reconstruction duration/bytes and duplicate work for a selected attempt. Reuse authoritative immutable evidence where valid, retain exact revision selection, and preserve one-time credit and disclosure ownership. |
| 8 | `activity/NativeGraph.tsx` memoizes layout by artifact object reference; deserialized snapshots supply new objects. Nodes/edges are rebuilt. `ResponseDetails.tsx` stringifies values even inside closed details. Selected `WorkspaceGraphActivity.tsx` runs continue polling when surrounding details are collapsed. | Profile layout, serialization and React commits during generation and selection. Retain artifact identity, serialize expanded details on demand, and make subscriptions visibility-aware. All topology still derives from the executable artifact. |
| 9 | `graph_runtime/budget.rs::outstanding` reconstructs current historical inspection across qualifying partitions to calculate admission. | Trace send/admission latency, partition count and checkpoint bytes. Read authoritative resident state where available without weakening admission or recovery semantics. |
| 10 | `platform/audio/reading-speech.ts` awaits `observer.onAudio` before playback. Drill's callback awaits `inspectDrillAudio` before returning. Message speech delivery also uses 80/100 ms waits in specific pending/stream paths. | Measure audio ready → first sound independently of provider wait and inspection readiness. Allow independent inspection/display where safe; preserve cache identity, cancellation, alignment and source ownership. |
| 11 | `TurnReplyHelp.tsx` performs conversation and workspace reads before a new helper request. `platform/ipc/native.ts` awaits diagnostic logging before rethrowing an IPC failure. | Count preflight reads and error-display delay. Remove redundant preflight only when native authority remains sufficient; preserve useful diagnostics without delaying visible failures. This helper path mutates requested work, unlike opening saved content. |

`execution.rs::maintain` checks checkpoint usage before compaction; it does not
compact on every transition. Include occasional compaction in long-tail traces,
but do not describe its cheap threshold check as a full rewrite.

Reply reveal deliberately ticks at 40 ms and can spread a burst across multiple
frames. Report first content, last received content and completed presentation
separately. Visible animation duration is not evidence of slow Rust execution.
Development React measurement retention is already bounded by
`platform/diagnostics/development-measures.ts`; benchmark the built UI as well as
development mode and keep capture measurements in their own bounded buffer.

## Agreed initial budgets and workflow set — stage 11.2

The user approved these targets on this machine on 2026-10-10:

- Existing loaded content responds by the next frame.
- Small local reads reach visible results within 50 ms at p95.
- Provider completion reaches visible results within 50 ms at p95, excluding
  provider waiting time. Record streaming first-content and final-content paths.
- Large cold-history loads receive a separately measured budget.

Representative workflows: conversation/send; opening saved suggestions and
feedback; activity selection during generation; cached playback;
recording/transcription; message revision.

These are acceptance targets, not passing measurements. Record sample counts,
warm/cold state, build identity, workspace size, capture enabled/disabled and
provider timing mode. Include p50/p95/max and individual long tasks rather than
only averages. Do not subtract provider time from unrelated queued local work.

## Measurement and repair discipline

Use correlated operation IDs for UI input, IPC dispatch, native entry, mutex
acquisition, SQL/projection, graph settlement/adoption, IPC return and browser
presentation. Each process uses its own monotonic clock; calibrate cross-process
offset and uncertainty rather than directly subtracting unrelated clock origins.
Measure queue wait separately from service time and recording overhead separately
from application latency. Repeated operations need distinct IDs.

First establish actual desktop attachment and explicit capture. The recording
agent's tooling is a foundation, not proof that native provider traffic, user
audio or the workspace have been captured. Native transport recordings must use
the real transport boundary; browser network recording cannot see Rust HTTP.
Replay must use an isolated workspace and fail when required data is missing.

At stage 11.10, take one ranked problem at a time: establish its baseline, make
the smallest coherent correction, rerun the same trace and affected regressions,
then record the result before moving on. Keep publication authority, exact source
identity, cancellations, disclosure, earned credit and executable topology intact.
The inventory above records the initial audit. Completed fixes and their evidence
are recorded below; a focused timing result is not a running-app acceptance result.

On 2026-10-10 the user authorized using the existing capture observations and
focused measurements to repair these findings without waiting for complete session
replay. Responsiveness is the priority. Full replay remains useful verification,
but is not a prerequisite for each local correction.

## Measured workflow observations — 2026-10-10

The saved learner session `e1d7bd6a-1870-4705-9908-eb2434ffccf8` contains 368.756
seconds, 5,482 input events, six microphone/transcription sequences and no sign-in
inside capture. Its assets passed integrity checks. The initial replay stopped on
an unexpected pointermove; after the recorder-marker fix, full-session replay still
failed at reading/provider matching. Repeatable learner-workflow acceptance remains pending.
These capture observations are useful, but are not verified before/after savings.

| IPC / observation | Samples | p50 ms | p95 ms | Maximum ms |
| --- | ---: | ---: | ---: | ---: |
| get_snapshot | 126 | 885.8 | 4292.1 | 7247.3 |
| get_skill_evidence | 125 | 629.5 | 2727.5 | 5852.3 |
| get_language_totals | 192 | 380.9 | 2669.3 | 7557.4 |
| read_speech_audio | 62 | 553.5 | 2757.9 | 4592.0 |
| next-frame callback | 190 | 3.6 | 50.9 | 121.2 |

IPC spans include queueing, lock contention and any work awaited by that command.
They do not isolate SQL execution. Frame callbacks are not proof of visible result
completion. Two timing endpoints are missing and remain explicitly unknown.
Long-poll subscription waits are reported separately and are not local read latency.

### Running savings tally

| Change | Baseline | Candidate | Verified savings |
| --- | --- | --- | --- |
| Measurement and replay infrastructure | Capture observations above | Full learner replay pending | Not established |
| Reading authority check, debug fixture with 25 contacts, 200 warm samples | p50 13.987 ms; p95 16.004 ms; max 19.010 ms | p50 0.771 ms; p95 1.300 ms; max 2.101 ms | Local median reduction 13.216 ms (94.5%); p95 reduction 14.704 ms (91.9%). Single before/after run; not end-to-end UI savings. |
| Idle claim, debug fixture with 12 retained coach runs, 100 samples | p50 9.211 ms; p95 15.334 ms; max 22.559 ms | p50 0.434 ms; p95 0.865 ms; max 0.987 ms | Local median reduction 8.777 ms (95.3%); p95 reduction 14.469 ms (94.4%). Single before/after run; not end-to-end UI savings. |
| Admission, one pending partner run, 100 samples per path in one debug process | Persisted reconstruction: p50 11.065 ms; p95 12.382 ms; max 12.857 ms | Matching resident state: p50 0.485 ms; p95 0.806 ms; max 1.828 ms | Median difference 10.580 ms (95.6%); p95 difference 11.576 ms (93.5%). Equal counts asserted; not end-to-end UI savings. |

Use `npm run workflow:tally -- RECORDING VERIFIED_REPLAY...` to build the JSON
report. Every candidate must have a successful replay receipt with the SHA-256
identity of the exact recording manifest. Reports retain sample counts,
p50/p95/max, matched/unmatched spans and missing endpoints. Paired p50/p95 savings
are baseline minus candidate; negative values are regressions. Do not add durations
of overlapping commands and label that sum elapsed time saved. Use repeated
same-build runs to establish noise before attributing a change to optimization.
The local report for this capture is `measurement-tally.json` in its session directory.

### Lightweight complexity hygiene

`npm run complexity:report` emits an advisory JSON inventory using the existing
TypeScript parser. It reports per-function source length, explicit decision count
and maximum decision nesting. Decision count includes if/conditional/loops/catch,
non-default cases, and logical &&/||/??; nested functions are counted independently.
This is a syntax metric, not formal cyclomatic complexity, Big-O, or a latency
estimate. Rust currently contributes file line counts only; no Rust parser or
new dependency was added. Generated sources and separately located tests are
excluded; Rust inline test modules remain in file length.

The initial inventory covers 1,104 source files and 7,085 TypeScript functions.
ConversationPage has 166 counted decisions over 818 function lines; DrillPage
has 101 over 455 lines. These flag review candidates, not automatic refactors or
new hard gates. Existing one-file-at-a-time decomposition policy still applies.
The local full inventory is `.local/workflow/complexity-baseline.json`.

## Audit item 1 — narrow reading authority queries

Implemented 2026-10-10: `Store::learner_id` reads the singleton owner directly.
Reading capture, source/access validation, cancellation cleanup and result
settlement use this query instead of a workspace snapshot. Non-speech source
validation checks the owner once through access validation. Speech source
validation retains its direct owner check. No cache, schema or polling change.

Previously each non-speech validation built two complete workspace projections,
including persona validation, conversation listing/settings and language data.
It now performs one indexed singleton identity read. Other existing source/access
queries remain. Cost of the identity check no longer scales with projected history.
This removes unnecessary lock-held work; the recording's multi-second IPC delays
have not been causally attributed to this check alone.

Timing command (the fixture setup is outside the measured interval):

```sh
cargo test --manifest-path native/Cargo.toml --lib reading_authority_timing -- --ignored --nocapture
```

The regression `reading_authority_does_not_project_unrelated_workspace_views`
failed before the fix and passed afterward. It temporarily removes the saved-topic
table inside a rolled-back test transaction to prove the reading checks do not
depend on that unrelated projection; it does not define a supported database state.
Existing cancellation, pause, access-change, conversation ownership and recovery
tests passed, along with a new different-workspace rejection test.

Final verification: native library suite 936 passed / 3 ignored; Clippy
(`--lib --tests -- -D warnings`), binary checks, `npm run check:fast` and
documentation links passed. The ignored timing test was run explicitly both before
and after the change. No running-app responsiveness acceptance or full learner
replay pass is claimed. The 10 ms validation polling interval is unchanged; a
notification-based replacement belongs to the separately ranked polling work.

## Audit item 2 — scheduler inspection work

Implemented and verified correction: scheduler claims read the native
engine's active/paused flags through `run_control` rather than serializing a full
inspection export. The accessor checks host validity and resident state, and
returns current core fields without caching or inferring readiness. The SQL scan
filters terminal coach rows using the existing host eligibility rule; cancelled
and invalidated rows still reach core cancellation. Partner and independently
requested helper/speech runs remain candidates. SQL still enumerates those
candidates; there is no additional application-owned scheduling index.

```sh
cargo test --manifest-path native/Cargo.toml --lib idle_claim_with_retained_history_timing -- --ignored --nocapture
```

The fixture completes 12 coach turns before timing 100 idle claims. Setup and
provider fixture execution are outside the measured interval. Controls, provider
capacity and adoption rules are unchanged. Further reduction of candidate scans
requires evidence about active partner workloads rather than treating completed
reply state as proof that independent helper work is finished.

Verification: 77 targeted native tests passed, including retry, restart, cancellation, independent helper branches and single-step controls. The core control-reader projection/uncertain-host regression, Clippy and fast gate passed.

## Audit item 3 — store contention and async-worker isolation

Items 1–2 remove unnecessary lock-held projections. The conversation long-poll
command now performs its synchronous lock acquisition and snapshot read on the
blocking-worker pool, consistent with workspace snapshot reads. This does not
make the database concurrent or reduce the projection's own SQL cost; item 4
addresses repeated projection reads. No extra cache or lock protocol was added.

The current-thread async regression holds the store mutex on another thread and
requires the async worker to remain available to release it. It passed, along with
the fast gate. Full-app lock-wait distributions remain unmeasured; no latency
savings are attributed solely to moving work onto a blocking worker.

## Audit item 4 — conversation context projection

Implemented and verified with six affected product-state, feedback-context and
partner-publication tests plus the fast gate: the message projection
loads and parses each owning turn's context once per snapshot, then reads its
saved fields from that value. The bounded per-call map is dropped when the
snapshot returns. There is no persistent cache or new revision/invalidation rule.
For one user/partner pair this replaces eleven direct context queries in the
message loop with one; assessment projection and graph-status reads are not
included in that count. Missing and JSON-null fields still produce absence;
present values still undergo typed decoding. No end-to-end timing gain is claimed.

## Audit item 5 — shared pending reads and immediate activity publication

Identical in-flight conversation reads share one IPC request per webview, keyed
by conversation, revision and history boundary. Entries disappear on success or
failure; completed data is not cached. The activity panel publishes an arriving
snapshot before refreshing the workspace directory, then reconciles any changed
selection. Same-revision heartbeat responses do not replace its current snapshot.
History refresh still preserves edits/deletions; distinct windows remain separate
subscribers. Verified by 22 UI tests, UI production build and the fast gate.
One regression keeps directory refresh unresolved and requires the graph to render;
another proves two simultaneous identical watches issue one native request.

## Audit item 6 — completion/change wakeups

Implemented and verified by the notification-driven application-host test and fast gate: command
completion and graph settlement notify the scheduler and conversation watchers;
retained streaming observations notify watchers. The scheduler notifies watchers
after draining a signalled change so adoption is visible promptly. Waiters register
before checking state to retain notifications arriving during the check. Idle
scheduler ticks do not broadcast extra watcher wakeups.

Notifications are hints only: the existing graph claims and workspace revision
remain authoritative. The 100 ms scheduler and 150 ms watcher timers remain as
fallbacks for other mutation paths. No claim that every path meets 50 ms is made.
The application-host test now waits on completion notifications instead of its
own polling loop while exercising reply and requested translation publication.

## Audit item 7 — reuse already displayed attempt evidence

Reopening the same attempt at the same exact revision reuses its loaded result.
Changing the revision/attempt still reads authoritative evidence; failed selection
does not prevent returning to another attempt. No historical evidence is relabelled
as a different revision and no additional response cache is introduced. First
inspection credit retains its native validation. Six UI regressions, build and
fast gate passed. Verified reduction: same-cut reopen adds zero native reads.

## Audit item 8 — presentation work follows visibility and artifact identity

Graph layout memoization uses native artifact identity, which binds immutable
topology, rather than the freshly deserialized object reference. Node states still
come from each current snapshot. Response details serialize only while expanded,
and unchanged expanded values reuse their formatted text. Workspace run polling
stops when its own or surrounding activity section is collapsed, then rereads on
opening. The selection remains mounted while hidden.

Verification: activity/response-details suite 55 passed; the subsequently added
artifact-identity regression passed with all five graph tests; UI build and fast
gate passed. Tests count one layout for repeated identical artifact identity, zero
serialization while closed, and no further workspace polling while hidden.

## Audit item 9 — admission from matching resident state

Implemented and verified by 77 native workflow regressions, the focused count/stamp test and fast gate: admission reads the loaded native
engine when its full stamp matches persisted storage. A mismatch fails explicitly.
One shared pending-count function handles full and redacted artifacts. Engines
without a loaded implementation retain persisted reconstruction so unavailable
catalogs still count toward admission. There is no cached counter to invalidate.

```sh
cargo test --manifest-path native/Cargo.toml --lib admission_current_state_timing -- --ignored --nocapture
```

The timing test compares both supported paths on the same pending partner run and
asserts equal counts on every sample. A separate regression compares counts and
requires rejection of a changed stored stamp. Queue limits and ancestor rules are
unchanged; all admission callers pass their existing runtime under store ownership.

## Audit item 10 — ready audio starts independently of inspection

Fetched reading/Drill audio starts playback concurrently with its inspection.
Inspection failures remain reported and stop the owned playback; interruption
still prevents late fetched audio from starting. Late spectrum arrival no longer
resets the playback cursor. The pending/empty-stream polling waits remain: those
branches have no ready samples to play and need a bounded wait until native
stream delivery provides data. No poll-frequency increase was introduced.

Verification: the pending-inspection playback regression failed before the fix,
then passed; 61 audio/Drill tests passed, with the additional inspection-failure
regression passing in the five-test audio suite. UI build and fast gate passed.
This removes inspection time from audio-start dependencies; no whole-app timing
saving or audio-device latency claim is made.

## Audit item 11 — helper admission and failure display

Helper commands no longer reconstruct the conversation before admission. Native
admission already checks source ownership and makes duplicate demands/retries of
active work no-ops transactionally. UI help lanes still avoid requests when
opening saved results. The workspace read remains for the command session identity;
this change removes one full conversation read per explicit helper request, not
all command reads. No automatic retry was added.

IPC errors start redacted diagnostic recording immediately and deliver the original
failure without waiting for durable logging acknowledgement. Diagnostic delivery
still tracks pending/failure state and surfaces failed persistence. The workflow
IPC timing boundary now excludes diagnostic persistence after a rejected command.

Both unresolved-read regressions failed before the fixes, then passed. Thirty
helper/analysis/IPC UI tests, production build and fast gate passed. Native source
validation and no-op handling were reviewed; final native regressions cover the
unchanged admission behavior. No end-to-end latency percentage is claimed here.

## Audit item 12 — final verification and prepared app

All twelve items in this focused correction checklist are complete. This is not
acceptance of the overall 50 ms p95 product targets. The complete learner-session
replay remains unresolved; original observations and focused comparisons are kept
separate. Timings from overlapping operations have not been summed into a claimed
whole-app saving.

| Final gate | Result |
| --- | --- |
| Native library suite | 938 passed, 5 ignored; ignored timing tests were run explicitly where reported above. Supported-version migrations, rollback and repeated startup included. |
| Native Clippy, library/tests, warnings denied | Passed. |
| Native binary checks and generated contracts check | Passed. |
| UI suite | 1,956 passed across 290 files. |
| UI TypeScript and production build | Passed. |
| Fast gate on final source/test state | Passed. |
| Capture-enabled native workflow tests | 13 passed. |
| Workflow tooling types/tests | Passed, 32 tests. |
| Documentation links | Passed. |
| Isolated desktop capture/replay smoke | Passed on the prepared build; no provider calls. |

The initial full UI run found two fixture issues: mocked conversation long polls
never settled between tests, and a reading-help assertion inspected collapsed
response details. Tests now settle their pending reads after unmount and open
response details explicitly. The complete suite passed after those corrections.
The final audio check also verifies cancellation after playback ends while
inspection remains pending. Existing jsdom canvas and bundle-size warnings remain;
these checks do not establish actual device latency.

### Exact build and app check

Prepared executable: `.local/workflow-build/skellyspeak.exe`, SHA-256
`f5766ee3d698d01a3b506aa66f86cf3d1d3afbc57a4e86bd07e7a2f046eea7e2`.
Executable plus served UI build identity:
`8121905bbc296e7dfa4c93d357907494f40ad0ef8a565f02425cb8e2acefbdfb`.
The ordinary prepare command linked successfully but could not replace the running
development executable on Windows. Its newly linked `native/target/debug/deps/skellyspeak.exe`
was copied into the isolated build location and verified through the actual
capture/replay smoke test. The user's running app and workspace were not stopped
or reset. Future preparation requires closing that development executable if
Windows still holds the same output path.

Smoke capture: `.local/workflow/c80b8e32-5550-40db-a6db-2dfdca735aec`.
Successful smoke replay: `.local/workflow/bde79c84-ec22-4fb8-95ec-1a4fcdffe98d`.
This onboarding smoke proves launch/capture/replay wiring, not the full learner
conversation/audio workflow. Start the prepared app with `npm run workflow:record
-- --mute`. Sign in/configure before starting capture. Then inspect:

1. Open saved suggestions/grammar and repeatedly close/reopen them.
2. Send a conversation turn; open and select activity nodes while work runs.
3. Play speech and Drill reference audio; inspect its spectrum during playback.
4. Request help, observe a failure if one occurs, then explicitly retry.

Report remaining visible waits and save the capture. New observations can be
compared descriptively; paired whole-workflow savings require a successful matching
replay and repeated runs. No commit, release or deployment was performed.

### Advisory source metrics

The existing syntax-count tool produced `.local/workflow/complexity-candidate.json`.
Baseline/candidate: 1,104/1,105 source files, 7,085/7,101 TypeScript functions,
10,855/10,889 counted decisions, and 122,803/123,001 source lines. These are source
hygiene observations, not a complexity improvement score, formal cyclomatic
complexity, Big-O, or latency predictions. Rust receives line counts only; its
colocated tests remain in those counts. The new runtime helpers and regression
coverage can increase source counts while removing repeated runtime work.

## Learner recording follow-up — 2026-10-10, 17:13 local

Inspected session `18905c08-e69c-4bcd-88c4-f0eafffaf2d1`. This is an incomplete
capture, not a verified replay or a before/after UI timing result. `incomplete.json`
reports `Desktop evaluation failed; capture is not complete.` No exported
`recording/session.json`, UI latency report or replay result exists. Native tape
and diagnostic logs survive; no app source was changed during this inspection.

The native timing decoder accepted all 96 complete records, spanning 202.992 seconds:

| Boundary | Count | p50 ms | p95 ms | Maximum ms |
| --- | ---: | ---: | ---: | ---: |
| Protocol transport | 45 | 153.516 | 403.416 | 986.128 |
| Text transport | 41 | 1981.384 | 5379.911 | 6596.382 |
| Speech transport | 3 | 1345.446 | 2283.478 | 2283.478 |
| Transcription transport | 4 | 1304.500 | 2208.582 | 2208.582 |
| Microphone capture | 3 | 3715.601 | 13891.337 | 13891.337 |

Transport durations include their provider/local adapter boundary; microphone
durations are capture time, not processing latency. There were seven overlapping
transport spans at peak, excluding microphone capture. No transport error frames
were found. This does not establish downstream semantic acceptance or UI paint/
audible playback latency. Summary saved locally as `partial-native-timing-summary.json`.

Launch-session diagnostics contain 271 language-total IPC starts and 148 skill-
evidence starts, plus 50 each for reward settings, playback rate and microphone
settings. These counts include setup before capture and cannot be treated as
capture-only rates or proof each call was unnecessary. They support investigating
statistics/evidence invalidation and settings refresh fan-out next. The log has
no correlated per-request timing endpoints suitable for reconstructing UI latency.

One browser ResizeObserver loop notice occurred at 21:12:47.525 UTC. The recorder's
browser observation handler marks every `error` event as failure; final checkpoint
throws if any such failure exists. The app diagnostic handler specifically treats
resize-observer-loop notices differently. This is a likely explanation for final
save failure, not a confirmed exact exception: the CDP adapter discards exception
details. A fetch warning and reload callback warning were also recorded earlier.

The save path exports only after the final checkpoint succeeds. Its failure path
writes a short incomplete marker but does not persist the already collected action
and timing bundle. Consequently this broader exercise cannot serve as a complete
reproducible baseline despite useful native evidence. Next recorder repair should
preserve incomplete events/assets, retain safe failure-stage diagnostics, and
separate known browser notices from genuine checkpoint failures. Do not ask for
another thorough recording until that path is repaired and verified.

Recorder follow-up implemented: the save boundary now preserves partial evidence,
records its failure stage and treats the known resize delivery notices consistently
with the application. Tooling regressions and real desktop success/failure checks
passed; see [recording verification](workflow-recording-replay.md#recorder-save-reliability--2026-10-10).
The next learner recording can provide local IPC/frame measurements for the
remaining statistics/evidence refresh investigation. The previous session's
missing UI events/timings remain unavailable.

## Refresh fan-out follow-up — 2026-10-10

Implemented source corrections while awaiting another learner recording. The
271 language-total and 148 skill-evidence starts in the incomplete session are
launch-session observations, not counts of proven redundant calls.

Trace: TopBar, XpOverview and EffortReport independently request all-language totals,
with evidence and effort changes as dependencies. Conversation snapshot revisions
request skill-evidence reloads; the shared evidence store already coalesces pending
reload bursts into one trailing read. Profile save returned a complete snapshot
that the store ignored before requesting another one. Settings load composes six
native reads; startup, conversation selection, settings dialogs and execution-
preference notifications can request that same projection together.

Required refreshes remain: conversation revisions may carry newly adopted evidence,
settings scopes change with conversation selection, and explicit writes may arrive
while a read is pending. Without a dedicated authoritative learning revision,
suppressing conversation-driven evidence reloads based on guessed node states would
risk stale results. No such heuristic or timer throttle was introduced.

Corrections and verified request-count effects:

| Path | Before | Implemented and tested |
| --- | --- | --- |
| Three totals requests in one synchronous burst | Three native reads | One native read, same results for all callers. |
| Three settings requests in one synchronous burst | Eighteen native reads | Six native reads, one shared projection. |
| Profile save with unchanged selected scope | Save plus another evidence read | Adopt the native save result; zero extra evidence reads. |

The shared batchRead helper clears its scheduled promise before dispatch. A later
invalidation starts a new read even while the previous request is pending. It adds
one microtask, no timer and no completed-result cache. Failures reach all callers;
subsequent reads recover normally. Existing native write validation and latest-read
publication rules remain unchanged. A profile save cannot replace another selected
scope; if the selection or pending invalidation changes, the store reloads instead.

These bounded corrections do not claim that the remaining conversation-driven
reads, or all 271/148 observed calls, have been eliminated. Future recordings must
measure actual frequency reduction and user-visible latency separately.

Refresh follow-up checklist and verification:

- [x] 1. Trace language-total reads: identify callers, triggers and overlapping requests.
- [x] 2. Trace skill-evidence and settings reads: identify callers, triggers and overlapping requests.
- [x] 3. Classify refresh requirements: distinguish necessary updates from reads triggered by unrelated changes.
- [x] 4. Fix language-total refreshes: add request-count regressions and verify the affected behavior.
- [x] 5. Fix skill-evidence refreshes: add request-count regressions and verify the affected behavior.
- [x] 6. Fix settings refreshes: add request-count regressions and verify the affected behavior.
- [x] 7. Complete verification and update the tally: run the relevant gates, record eliminated work and separate it from unmeasured UI latency gains.

Final verification: 1,960 UI tests across 291 files passed; final TypeScript/UI
production build, fast gate, documentation links and diff whitespace check passed.
The full suite preceded only import relocation; final build/fast checks include
that relocation. Existing jsdom canvas and bundle-size warnings remain. Native
code and persisted contracts were unchanged, so native suites were not repeated.
The prepared recorder serves the updated UI build on its next launch. No new
learner recording or whole-workflow latency comparison was performed, and all
changes remain uncommitted.


## Complete learner capture and next measurement batch — 2026-10-10, 19:30 local

Capture `3cd58ba4-8b39-418c-b8ce-4771fad9843b` passes bundle integrity validation:
272.947 seconds, 8,386 actions, 5,447 timing events, and 132 complete native calls.
Content-free distributions are saved beside the capture in `measurement-summary.json`.
Replay `1aa3432e-64fe-434c-be93-83cf991a0144` stopped at wheel action `action-159`
(3.908 seconds), awaiting recorded application state. No unexpected-input marker
was retained. This capture is complete; whole-workflow replay remains unverified.

| Observed boundary | Samples | p50 ms | p95 ms | Maximum ms |
| --- | ---: | ---: | ---: | ---: |
| Workspace snapshot IPC | 194 | 139.8 | 1598.6 | 3393.2 |
| Skill evidence IPC | 156 | 145.7 | 1113.7 | 2406.6 |
| Language totals IPC | 277 | 91.4 | 1056.5 | 2411.2 |
| Speech audio read IPC | 42 | 218.3 | 1143.1 | 1822.6 |
| Execute command IPC (mixed operations) | 26 | 296.1 | 1785.4 | 2223.6 |
| Next-frame callback | 487 | 1.9 | 20.2 | 98.6 |

These are observed IPC round trips, including possible native queues/locks and UI
callback delivery, not isolated SQL timings or visible-result measurements. The
frame callback does not prove content painted. Intentional conversation long polls
are excluded from the local latency ranking; one watch endpoint is absent at stop.
Microphone durations represent the user's recording time. Transport peak overlap
was eight (excluding microphone capture); this proves some concurrency, not optimal
scheduling or immediate publication. Text transport p50/p95 was 2043.94/4339.78 ms;
protocol transport 164.53/415.12 ms. Provider waits remain separate from local work.
Two native records ended by cancellation; cancellation alone is not an error.

A reading admission error reported `Finish or close pending reading requests first.`
Its request lifecycle needs investigation. High call counts identify candidates,
not proof that every call is redundant. This workload differs from earlier learner
sessions: differences between their distributions must not be advertised as savings.

Next linear checklist (planned except the completed measurement item):

- [x] **1. Validate the capture and save baseline measurements and limits.**
- [x] **2. Fix the recorded scroll replay failure without weakening correctness checks.**
- [ ] **3. Repeat the unchanged workload to establish replay reliability and timing variance.**
- [x] **4. Measure where slow local requests wait: dispatch, locks, native work, and UI delivery.**
- [x] **5. Fix shared blocking or contention demonstrated by those measurements.**
- [x] **6. Reduce unnecessary workspace snapshot reads and projection work.**
- [x] **7. Reduce unnecessary language-total reads and recomputation.**
- [x] **8. Reduce unnecessary skill-evidence reads and recomputation.**
- [x] **9. Remove avoidable delay from cached speech delivery and playback startup.**
- [x] **10. Remove avoidable local delay from Drill navigation and saved-result inspection.**
- [x] **11. Fix the recorded pending-reading request failure.**
- [x] **12. Trace grammar analysis from the coach panel and message bubble; share equivalent work and immediately display existing results.**
- [x] **13. Fix Skills -> Ask the Coach so it opens the intended coach conversation with the requested context.**
- [x] **14. Document verified read, refresh, cancellation, and publication contracts; remove superseded code from these fixes.**
- [x] **15. Run final regression gates and repeated measurements; report improvements and remaining acceptance gaps.**

For each implementation item, establish a reproducer and same-workload measurement,
make one coherent correction, run affected regressions, and append counts plus
p50/p95/max before and after. Repeat trials and retain failures/unmatched samples.
Do not sum overlapping request durations as elapsed user-time savings. Attribute
improvements per boundary; measure visible completion separately. Replay variance
and instrumentation overhead must be disclosed. If full replay meets a distinct
blocker, preserve the exact failure and use a faithful focused scenario for an
independent correction; do not fabricate a full-session pass or demand another
learner recording prematurely. No additional optimization is implemented by this plan.


### Checklist item 2 — scroll replay correction

The original hit-test failure was reproduced at action-159; failure diagnostics now
identify the awaited boundary and retain content-free target/hit geometry. Wheel
replay delivers the exact recorded event to the exact recorded target and applies
recorded scroll positions instantly. It does not additionally simulate browser
wheel inertia. Wheel hit validation follows the shared scrollable ancestor chain,
covering nested text areas in a scrolling dialog; independent scroll surfaces are
rejected. Pointer click hit tests remain exact. Regression tests cover those cases.

Workflow tooling: 37 tests/types passed. The recording-controls desktop smoke
passed (capture e9ad9f55-3254-465a-b641-fea171ecc19e; replay
4671a513-d966-4aff-8620-267014e282da). Full learner replay
84bc6f46-68d2-42bb-b96f-bfe856def014 passed the previously failing wheel gestures,
then reported native replay_request_not_found at 51.100 seconds after native
startup. It ultimately stopped at action-1907 awaiting an absent target. This is
not a successful full replay or performance comparison. Item 3 remains open until
full-workload repeatability is established; independent focused measurements may
proceed under the previously documented fallback if that mismatch persists.

The audit note was restored from this session's recorded edits after an encoding
error during a checklist update truncated it. The restored note retains all twenty
sections and prior measurements; subsequent writes use explicit UTF-8.


### Focused read profiling (item 4)

Implemented opt-in capture-build store-lock measurements (caller, wait, hold;
no content). Samples are bounded at 100,000 with explicit overflow. The store
lock is released before storing measurements. Inactive capture uses an atomic
flag; ordinary builds contain no recorder. The isolated profiling harness restores
a read-only SQLite backup of the completed recording workspace and supplies an
empty provider tape, so it cannot contact a live provider. Source data is unchanged.

Two runs use the same input database and 20 serial plus 20 concurrent repetitions
of snapshot, totals and evidence commands. Baseline build:
`d525f81b850a3b54a77b6abba23685a084e09d41e70de48749405a5f3458ea44`.
Evidence: `.local/workflow/4982f151-68c6-43bc-92f0-292c8b071cf5/local-read-profile.json`
and `.local/workflow/1ac450f0-d67b-48bd-89a3-ea01c49be48b/local-read-profile.json`.
First run totals lock hold median 128.84 ms, p95 157.61 ms; evidence hold median
6.45 ms, p95 8.03 ms. Concurrent IPC medians 161-162 ms. Frame interval maximum
550 ms. Startup/background reads also appear in native samples; they are grouped
by caller and are not claimed to correlate one-to-one with injected requests.
IPC includes dispatch and delivery; hold time includes all work inside the guard,
not just SQL. Frame intervals are not visible-result paint timing. The focused
workload is not a full-session replay acceptance receipt.

Verification: instrumentation unit test, 37 workflow tooling tests and types,
fast gate and capture executable build passed. Full native gates remain due for
the implementation batch. Item 3 remains blocked by a repeated provider request
mismatch; no full-workflow savings are asserted.


### Worker dispatch (item 5)

Skill evidence and language totals now dispatch blocking store work via
`spawn_blocking`, preserving their projections, errors and single-store ownership.
Focused workload evidence: `.local/workflow/b61b6c22-3b1f-4cf8-8aa8-45465d66238c/local-read-profile.json`.
Concurrent snapshot median/p95 changed from 162.4/178.3 ms to 12.9/16.9 ms.
Totals median remains 121.1 ms; evidence median remains 129.3 ms because the same
store lock still serializes database work. Maximum frame interval remains 466.7 ms
near startup. No claim of eliminating all dispatch or frame stalls. Actual IPC
workload, executable build and fast gate passed; full native gates remain due.


### Workspace read batching (item 6)

`readWorkspace` coalesces synchronous bursts using the existing `batchRead` owner.
Three consumers cause one native projection. Later invalidation during an in-flight
read dispatches afresh; errors do not remain cached. No durable snapshot cache or
weakened native validation. Regression: 52 IPC/conversation tests; UI build and fast
gate passed. Count reduction is measured by controlled dispatch tests, not inferred
as a percentage of all recorded snapshot calls.


### Compact language totals (item 7)

Totals read the durable ledger directly, scan assessment contexts once, construct
the shared catalog once and use one consistent read transaction. Existing caller
transactions remain owned by the caller. Credit decoding, catalog validation,
duplicate rejection, exclusions, registry identity and checked totals remain active.
No writes, cache or migration. Assessment-context scans decrease from 21 to one
for this workspace; this is an operation-count result, not a whole-app complexity
claim. Effort remains owned by its existing projection.

Final focused evidence: `.local/workflow/21b7b25c-1ed6-4557-b00d-79131fcd79b2/local-read-profile.json`,
build `17e626fa6ce81e2d1729470658e0a328ebcacbbfdce92ec20da9d05317154004`.
Totals lock hold median/p95: 128.84/157.61 ms baseline, 10.74/13.01 ms now.
Concurrent IPC median/p95: snapshot 15.6/18.8, totals 24.2/29.7, evidence 34.3/41.1 ms.
All 21 languages matched detailed evidence XP/conversation counts in the running
isolated app. Maximum startup frame interval still 533.5 ms; this remains open.
Focused totals tests and fast gate passed. Full native suite was interrupted before
completion to serialize Windows test-executable use; it is not a passing gate and
must run on the final batch state.


### Evidence computation (item 8)

Guide edition selection now runs once per skill per read, with rendering still
performed for each variety. Catalog-version derivation reuses the already computed
construct hash. No persistence or invalidation changes. Fifteen learner regressions
and the fast gate passed, including totals ownership/rollback coverage. Additional
latency contribution is not yet isolated; repeated final measurements remain due.


### Speech delivery (item 9)

Playback polls use authoritative `run_control` for active state. Pending reads no
longer materialize inspection history and response diagnostics only to discard
them; failed/unknown outcomes retain those details. Speech delivery and cached
reading audio run on blocking workers. Stream PCM is copied under ownership locks,
then encoded after releasing them. No synthesis, ownership or eviction policy change.
Verified: partner-host integration; 15 native reading tests; six playback UI tests;
fast gate. These establish behavior, not measured audio-start latency savings.


### Drill local reads (item 10)

Practice-set, item-list, last-item and saved-audio reads run on blocking workers.
Saved-audio encoding happens after releasing the store guard. Retained attempt
audio reaches playback before waveform inspection completes; failed inspection
keeps playable audio and reports its error. Existing hold/deletion behavior remains.
Verification: 41 native Drill tests, affected UI run (66 tests), additional control
regression (eight comparison tests), UI build and fast gate passed. Timing claims
remain limited to measured workloads, not inferred playback-start savings.

### Recorded reading refusal (item 11 investigation)

The copied recording contains eight gloss receipts created between 23:28:25.669Z
and 23:28:25.714Z, all successful by 23:28:30.093Z. The intervening refusal at
23:28:27 matches the independent eight-entry volatile reading registry cap.
Provider network admission is separately bounded at 32. Removing the local
consumer-count cap preserves per-input bounds, expiry, ownership and cancellation;
it does not alter network admission or initiate automatic retries.


Reading refusal fix verified by 11 native reading tests (one manual benchmark
ignored) and fast gate. Regression admits and claims 16 simultaneous consumers,
cancels one without invalidating the others, then releases all ownership. Input
bounds and provider admission remain intact.


### Grammar surfaces and Skills handoff (items 12–13)

Full-message grammar in the bubble and coach panel already uses the same
`requestExplanations` admission and saved `replyExplanations.cards`. Tests verify
that both surfaces render saved grammar immediately without another request.
Selected-text explanation has a different source identity and remains a distinct
request. No additional grammar route or cache was introduced. Verification:
29 grammar regressions, UI build and fast gate passed.

The recorded workspace contains two successful guide-coach turns with retained
subskill context. The outer coaching panel opened, but its private conversation
remained minimized. Successful guide submission now explicitly requests that the
private thread expand. Background updates do not reopen it after manual minimization;
changing conversations resets that request. The submission and its native context
capture remain unchanged. Verification: 67 affected coach/navigation/conversation
tests, UI build and fast gate passed. Native ownership and UI rendering are tested
at their respective boundaries; running-app user acceptance remains pending.

### Read, refresh and publication contracts (item 14)

Implemented contracts, with focused regression coverage:

| Boundary | Contract |
| --- | --- |
| Workspace reads | Consumers in one synchronous burst share one dispatch. The batch clears before dispatch; later invalidation starts a new read, even while the first remains pending. Errors are not cached. |
| Native read workers | Blocking database work runs on blocking workers. The store guard still serializes access to its connection; worker dispatch does not imply parallel SQLite use. Join failures retain diagnostics. |
| Language totals | One read transaction and one assessment-context scan. Existing caller transactions remain caller-owned. Credit validation, exclusions, registry identity and overflow checks remain active. Reads do not award credit or write projections. |
| Skill evidence | Shared catalog identity and guide edition selection are computed once per appropriate scope. Each requested variety still receives its own rendering. No persistent cache or invalidation policy is added. |
| Speech delivery | Playback reads adopted outputs and current source ownership. Pending polls avoid discarded history materialization; failures retain diagnostics. PCM copying remains under ownership locks; encoding happens after release. Reading never requests synthesis. |
| Drill audio | Retained audio becomes playable before waveform inspection finishes. Inspection failure preserves playable audio and reports the failure. Selection changes suppress late results; explicit retry repeats local reads without creating an attempt. |
| Reading requests | Independent consumers have independent claims and cancellation. Input bounds, unclaimed expiry and provider admission remain enforced. A local consumer-count cap does not gate accepted work. |
| Grammar inspection | Existing message grammar renders directly from saved cards. Missing results use the shared explicit request path; viewing saved cards does not dispatch another provider call. |
| Skills coaching | Native submission captures the guide and selected focus. Successful navigation explicitly reveals the private thread; subsequent background publication respects user minimization. |

Superseded work was removed at its owner: full evidence construction for totals,
repeated per-language assessment-context scans, repeated guide-edition selection,
discarded speech-history materialization and the independent reading-consumer cap.
No schema migration, alternate runtime, provider fallback or duplicated viewer
interpretation was introduced. This review does not claim every application read
is optimal; final performance acceptance uses the stated workload and limits.


### Final verification and repeated measurements (item 15)

Prepared executable plus served UI build identity:
`a2886805242c17bc68392363ca9714480c74fe14ef9d1dd48cfdec0d43b72530`.
Two muted, isolated focused runs against the same consistent workspace backup:
`6af0f48a-3b66-4999-b573-456762a29281` and
`e10002d8-7fb0-49b3-9002-8b71acf4a7c6` under `.local/workflow/`.
Provider traffic was blocked, compilation/test processes were idle, no measurement
overflow occurred, and all 21 language totals matched detailed evidence in both runs.
The aggregate is `.local/final-read-comparison.json`; each original run retains its
build identity, raw samples and native lock measurements.

Each range below is the two runs' p95 values, with 20 samples per command/mode/run.
These ranges are repeat observations, not confidence intervals.

| Focused command/mode | Baseline p95 ms | Current p95 ms |
| --- | ---: | ---: |
| Concurrent workspace read | 165.3-178.3 | 15.9-17.0 |
| Concurrent language totals | 163.8-177.1 | 22.9-26.0 |
| Concurrent skill evidence | 164.1-176.9 | 32.4-35.2 |
| Serial workspace read | 77.9-91.0 | 31.4-33.4 |
| Serial language totals | 164.0-276.3 | 29.4-36.5 |
| Serial skill evidence | 140.3-152.8 | 25.8-35.0 |
| Native totals store-lock hold | 135.08-157.61 | 12.69-13.39 |

The concurrent p95 reduction is approximately 148-162 ms for workspace reads,
138-154 ms for totals and 129-145 ms for evidence when comparing conservative
range endpoints. These overlap and must not be added as wall-clock savings.
The recorded whole-session 1-1.6 second observations are a different workload and
are not used as this comparison's baseline.

Remaining responsiveness gaps: serial IPC maxima still reach 129-181 ms around
startup; maximum frame intervals are 466.7 and 516.6 ms. Frame p95 is 83.3/83.4 ms
in these shorter runs. Faster completion shortens the measurement window (79 frame
samples versus 417-418 baseline samples), so its frame distribution must not be
presented as a matched steady-state rendering regression or improvement. The
startup stall remains real and unresolved. Visible content hydration and provider
completion-to-paint are not causally measured by this harness. No full-workflow
responsiveness acceptance is asserted.

Final automated verification:

- Native capture-enabled library: 957 passed, five ignored, zero failures; includes
  migration paths and repeated startup (500.91 seconds).
- UI: 2,020 passed across 297 files with two workers. The initial overlapping broad
  run had three test timeouts and one lazy-load lookup timeout; the full isolated
  rerun passed without changing assertions or timeouts.
- Clippy with capture feature, library and tests, warnings denied: passed.
- Production binary checks and generated-contract checks: passed.
- UI production build, capture-enabled executable build and preparation: passed.
  The frontend bundle-size advisory remains.
- Workflow tooling: 37 tests plus types passed. Complexity tooling: two tests plus
  types passed. Advisory report saved as `.local/final-complexity.json` (1,120
  files, 7,221 TypeScript functions; Rust line counts only).
- Final code-state fast gate, documentation links and Git whitespace checks: passed.

Item 15 completes verification/reporting of this batch. Item 3 remains unchecked:
full-session replay still has a provider-request mismatch. Stage 12 user acceptance
and the startup/visible-hydration performance budgets remain open. The prepared
recorder is ready for a fresh process (`npm run workflow:record -- --mute`). Focused
user checks: Skills Ask Coach reveals the question/context, saved grammar opens
without a request, and saved Drill audio is playable before waveform inspection.
Changes are uncommitted; no release, version change or deployment occurred.

## PR readiness: startup checkpoint

- [x] 1. Startup layout fixes and measurements. Cold WebView frame gap remains a
  documented acceptance limitation; it has not been eliminated.
- [ ] 2. Resolve full-session replay reliability.
- [ ] 3. Verify the activity agent's release handoff without editing its UI.
- [ ] 4. User final app acceptance and reported-defect repairs.
- [ ] 5. Complete branch review.
- [ ] 6. PR and final CI after commit authorization.
- [ ] 7. Actual release-package validation.
- [ ] 8. Chosen version and publication after release authorization.

The release-readiness sequence is startup responsiveness, full-session replay,
AI activity agent handoff, user acceptance, branch review, PR/CI, package
validation, and separately authorized version/publication. Activity UI changes
belong to the other agent. No commits or version changes are authorized.

Implemented and verified startup fixes:

- Completed conversation bubbles do not read geometry to establish hydration
  height floors. Hydrating/inspected bubbles retain their existing behavior.
- Toolbar fitting coalesces resize, content and font notifications into one
  animation-frame pass and avoids redundant attribute writes. Initial fit stays
  synchronous; pending work is cancelled on unmount.
- Both regressions failed before the fixes. All 65 affected UI tests passed;
  the production UI build and fast gate passed. Workflow tooling has 37 passing
  tests and passes its type check.

The opt-in local-read profiler now separates startup from repeated reads, records
bounded content-free trace metadata, renderer CPU profiles, native store timings,
frame intervals and event-loop delay. Warm reload readiness requires a changed
performance.timeOrigin, preventing accidental measurement of the outgoing page.

Build: 39fcaa1c8bde10536db9f42265c5b9d23f1bda3814c9be3a5e8f1bc5e72ae0d3.
Verified warm reload: 75717a5d-0546-4539-8880-cc2c88bbc458. Frame p95 16.8 ms,
maximum 33.4 ms (208 samples); event-loop delay p95 1.3 ms, maximum 37.7 ms
(338 samples). Earlier warm repeats were consistent (frame maxima 50-50.1 ms),
but lacked the explicit document-identity readiness assertion.

Limit: fresh isolated WebView launches still show frame gaps around 450-583 ms.
This is not eliminated. Trace evidence includes font decoding, initial layout and
a long task in a separate browser process; it does not identify a single proven
root cause for the full gap. The measured DOM was an empty conversation with
393 elements, not a large saved conversation. An offscreen-content experiment
was removed because it did not address the measured workload. Warm reload numbers
must not be presented as proof of improved cold startup or click-to-paint latency.

### PR readiness: replay checkpoint remains open

The persona-sampling identity defect is fixed and tested through actual application
commands. Native tests: 959 passed / five ignored; Clippy and production binary
checks passed. Workflow tooling: 38 tests/types passed. A fresh controlled-provider
capture and offline replay passed. The detailed receipts and root-cause evidence
are in workflow-recording-replay.md.

The repaired copy of the full recording now passes its original provider mismatch,
but stops at a missing positional microphone-button selector around 120 seconds.
The recorder now prefers unique accessible control labels. Full-session acceptance
requires a fresh recording using the current layout. Item 2 remains unchecked.
The concurrent Activity UI work currently blocks the fast gate with missing
translation keys; no Activity UI file was edited in this batch. No PR, commit,
version change, tag, publication or deployment was performed.

## Read/write amplification review — 2026-10-11

This pass continues the completed Activity-panel work at commit `05783658`.
It is a measured performance batch, not a replacement graph architecture.

Implemented changes:

- Original capture still commits every chunk and retains the same bytes, digest,
  state and eviction provenance. Begin/Finish and appends without new blob bytes
  no longer run global maintenance. New bytes check capacity; overflow retains
  the existing eviction policy. Pruning no longer counts reusable results merely
  to obtain the byte budget. Format 75 adds five reference indexes; no historical
  row is rewritten.
- Conversation speech delivers and authorizes every PCM chunk but persists a
  diagnostic observation only when the evidence changes. A failed persistence
  operation is never remembered as success. Completion/settlement is unchanged.
- Workspace identity-only checks use the existing narrow learner-ID query instead
  of building full workspace/UI snapshots, including speech/transcription loops,
  recordings, generation, access and reading-cache identity.
- Inspection cache retention uses its 16 MiB budget, including key and entry
  overhead, instead of a 32-entry ceiling that cannot hold a 50-recording page.
  Keys and ownership/corruption validation are unchanged.
- Conversation message projection resolves each run's source statuses once within
  one protected page read. Turn projection reuses its authoritative owner lookup.
  No cross-request cache or alternate runtime interpretation was added.

Regressions reproduced before implementation: a streamed append alongside 1,000
retained chunks used 2,043,365 SQLite VM instructions (actual bundled SQLite and
production write path); the 50-receipt cache test failed on the second sweep.
Instrumented VM elapsed time is not an application-latency measurement.

Measurement method: isolated workflow desktop runner; disposable copies of
`.local/activity-diagnostics-workspace-20261011`; empty provider tape and network
verification; same 19 messages/10 turns; ten forced conversation reads plus five
queued workspace reads. No compiler or test process runs during timing. The
ignored `.local/review-conversation-profile.ts` harness retains content-free
latencies, output digest and native lock timings with the executable/UI identity.
The saved seven-action recording is replayed independently to verify actual
provider-tape, rendered and playback checkpoints. That short recording does not
represent every learner workflow or prove click-to-paint acceptance.

The first probe exposed a stale prepared executable (`dd87dbd...`); its 886–1,083 ms
results are excluded. A freshly compiled committed baseline (`bc45bde...`) measured
191–208 ms per conversation read and 150–160 ms for queued workspace reads
(receipt `.local/workflow/9c027ad0-6321-43a9-9f6d-64ebbccd0672`). Baseline saved-session
replay passed at `.local/workflow/795f5c2d-dfbb-413e-83f2-7d7bbb5f0f53`.

The fixed append uses 4,240 VM instructions (99.79% less). The same regression
verifies retained output. A fresh-workspace identity lookup uses 10 VM instructions
versus 74 for the full snapshot; larger workspace projections do additional work.
The cache regression retains all 50 small receipts through repeated sweeps. Speech
observation coverage verifies 1,000 identical metadata emissions persist once,
changed metadata persists again, and failed persistence remains retryable.

Native capture-enabled library verification: 992 passed, zero failed, five ignored
(725.44 seconds). This includes supported migrations, rollback/reopen, original
retention, corruption rejection and failed/retried source-status parity. Clippy
with the capture feature, library and tests, warnings denied: passed. Final
code-state fast gate, production binary checks and generated-contract checks:
passed. Documentation links and whitespace checks passed. No UI source changed.

### Final alternating desktop measurements

Two fresh baseline/candidate pairs ran sequentially, with no builds or tests in
progress. Each variant has 20 conversation reads and ten queued workspace reads.
Candidate combined build identity:
`17303fc1008876d7ffa0e29bbe45e9591cfc7fadb6e0e592c37bd9d6f1c01293`.
All four runs produced the same conversation output SHA-256:
`5ffefcde7a570949149a1c13a8c24ca9721ea561de2024cacb39681aa7a33343`.

| Measurement | Committed baseline | Candidate |
| --- | ---: | ---: |
| Conversation read median | 420.10 ms | 386.90 ms |
| Conversation read p95 | 447.30 ms | 445.90 ms |
| Queued workspace read median | 355.55 ms | 327.30 ms |
| Queued workspace read p95 | 382.00 ms | 387.70 ms |
| Stream append, 1,000 retained chunks | 2,043,365 VM steps | 4,240 VM steps |

Interpretation: the targeted database-work reduction is established. The desktop
median improved about 8%, but the tail did not improve reliably. The earlier
191–208 ms pilot and these slower matched runs also demonstrate substantial
session/environment variability. Do not combine that pilot with candidate samples
or claim consistent sub-300-ms reads, general UI responsiveness, or a measured
click-to-paint improvement. These samples justify further profiling, not a claim
that conversation latency is solved.

Content-free aggregate: `.local/review-performance-comparison.json`. Raw paired
receipts are `.local/workflow/f29d18ce-f2cc-4efb-9e77-b4a55bf08992`,
`dce1654e-717e-4268-a620-eb86957965a2`,
`aa1e4644-e08b-451f-81d9-2c066eb6b642` and
`ecb848cf-6487-40f5-b19e-6555eaf440f6`.

The candidate saved-session replay passed all seven actions, tape consumption,
rendered and playback checkpoints at
`.local/workflow/ebc1082b-213c-43e9-ae9d-0dbd0c8daeb9`. Comparing the two isolated
application databases verified identical original-content metadata, lengths and
SHA-256 digests, and recomputed digests from ordered retained chunks: complete
5,083-byte request and 610-byte response. The restored-workspace directory is the
input backup; verification reads the final isolated application database.

Changes remain uncommitted. No application release version, tag, publication or
deployment changed. The prepared workflow executable is the tested candidate;
the normal running application was not replaced.

Remaining targets, not implemented or claimed fixed: prepared-history caching
above the protected core with explicit mutation invalidation; repeated history
capture across recordings sharing an engine; scheduler rescans; full checkpoint
serialization per useful transition; incremental byte accounting and eviction
batching for very large caches. Stamp-only caching is insufficient: same-stamp
payload corruption must still invalidate and fail. These require separate
measurements and regression evidence, not speculative cache layers.

Separate PR-readiness finding (static, not dynamically reproduced): changing AI
access while a partner helper is in flight can leave its turn marked `succeeded`
without cancelling the graph. A late successful helper can reach publication,
fail captured-access validation, and propagate through the scheduler to
`Application::stop`. This is not introduced or repaired by this performance
batch. Reproduce and handle authority revocation as a local execution outcome
before treating the branch as merge-ready.
