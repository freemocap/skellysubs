# Private AI content retention and lineage review

Date: 2026-10-11. Status: source/history audit followed by local database inspection,
real desktop replay and timing measurements. Findings and proposed repair sequence,
**not an implemented retention or lock-contention repair**.

## Review requirement and scope

The learner owns their microphone measurements and purchased computation.
Private application records must retain source content and purchased exchanges,
with an inspectable path from each displayed AI element through its transformations
back to its inputs and producing execution. Diagnostic/log redaction is a separate
projection and must not substitute for private content retention.

The user explicitly restated this requirement in this review. It supersedes any
interpretation of content-free diagnostic rules as a ban on private source records.
Bounded LRU payload retention is acceptable in principle; it must be explicit and
must not erase durable provenance or present eviction as content never existing.

Reviewed current shared checkout at HEAD `c3dd038f`, including uncommitted activity
and conversation-data changes. Historical reference: `6ed72747` (`e952cbbb^`),
then graph integration/cleanup through `e84b44e2`. This is a system audit, not a
claim that every finding was introduced by the most recent UI diff. Historical
code was read, not run or restored. The initial source audit did not inspect the
development database; the subsequent on-disk investigation below does. Its results
supersede any implication that development recordings or all original text were lost.

## Conclusion

There are both real retention gaps and inspection regressions. Native graph
records do retain inputs, results, identities and evidence; the system has not
lost all source data. Conversation calls also capture final completion text before
domain cleanup. But private retention differs by host, exact adapted exchange
capture is missing, and the activity API excludes most retained content.
The requirement that a learner can trace every AI-generated element is not met.

## Standards findings: measurements, ownership and retention

### P1: ordinary chat microphone audio is transient

`native/src/speech/recording/transcription.rs:227` calls `results::save`, but
`native/src/speech/recording/results.rs:26` saves a WAV digest and normalized
transcription, not the WAV. Durable source audio publication is conditional on
Drill ownership at `transcription.rs:230`. Chat audio is returned to the UI at
`voice.rs:651` and held in `ui/src/platform/audio/useMicRecorder.ts:39` as the latest
transcription. Restart/replacement loses that copy. A digest cannot recover audio.
The graph's transcription source captures a digest/reference rather than audio.

### P1: messages do not retain their recording lineage

`ui/src/features/conversation/ConversationPage.tsx:378` sends text and InputEvidence
but keeps the recording association in React `sentRecording` state. At line 569,
appending another transcription replaces the single draft recording ID.
`native/src/learning/coaching/mod.rs:45` has no recording ID in InputEvidence.
Persisted recording results therefore do not provide a complete message-to-source
relationship, particularly for drafts assembled from multiple takes or edited text.

### P1: transcription retains an interpretation, not its original response

`native/src/ai/transport/transcription_provider.rs:138` receives bytes and returns
the adapter result. `transcription_adapters.rs:100` extracts transcript text,
normalizes timing and computes diagnostics. Invalid timing can become absent
at line 133. `native/src/ai/audio.rs:46` retains text/timing, not the original
response. The final multipart request is also assembled with provider-specific
fields, language mapping and prompt transformations at adapter lines 38–91.
These original request/response representations cannot be recovered from the
normalized result alone.

### P1: failed microphone measurements can disappear before durable capture

Manual failures keep only one in-memory RetryTake (`voice.rs:394`); another take
or restart loses it. Continuous capture calls transcribe directly
(`continuous.rs:341`). Failed recognition does not enter successful Drill audio
publication. This is precisely when original input is needed to diagnose failure.

These speech findings predate the panel refactor: history attributes Drill-only
WAV publication to `cd57bc82f` (September 22), independent recording result storage
to `cc0b5437e` (September 25), and manual retry memory to `3a95e18e` (September 29).
They violate the current end-to-end requirement; they are not evidence that the
latest panel change deleted audio.

## Specification findings: purchased computation and inspection

### P1: workspace structured calls lack original completion retention

Conversation host `native/src/application/graph_execution.rs:112` captures final
`completion.text` before downstream cleanup. Workspace host
`native/src/application/workspace_text.rs:111` returns the completion without that
capture. Shared `graph_request.rs:69` only captures text from deltas, while
`grouped.rs:382` enables deltas only for prose without decisions.

Affected workspace routes include structured standalone reading, guide translation,
persona proposals and Drill proposals. Successful parsed outputs survive, but the
original completion string is not retained by this path. In generation,
`native/src/ai/generation/graph.rs:246` parses the string; malformed text becomes
failure metadata, and valid text becomes a parsed candidate. Domain rejection can
therefore leave no original text to inspect. Conversation-hosted structured work
must not be conflated with this workspace-host gap.

### P1: activity cannot show retained private input/output values

`native/src/ai/inspection.rs:14` defines a detail DTO containing evidence and
retained text, not request messages, input values, result values or transformation
values. Lines 119–126 extract observations and provisional text only.
`native/src/ai/graph/projection.rs:57` deliberately omits constants; its graph
snapshot is structural. Meanwhile `native/src/ai/graph/state.rs:7` stores run
inputs and line 51 stores execution work/outcomes. Private values exist internally
but no corresponding learner inspection contract exposes them.

The structural/diagnostic projection was made the main inspection interface
without completing a separate private-content reader. This is an access gap,
not proof that those stored values were deleted.

### P1: no consistent archive of final adapted requests and raw responses

Some operations persist actual prompt messages: conversation/coach context,
proposal messages, and guide messages. Others build prompts from captured semantic
inputs (`language/translation_graph.rs:35`, `language/gloss_graph.rs:47`). Rebuilding
with code is not a saved copy of what was actually submitted.

Final payload construction occurs at `ai/transport/provider/payload.rs:168`.
Server adaptation can further change fields (`server/app/inference/contracts.py:85`).
On return, `provider/response.rs:46` decodes JSON, projects decision answers into
choices and returns text plus classified diagnostics. `provider/stream.rs:37`
reconstructs a completion from frames before this decode. Neither is a durable
archive of the original body/frame sequence. Malformed response bytes are not
preserved by the decoder's diagnostic error path.

The server does distinguish private client data from logs:
`server/app/inference/relay_payload.py:1` explicitly preserves response content
while removing secrets. `server/app/main.py:842` relays that response to native.
Thus diagnostic policy is not a universal requirement to discard client content.
The native persistence boundary needs its own content record before interpretation.

### P2: inspector loses content-capture completeness information

`ai/graph/provisional.rs:56` latches a capture-limit failure while retaining the
last accepted text. `ai/inspection.rs:120` exports only the text;
`capture_failed` refers to metadata evidence failure, not provisional failure.
The text's session, sequence and failure are omitted. A partial capture can be
shown without its actual content-capture failure state.

## End-to-end coverage inventory

| Path or data | Retained | Missing or inaccessible |
| --- | --- | --- |
| Chat microphone | Recording ID, digest, normalized transcript/timing | Durable source WAV and message-to-take lineage |
| Failed microphone transcription | Failure metadata; one manual retry take in memory | Durable original failed audio/response |
| Drill successful recording | Pending WAV then owned audio file, transcript/comparison | Exact transcription exchange; audio subject to oldest-first retention |
| Conversation/coach/provider helpers | Graph inputs/results and final completion text before domain cleanup | Exact raw provider envelope, final adapted payload, private-value inspector |
| Standalone reading/guide/proposals | Graph inputs, successful parsed outputs, classified metadata | Consistent original structured completion text; exact adapted exchange |
| Generated speech | Cached WAV; durable graph audio receipt and alignment | Uniform exchange archive and generic path through every transformation |
| Reuse/sharing | Graph producer/consumer execution identities | Complete user navigation of private content and every adoption |
| Historical legacy attempts | Schema still contains request_messages/response_text/preview_text | Current graph-only inspector does not read them |

## Historical regression and recoverability

At `6ed72747`, `conversations/execution/tests/attempt_bodies.rs` explicitly tested:

- exact outgoing prompt messages available through attempt inspection;
- original response retained while cleaned prose is published;
- invalid/failed replies retaining original response text;
- private inspection content surviving while diagnostics redact content/credentials.

`publication.rs` wrote response_text and preview_text before checking whether the
attempt could still publish. Commit `e84b44e2` deleted that implementation and the
364-line test suite. Current conversation final-text capture replaces part of the
old guarantee; current inspection does not replace its request-content access.
The old tests establish message/text retention, not byte-exact HTTP archives for
all old workflows.

Current `storage/schemas/schema.sql:28` still defines legacy attempt content fields.
Migration 71 drops separate inference cache tables, not the attempts table
(`storage/store/migrations/v71_cache.rs:5`). The documented migration ownership
review explicitly retired those cache records. Do not infer from that review that
all historical attempt contents were erased or that deleted cache content can be
reconstructed. Actual recoverability depends on the user's database, deletion
history and recovery copies; none were opened for this review. Never rerun a model
and label the new output a recovered original.

## Proposed repair order (not implemented)

1. Establish one private source/exchange/derivation retention contract shared by
   every host. Keep learner-owned content distinct from diagnostic projections.
   Preserve recording segments and edits, submitted prompt messages and effective
   parameters, received content, parsing/validation/cleanup results, adoption and
   reuse relationships. Credentials remain excluded; learner content does not.
2. Capture original input and received response at the shared boundaries before
   decoding, validation or cleanup, including failed, cancelled, interrupted and
   nonstreaming calls. Preserve both native-to-service and service-to-provider
   effective requests where adaptation changes what the provider receives.
3. Give microphone takes durable bounded content ownership before transcription.
   Persist source associations for combined/edited messages and purchased results.
4. Specify LRU semantics: what is evictable, capacity/recency behavior and explicit
   retained/partial/evicted/never-captured states. Keep provenance after payload
   eviction. Memory unloading of graph records is not disk-content eviction.
5. Add owner-checked private inspection APIs, separate from log/export redaction.
   Let each displayed AI element open its actual inputs, exchange, transformations
   and producing execution. Do not reconstruct viewer semantics separately.
6. Restore acceptance tests for every workflow/host and failure mode. Use distinctive
   fixture content to prove original and transformed values survive independently;
   prove logs exclude that content. Test restart, migration, reuse, eviction and
   navigation from the actual product element. Schema changes require reviewed,
   consecutive migrations; preserve old records and expose legacy history honestly.

Do not solve this by simply disabling redaction or storing credentials in raw
headers. Do not treat an LRU cache as permanent archival storage. The reviewed
policy must distinguish payload removal from destroying provenance.

## Verification and limits

- Read-only source and Git-history review by separate standards/spec reviewers.
- Ran `cargo test --manifest-path native/Cargo.toml --lib ai::transport::graph_request::tests -- --nocapture`:
  **4 passed**. These verify their existing transport/evidence cases; they do not
  establish complete raw exchange persistence across hosts.
- No application source changes, database migrations, native app execution,
  provider calls, private database reads, commits or deployment for this review.
- This review identifies concrete counterexamples to universal traceability. It
  does not certify every remaining path merely because it appears in the inventory.

## On-disk investigation and desktop verification

These are measurements on 2026-10-11, not promises about the repair. Source data
was read with SQLite read-only access, and SQLite's backup API created a consistent
copy at `.local/activity-diagnostics-workspace-20261011/`. Replay restored disposable
workspaces; original captures and the ordinary workspace were not rewritten. The
user authorized closing the running development app to rebuild its locked executable.
No paid provider calls, commits or deployment were made.

### Data actually present

The local development database passes `PRAGMA quick_check`, uses format 73, and
contains 148 conversation graph executions and 10 workspace graph executions.
All 158 have nonempty producer inputs and non-null outcomes; this count does not
mean every outcome is successful or every input is the final provider wire request.
The legacy `attempts` table has zero rows in this particular database.

Retained nonempty provisional/original text exists for all observed conversation
text operations: 12 replies, 21 glosses, 21 translations, one grammar explanation,
nine assessments, nine observations and six skill attributions. The nine workspace
transcriptions and one selection explanation lack provisional text, although their
inputs, normalized outcomes and metadata remain. Nine recording-result records and
nine graph audio receipts/cache references exist. Cache references alone do not
prove each external audio blob remains available.

Three complete real-use captures were inspected and their provider-tape digests
verified:

| Capture | Duration | Events | Tape records by kind |
| --- | ---: | ---: | --- |
| `3cd58ba4-8b39-418c-b8ce-4771fad9843b` | 272.95 s | 13,838 | 60 protocol, 54 text, 6 speech, 5 microphone, 7 transcription |
| `6cd8898b-107a-462e-9308-7a5effd94cde` | 277.36 s | 9,184 | 41 protocol, 35 text, 8 speech, 4 microphone, 4 transcription |
| `e1d7bd6a-1870-4705-9908-eb2434ffccf8` | 368.76 s | 12,637 | 57 protocol, 50 text, 7 speech, 6 microphone, 6 transcription |

These development tapes retain response frames and captured audio; they are not
ordinary diagnostic logs. Fingerprints are request identities, not a claim that
complete raw HTTP request bytes are in the tape. Their existence materially changes
recoverability assessment. Importing recovered content into ordinary product history
would still require an explicit, identity-checked recovery design. No such import
was performed. Content-free inventory: `.local/activity-data-inventory-20261011.json`.

### Rebuilt executable and actual replay

Current combined executable/UI identity:
`ac7fddeb68e1e661e689f0f672bc2bee979be85bd2cc3fb4883e2c2d4b1daa22`.
UI production build and capture-enabled native build/preparation passed. The first
native build failed because the running development executable was locked; it passed
after the user-authorized close. This npm installation rejects the documented
`npm run workflow:replay -- ... --mute` forwarding, so the working invocation is:

```powershell
node tools/workflow-replay/replay.ts .local/workflow-recovered-identities --mute
```

Full saved-session replay: **failed**, receipt directory
`.local/workflow/ddd6c485-d0be-4157-b9b4-ef72ade2bfb2/`.
It reached action-3868 at recorded time 122.58 seconds, then timed out at
`target-hit-test`: a positional button SVG target was absent at the click.
Unexpected input was null. This is a UI replay compatibility failure, not proof of
provider serialization, and no full-session timing comparison is claimed. The
identity-recovered source is a documented derived fixture, not the untouched original.

Fresh controlled-provider UI capture and offline replay: **passed** using
`node tools/workflow-replay/provider-smoke.ts`. Capture
`628592de-36ff-4041-aeb2-f5cc465f3ce5`, replay
`bbb75282-b124-441d-a916-bf872ad320fb`. The local fixture was closed before replay;
rendered/media checkpoints and tape consumption passed. This proves the current
recorder/replayer works for that interaction, not every learner workflow.

Actual overlapping identical native requests: **passed** using
`node tools/workflow-replay/overlap-smoke.ts`. Capture
`3f236c2b-2146-4c6d-a22b-ea1dd5ecaf9b`, replay
`4105f97a-a469-46cc-be52-95fc0d6eb9ef`. The fixture waits for both requests before
answering either. Therefore the tested request path is not globally serialized.

### Confirmed activity inspection contention

Baseline-workspace probe:
`node tools/workflow-replay/local-read-profile.ts .local/read-profile-workspace --profile`.
Receipt `20dbb851-a636-4435-8884-24c598b637d2/local-read-profile.json`.
Twenty concurrent samples per command: workspace p95 13.6 ms, totals 19.4 ms,
evidence 26.6 ms. Prior audit measured 16–35 ms on the same export. This single run
shows no general read regression; it is not a statistical performance guarantee.
All 21 language totals matched detailed evidence. Cold frame maximum remained
366.6 ms; frame timing is not provider-result-to-visible-content timing.

Activity-specific probe on the current database backup:
`node .local/activity-lock-probe.ts`.
Receipt `fb4565c2-c74a-46b3-b7dc-ca5990dca722/activity-lock-probe.json`.
Three sequential samples per operation (ranges, not meaningful p95 estimates):

| Read | Observed duration |
| --- | ---: |
| Workspace snapshot | 12.5–64.0 ms |
| Current selected graph | 12.8–21.1 ms |
| One history page | 2,585.5–2,714.0 ms |
| One attempt inspection | 2,322.7–3,417.4 ms |

The selected attempt returned 1,574 retained characters, but its response fields
still contain no producer input or domain output. The probe verifies both the
content exposure limitation and actual desktop command latency.

A concurrent test starts attempt inspection, waits 50 ms, then reads the workspace.
Inspection took 3,151.7 ms; workspace read took 3,098.8 ms. Native samples confirm
inspection held the store lock for up to 3,136.3 ms and history for 2,612.6 ms.
Graph-execution work waited up to 2,533.9 ms. No timing overflow occurred.

`application/commands/workspace.rs` holds `state.lock()` across `inspection::attempt`
and `inspection::history`. Those paths reconstruct history with the authoritative
core; `graph/history.rs` explicitly replays retained records. `spawn_blocking`
protects async executor workers but does not release the shared store mutex.
This is a **confirmed P1 contention defect** in the current app. Its introducing
commit has not been bisected, so it is not attributed solely to the latest UI diff.

The first two aggregate probes exceeded CDP's 15-second evaluation budget. A third
saved partial progress with multi-second individual reads. The final probe splits
commands into separate evaluations and completes; this distinguishes batch timeout
from a deadlock. Preliminary receipts are retained, not counted as successful runs.

## Finite branch completion plan

This is a proposed implementation/acceptance checklist. None of its unchecked
items is described as completed. The user's content-ownership requirement is settled;
the detailed storage contract and any ontology changes still need specification
review under the repository agreement. No deployment or release work belongs in
this branch-completion gate.

- [x] Inspect real database and saved captures; preserve originals and recoverability evidence.
- [x] Rebuild and run offline replay, fresh capture/replay and concurrent-request diagnostics.
- [x] Establish a reproducible activity read/lock failure with raw timing receipts.
- [ ] **1. Remove inspection contention.** Keep exact engine/run/revision ownership and
  authoritative graph validation. Use bounded, consistent immutable read material
  outside the application mutex; use existing committed-record access for current
  evidence where valid, and avoid replaying the entire journal for every reopened
  detail. Do not introduce a second topology or weaken integrity checks. Re-run the
  exact lock probe and concurrent graph execution. Proposed gates on this fixture:
  ordinary reads queued behind inspection under 100 ms, cached attempt/history
  reads under 100/200 ms, cold history explicitly measured and interruptible. Add
  correctness tests for concurrent adoption and historical cuts, not only speed tests.
- [ ] **2. Make retained computation inspectable.** From a generated element or graph
  node, expose producer inputs, original retained response, normalized output,
  transformations and upstream/downstream identities. Credentials remain excluded;
  content is private application data. Show explicit absent/evicted/not-captured
  states. Prove the displayed original text matches retained source, including
  failure/rejection and historical selection. Avoid further collapse labyrinths.
- [ ] **3. Close measured retention gaps.** Specify bounded private payload ownership,
  LRU eviction and durable lineage once across text, structured responses and audio.
  Capture originals before parsing/cleanup on success, rejection and cancellation;
  link microphone measurement through transcription to every consuming turn. Keep
  export/log redaction separate. Preserve existing records with consecutive migration
  if persisted shapes change. Inventory every AI operation; no silent exceptions.
  Recovery from developer tapes, where identities match, is a separately reported
  recovery action, never fabricated source provenance.
- [ ] **4. Restore complete workflow acceptance.** Keep old captures unchanged. Use a
  documented compatibility copy only when a target mapping is unambiguous; otherwise
  capture the same finite scenario on the final layout using existing recordings as
  coverage evidence. Require conversation speech input, reply, translation/gloss,
  generated audio, structured analysis, retry/failure, activity opening and historical
  playback. Full tape consumption, final state/media checkpoints and no live fallback
  must pass twice. Never skip failed actions or relabel a partial replay as a pass.
- [ ] **5. Final panel and branch gates.** Verify centered across-layout flow, live
  animation and historical playback, direct readable inspection and content links
  in the actual desktop app. Measure panel closed/open/playing under the same workload,
  with no compiler/tests running, and separate provider delay, store wait, processing
  and visible rendering. Run fast gate, UI tests/build, native tests/Clippy, generated
  contract and workflow checks. Review the complete branch, then request the user's
  concrete final app check. Commit requires a separate explicit instruction; PR/CI
  follows authorized Git work. No release/version/deployment implication.

### Challenge assessment

This does **not** currently justify discarding the graph refactor or accepting a
permanently useless panel. The authoritative graph, substantial original text and
working capture infrastructure exist. Exposing retained records is medium effort;
removing shared-lock historical reconstruction is medium-to-high risk because exact
revision consistency must survive. Universal original-payload/audio retention and
migration are the largest part. Full replay repair is medium effort with uncertainty
from old positional targets. Visual cleanup is comparatively small.

Planning estimate, not a delivery guarantee: roughly 1–2 focused engineering days
for contention plus existing-content inspection, and another 2–4 for the retention
contract/implementation, complete workflow acceptance and regression gates. Unexpected
migration or replay-input gaps can extend that range. Do not call this a cosmetic
one-hour fix, but do not reopen the whole architecture either. A smaller PR can be
reviewable only if it accurately names unresolved retention requirements; it cannot
be called completion of the user's every-element traceability requirement.

The branch is **not ready to check off today**: the measured multi-second activity
lock and incomplete inspection are blockers. The cold-start frame gap is also
unresolved and must be explicitly assessed, not hidden behind the healthy read p95.

Final checks for this diagnostic pass: fast gate passed; workflow tooling types and
all 38 tests passed; documentation links passed. UI production build and native
capture build passed as recorded above. No application source changed in this pass;
the full native/UI suites were not rerun, and no new full-suite guarantee is claimed.
The local focused scripts are diagnostic probes, not committed regression tests.

## Implementation stages (authorized 2026-10-11)

The agreed execution order is fixed: (1) inspection contention, (2) existing-content
inspection, (3) retention contract review, (4) retention/lineage implementation,
(5) full desktop replay and panel acceptance, (6) final regression/branch review.
Each stage ends with a complete checklist in chat; work continues without waiting
unless a material decision requires user input. No commit is authorized.

Stage 1 implementation, pending final desktop remeasurement: immutable bounded
checkpoint/archive bytes are captured in a single read transaction, then decoded
and reconstructed outside the application store mutex. This applies to both history
commands, attempt inspection and the follow-on inspection-credit command. Credit
publication remains transactional and rechecks current ownership. A private 16 MiB,
32-entry LRU caches derived responses by the complete source-byte digest plus exact
selection. Cache hits cannot bypass ownership checks or conceal changed/corrupt bytes.
The cache is cleared alongside reset's protected report buffer. No graph semantics,
provider admission limits or persisted encoding changed.

Focused graph-store tests (eight) and native-coach/transport tests (seven) passed
at intermediate checkpoints. They cover detached reads after source mutation/close,
transaction/size boundaries, selected-revision ownership and normal graph publication.
Final stage timing and checks are recorded below when complete.

Stage 1 desktop verification: rebuilt identity
`bbd1ffe84dc774f532156ee8ee0af5feb2a8dd5cb76ef9a9c68e4d458dcb582f`, receipt
`.local/workflow/793470a4-d4b8-44d6-8313-8c762ad5c0cd/activity-lock-probe.json`.
Uncached inspection 3,469.8 ms overlapped a workspace read completing in 8.3 ms
(previously 3,098.8 ms behind inspection). Inspection/history store holds max
2.541/2.713 ms. Repeated attempts 31.1–31.4 ms, history 148.2–156.7 ms.
First history/attempt requests still cost 4,067/4,317 ms in this run. Reconstruction
is outside the store mutex, not made intrinsically cheap. No timing overflow.
The probe also observes a separate conversation-snapshot hold of 1,708 ms; this
was not inspection's guard and remains a stage-5 performance investigation item.

Stage 2 implemented: protected native historical inspection returns producer input
values and the recorded outcome at the selected cut. Credential-store references
are omitted only at declared runtime configuration boundaries; quoted source content
is preserved. The UI shows inputs, retained original/provisional text and outcome
without additional disclosures, supports exact source view, and links upstream nodes.
Missing original text and an unfinished result are distinct explicit states. The
automatic graph projection still excludes protected content. Generated contracts
were regenerated from Rust. Focused native historical/credential/cache checks,
22 inspector/workspace UI tests, production UI build and fast gate passed. Desktop
coverage of the complete flow remains stage 5.

## Stage 3: reviewed private retention contract

Status: implementation contract reviewed against the user's explicit retention
requirement and G1/G8/G9/G11. This adds protected host storage and inspection, not
new graph execution semantics, scheduling rules or a viewer topology. No user
choice or ontology exception is required to proceed within the authorized scope.

1. Ownership: executions own purchased request/response content; recording identities
   own source microphone measurements. Consumer/message associations preserve the
   producing execution and every contributing recording ID. Cancellation revokes
   publication, not ownership of the bytes already received.
2. Source boundary: preserve exact native-to-inference-service request bodies and
   response bytes before decoding, cleanup or domain validation. Preserve the exact
   source WAV and transcription request fields. Multipart field content and audio
   are source data; generated multipart boundaries are transport packaging. The UI
   identifies this boundary explicitly and does not claim visibility into upstream
   provider HTTP exchanges the service never forwards.
3. Original versus derived: retained original text/body and validated domain output
   are distinct records. No normalization is applied to original content. Existing
   original text stays intact; it is not deleted because a new cache exists.
4. Credentials: authorization headers, API keys and credential-store references are
   excluded. Actual credential values echoed by a service are removed explicitly;
   prompts, transcripts and ordinary content are not treated as secrets to redact.
   Logs/export diagnostics remain a separate content-free projection.
5. Bounds: reuse the learner's existing inference-cache capacity (currently 256 MiB
   by default), with LRU access and shared blob accounting. Eviction removes payload
   availability, not ownership, digest, byte count or consumer lineage. Zero capacity
   means explicit eviction, not an invented empty successful response. Existing
   transport input/response bounds remain; truncation/partial/cancelled/not-captured
   states must be distinguishable. Storage failure is reported, never silently passed.
6. Lifecycle: capture request before dispatch; capture response chunks before parsing;
   retain partial data on failure/cancellation; mark terminal state explicitly. A
   restart never infers completion from missing evidence. Source reads do not issue
   AI calls. Historical graph outcomes remain at their selected revision; any later
   exchange completion is identified separately rather than backdated.
7. Persistence: consecutive format migration adds records and associations, preserving
   the full supported migration chain. No historical records are fabricated from
   present-day settings. Old missing content is explicitly not captured; source tapes
   remain unchanged. Recovery imports are not part of this implementation.
8. Acceptance: exercise successful text/structured/audio requests, malformed response,
   cancelled stream, restart, eviction, unchanged source text and contributor linkage.
   Inspect retained content through real app commands; verify diagnostics exclude it.
   Measure storage/inspection contention again before final acceptance.


## Stage 4 implementation checkpoint

Implemented private original-content storage in format 74 (additive migration),
using the existing inference blob capacity/LRU with durable eviction receipts.
Text, speech and transcription transports tee exact incoming bytes before decoding.
Multipart transcription retains actual field values and the source WAV separately.
Credentials are removed only by exact actual credential value; content remains.
Recording WAV writes are atomic before recognition, including cache reuse. Turn input
provenance now retains every contributing recording ID, validates conversation
ownership, and exposes it again after restart through the conversation record.
Native graph inspectors expose current original-body availability separately from
historical graph outcomes. Reads verify content after releasing the store mutex.
New workflow tapes contain timed sanitized original-content writes; older tapes
remain readable but cannot supply uncaptured bytes.

Verification at this checkpoint: 4 retention tests, 4 real-HTTP graph transport tests,
30 focused UI tests, production UI build and fast validation passed (fast gate will
be rerun after final changes). Migration suite passed 50/51 initially; the failed
source-format reconstruction fixture needed to remove the new child tables before
its historical parent tables. That fixture was corrected and its targeted rerun
passed. New migration rollback/preservation and all supported baseline migration
checks passed. No commit, deployment or release operation was performed.

Desktop verification and final full branch checks are still pending at this checkpoint.

## Stage 5 desktop verification

Verified fresh actual-desktop coach capture and offline replay with a local synthetic
service (no paid external call). Capture `8ea12654-7014-4add-bf6a-1dcdd95107d4`
and replay `acfc3952-b615-4f6c-b1a8-b6eb2230c409` preserve exactly the same
5,083-byte request and 610-byte NDJSON response, including Unicode source text.
The Reply Details UI displayed both originals. Screenshots and local receipts are
in their `.local/workflow/<id>/` directories. Visually verified centered horizontal
cards and direct Details controls. Corrected stale inspector text that still claimed
original bodies were never retained. The transcription picker now opens the recording
source inspector as well as the graph and message-record entry points.

Inspection contention probe `c7bb4b1f-d883-4bb0-a3a3-9ee8e9df856e` against a
consistent copy of the development workspace: uncached attempt 3,447.9 ms while a
concurrent snapshot completes in 13.1 ms; warmed attempt 34.2 ms and history 150.3 ms.
Earlier blocked-snapshot measurement was about 3,099 ms. Historical reconstruction
remains expensive but runs outside the application mutex. Transcription receipt
reconstruction also now uses immutable captured history outside that mutex and the
same receipt-construction functions as ordinary native reads.

**Performance limitation at this checkpoint (addressed below):** content-free phase profiling found the ordinary
conversation product projection still spending about 910 ms on message status and
365 ms on turn views (about 1.28 s total before final fields) under the store mutex.
This is separate from the repaired historical inspector and is not a claimed solved
latency guarantee. Temporary phase instrumentation was removed after measurement.

**Replay scope:** fresh controlled current-UI recording/replay passes. The earlier
full saved user recording still hits an obsolete positional SVG target at action
3868 (122.58 s), documented in the earlier replay audit; it has not been represented
as passing. Original recordings were not changed. This work does not fabricate
missing old raw responses from decoded replay results. Native HTTP and lifecycle
regressions cover speech/transcription and failure paths; the final acceptance is
still a human test of the rebuilt app, especially microphone use and layout comfort.

Final branch checks and independent review follow in stage 6.

## Stage 6 verification and acceptance barrier

Implemented and verified the final review correction: original-audio playback now
uses the learner's playback rate and master/voice volume, including mute. Independent
source review found no additional actionable retention/lineage defects after that
correction. This is a source review, not a claim that every possible workflow was
manually exercised.

Final checks:

- UI suite: 301 files, 2,062 tests passed. After the playback-settings correction,
  the five affected inspector/architecture suites passed all 50 tests; production
  UI build passed on that state.
- Full native run: 955 passed, nine failed, five ignored. All nine failures were
  corrected and verified by rerunning the entire application group (69 passed) and
  speech-recording group (13 passed). The fixes preserved the reusable-result count,
  made the audio-publication failure fixture target publication after original
  capture, and gave transcription fixtures their required recording owners. The
  supported migration matrix passed in the full run. The entire native suite was
  not rerun after these focused corrections.
- Final fast gate, Clippy with warnings denied, generated-contract verification,
  workflow-enabled binary checks/build, 38 workflow-tooling tests, documentation
  links and diff whitespace checks passed. Logs are under `.local/final-*.log`.
- Final desktop capture `38c1ef9b-5f48-41e0-bd24-2e135d15f87d` and offline replay
  `c3d36adc-df4a-42f0-bc76-3efa28112380` passed. The product inspector showed both
  originals; History → Play advanced from revision 1. Captured and replayed request
  (5,083 bytes) and response (610 bytes) had identical digests, states and retention
  availability. This used the local fixture service, with no paid provider call.

The rebuilt desktop app is now running on the normal development workspace, with
AI activity open and 20 graph nodes rendered. The normal connection reports
connected. Workspace format is 74 and SQLite `quick_check` returns `ok`; the
workflow recorder is disabled. Local evidence: `.local/acceptance-ready.json` and
`.local/acceptance-activity.png`. No commit, release or deployment was performed.

Human acceptance remains: inspect an existing run's inputs/outcomes, play its
history, then create a new microphone/message computation and inspect its source
recording and original request/response. Older uncaptured bodies remain unavailable.
At this checkpoint the ordinary conversation-projection delay and old positional
replay incompatibility remained limitations. The following performance work addresses
the ordinary projection delay; old replay compatibility is outside this follow-up.

## Follow-up: ordinary conversation projection performance

Implemented after the user rejected leaving the roughly 1.3-second delay unresolved.
The baseline was measured through the running desktop's real `watch_conversation`
IPC on the development workspace: 19 messages and 10 turns, with three forced reads
of 1,324.6, 1,319.2 and 1,335.6 ms. This was local projection work, not provider
latency or an intentional timer. An instrumented native read performed 5,151 graph
record reads, repeatedly loading/decoding the same records and constructing full
inspection reports for narrow product fields while holding the workspace lock.

Implemented changes:

- Protected immutable reads memoize verified records only for that operation. Each
  subsequent operation reads storage and verifies stamps, hashes and byte limits
  again. A matching committed resident record avoids redundant JSON decoding only
  after storage verification succeeds; there is no fallback on storage failure.
- Source inputs are resolved together, with one protected input set shared by the
  messages belonging to the same run within a conversation page read.
- Receipt validation and reply previews use narrow core reads instead of building
  complete inspection reports. They share the existing authoritative state and
  preview rules. Scheduling and execution identity are unchanged.
- Readiness checks observe value presence without cloning content. Bounded SQL
  reads use prepared statements and one query, retaining allocation/type checks.

The rebuilt unoptimized desktop measured 288.2, 277.4 and 296.9 ms after startup,
about 78% below baseline. A snapshot queued 20 ms behind a forced conversation read
completed in 236.7 ms (289.7 ms total). Initial reads after startup measured
313.6–346.6 ms; the steady-state result is not a universal sub-300-ms guarantee.
The existing lock remains, with substantially less repeated work inside it.
Measurements used the same normal workspace without external inference calls.
Local receipts: `.local/conversation-latency-baseline.json`,
`.local/conversation-latency-final-steady.json` and
`.local/conversation-contention-final-steady.json`.

Verification includes a regression demonstrated failing before the fix (six reads
for three unique records), fresh-read failure propagation, batched input results,
warm/cold byte and record budgets, scoped/full preview equivalence, unrelated
producer isolation, and bounded SQL reads that recheck changed content and limits.
Temporary profiling code was removed. Final fast validation, Clippy with warnings
denied and the workflow-enabled desktop build passed. The full workflow-enabled
native library suite passed: 987 passed, zero failed, five ignored (678.60 seconds).
Documentation links and diff whitespace checks passed.

The final post-suite desktop repeat measured 442.7, 367.8 and 367.4 ms, with a queued
snapshot taking 344.2 ms (405.2 ms total). This repeat failed the local harness's
aspirational 300-ms ceiling, despite the earlier steady-state run passing it.
Observed measurements therefore support a substantial reduction of the 1.3-second
bottleneck, not a consistent sub-300-ms guarantee. No threshold was weakened and
the slower receipt is retained as `.local/conversation-latency-acceptance.json`
and `.local/conversation-contention-acceptance.json`.

The rebuilt normal desktop remains running with AI activity open: 20 nodes,
connection active, recorder disabled, format 74 and SQLite `quick_check = ok`.
The final screenshot was inspected; receipts are `.local/acceptance-ready.json`
and `.local/acceptance-activity.png`. Human acceptance can proceed in that app.
All changes remain uncommitted; no release or deployment was performed.
