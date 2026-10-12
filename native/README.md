# Native — Rust and Tauri

The application code running on the user's device: persistence, credentials,
AI execution, recording, and native integration. This is separate from the
remote [Python server](../server/).

iOS diagnostic export uses the private plugin crate and Swift package under
`src/diagnostics/ios/`. Tauri's plugin build helper links it during the normal
iOS build; no generated Xcode project edits are required. Rust creates a ZIP
of the existing structured-log allowlist in app cache. UIKit presents the share
sheet, including Save to Files, and Rust removes the temporary ZIP after completion
or cancellation. Desktop continues to export to Downloads. iOS presentation must
be verified on macOS/Xcode and on an iPhone/iPad; Windows native checks do not
compile that target.

## Shared language behavior

Follow the repository's [language-independent behavior rule](../AGENTS.md#language-independent-behavior).
Implement one general policy using Unicode properties and shared capabilities;
do not add language-specific code paths or character lists to solve a general
problem. Declarative language-config overrides are a documented last resort,
only after the shared approach has been shown insufficient. Preserve original
text; normalization belongs only to the operation that requires it.

## Native graph integration

Native graph publication adapters now cover reading helpers and assessment/feedback
with receipt-owned disclosure and transactional learning credit (workspace format
63). Format 64 adds native audio receipts separately from evictable payloads under
the existing shared cache budget. Format 65 records delivered-audio identity
against native consumers without retaining another audio copy. Format 66 is a no-op compatibility marker; it changes no persisted data.
Format 70 records each native assessment receipt's producer engine and run,
preserving prior results and disclosures.
Format 69 adds message-owned native helper requests for explicit fresh runs.
Existing results and source identities remain intact.
Format 68 adds workspace-owned native execution and bounded reading projections.
Standalone text reading commands use those engines; transcription, standalone
speech and proposal conversion remain in progress (migration step 5).
Format 67 adds message-owned native playback requests, without duplicating audio
or conversation turns. Explicit playback uses the shared speech graph.
Ordinary partner Send and every opening entry point now admit native graphs through
the application command transaction. The application host binds their text,
structured-response, evidence and speech capabilities. Conversation controls and
revisions also use native ownership. Other AI entry points and running-app
acceptance remain on the
[production checklist](../docs/notes/ai-graph-production-integration.md).

## Source folder map

`application/conversation_reads/` owns the ordinary conversation display response
and its workspace-lifetime memory cache. A cold read uses the existing validated
projection; unchanged reads reuse its encoded JSON without SQL, graph inspection
or repeated encoding. The cache is bounded to 32 MiB and keyed by conversation,
page, session and the owning SQLite connection's in-memory change counter. Local
writes invalidate it conservatively, including rolled-back writes. Navigation-only
`OpenConversation` commands preserve current content and advance its observation
revision under the same store guard as commit. If that command gains a new
display-affecting side effect, remove or revise this optimization and its parity
test. Other commands must not promote cached content.

Conversation responses also carry an ephemeral decimal `contentRevision`, taken
from that connection's mutation counter. Within a session, conversation and page,
retaining this token means retaining the same display fields even when navigation
advances the observation revision. The UI keeps message/history object identities
and skips learner refreshes in that case. Different tokens trigger normal refresh;
numeric token ordering breaks ties between same-revision history/live responses.
Cold reads may assign a newer token conservatively. This is not a persisted
version, execution identity or cross-session comparison key.

Rendering outside the store lock may finish after another write; such a response
is never installed as the current cache entry. The ordinary watcher observes the
new revision on its next read. Reset clears retained responses; fatal workspace
state prevents cache access. This relies on exclusive workspace ownership and
does not detect unsupported writes from another connection or offline corruption
on every warm display hit. Protected storage/execution reads, startup validation
and persisted history are unchanged. This cache is a derived display projection,
never authority for commands, graph execution or result adoption.

`application/learning_reads.rs` applies the same workspace version and exact
navigation exception to skill evidence, language totals and scoped effort
progress. Its 16 MiB encoded-value cache shares the bounded cache implementation
in `application/response_cache.rs` with graph inspection. Encoding and warm
decoding happen outside the store guard. Credit publication, award claims and
time-dependent reports remain uncached; their writes invalidate display entries.

The scheduler retains its 100 ms wake interval, but an unchanged idle workspace
requires no SQL or graph inspection. `application/scheduler.rs` remembers a scan
only when it returned no claim and made no writes. Each tick still checks actual
provider capacity; any capacity, session or database change requires a new scan.
Navigation can preserve an exact idle observation because it changes no scheduling
input. A scan that advances/adopts/cancels without launching work is not idle and
must remain eligible for another scan. This is a memoized scheduler observation,
not another readiness model or permission to bypass dispatch/adoption validation.

| Folder | Responsibility |
| --- | --- |
| [src/application/](src/application/) | Startup, runtime state, native command registration and background scheduling |
| [src/conversations/](src/conversations/) | Conversation turns and execution, prompts, opening choices, revisions, reading results and conversation export |
| [src/partners/](src/partners/) | Persona definitions, prompts, generation request projection, and reactions |
| [src/drill/](src/drill/) | Manual phrases, attempts/comparisons, sessions/visits, reference cache and recording retention; recording and AI execution stay shared |
| [src/learning/](src/learning/) | Coaching, learner evidence/state, progression, rewards and reward settings |
| [src/speech/](src/speech/) | Capture, recording commands, transcription receipts, audio inspection, fluency timing and one-time audio delivery |
| [src/ai/](src/ai/) | Access, credentials, routing, admission, refusals, shared generation lifecycle and receipts, hosted connections and provider transports |
| [src/storage/](src/storage/) | Workspace ownership, database initialization and schemas, reset and workspace-copy export |
| [src/language/](src/language/) | Language lookup, Unicode/emoji handling, existing linguistics code and its fixtures |
| [src/configuration/](src/configuration/) | Configuration loading, validation, types, citations and schema checks |
| [src/statistics/](src/statistics/) | Mechanical usage summaries, formerly in `profile.rs` |
| [src/diagnostics/](src/diagnostics/) | Logging and diagnostic records |
| [src/updates/](src/updates/) | Application update discovery |
| [src/bin/](src/bin/) | Contract exporter and offline gloss benchmark entry points |

`src/main.rs` is the executable entry point. `src/lib.rs` declares the native
modules and exports `application::run`. Application composition lives in
`src/application/startup.rs`, runtime state in `state.rs`, the dispatch loop in
`scheduler.rs`, and command handlers in `commands/`. `application/mod.rs` provides
their shared imports and application entry points.
`src/model.rs` retains mixed types pending a separate ownership-based split.

Language- and script-specific rules belong in `language/` or editable language
configuration. Feature and transport code consume generic language capabilities;
they must not branch on individual language or script identities. `language/script_text.rs`
owns configured-script validation, and `language/text_diagnostics.rs` owns
content-free script/Unicode comparison metadata. Its serialized diagnostic fields
remain stable when called by audio transport. Language-specific test examples,
localized messages and font asset declarations are data, not exceptions for
embedding language behavior in feature code.

### Executable graph foundation

`ai/graph/` owns the typed DAG compiler, bound operation registry, composition,
execution state machine and inspection. It has no product, database or provider
dependencies. All application AI workflows execute through graph hosts. Follow the
[foundations](../docs/notes/ai-graph-foundations.md) and
[semantic profile](../docs/notes/ai-graph-core-semantics.md) when extending it.
The executable artifact owns topology; run state overlays that artifact.

`graph::DurableEngine` implements the reducer's
[owner-transaction contract](../docs/notes/ai-graph-durability.md).
`ai/graph_store/` supplies SQLite storage. A partition belongs to a conversation
or workspace and an exact executable catalog. Protected reads verify native
bytes, stamps and integrity. The domain owner checks authority and stages
publication in the adapter's single transaction.

`turn_execution_owners` declares each turn's channel, engine, run and artifact.
`conversations/execution/graph_publication/` publishes replies, assessment receipts,
reading results and stable learning effects from adopted graph values. Exact source,
attempt and producing execution identify publication. One-time credit and product
state commit together. Revisions preserve predecessor wording and coach history,
remove the dependent partner suffix and revoke replaced-source authority.

`conversations/execution/graph_runtime/` owns admission, source status, controls,
recovery and usage. `application/graph_execution.rs` binds credentials, provider
admission, transport, bounded provisional text and checks after asynchronous work.
`application/native_reading.rs`, `native_speech.rs`, `native_transcription.rs` and
`commands/proposal_execution/native.rs` connect workspace-owned workflows.
The [integration checklist](../docs/notes/ai-graph-production-integration.md) records
implementation, verification and running-app acceptance separately.

`execution/context.rs` validates captured prompt context while preserving exact
text and source identity. `coach_graph.rs` binds its validated output to the typed
prose operation; `partner_graph/` composes reply, opening and helper artifacts.
Shared language, learning and speech operations own their typed contracts.

`ai/transport/graph_identity.rs` stages immutable wire request IDs per producer
inside Dispatch. Recovery retains those IDs; explicit retry creates a new producer.
`graph_authority.rs` checks captured sources, ownership, holds, destination and
credentials. Adoption checks authority independently of dispatch; pause alone does
not revoke an available result. `ai/transport/graph_evidence.rs` retains classified,
redacted metadata and partial outcomes. `graph_request.rs` connects an authorized
producer to transport and native evidence/provisional capture. The application host
persists reports and owns admission and asynchronous authority checks.

`DurableLimits` requires an explicit maximum serialized settlement-event size.
Admission reserves checkpoint space for running outcomes, consumer adoption and
recovery. Oversized outcomes fail without truncation; callers must retain them.
Reservations do not guarantee that a storage commit succeeds.
The [inspection export](../docs/notes/ai-graph-inspection.md) preserves these same
graph types while replacing sensitive constant values with explicit omissions.
Generate its TypeScript contracts with the normal `npm run contracts` command;
do not add a frontend graph catalog. Activity IPC exposes live snapshots, timeline pages and revision-bound attempt evidence.
Saved checkpoints expose the same definition projection without executable
handlers through `Checkpoint::inspection_definition`. This reads historical
evidence; live recovery still requires compatible implementations. The
[retention contract](../docs/notes/ai-graph-retention.md) describes formats 2/3/4 snapshots
and atomic compaction. Compaction retains exact old checkpoint bytes, preserves
absolute revisions and checks settlement reservations against the new footprint.
Recovery audits the full bounded archive chain through native replay. Explicit
record eviction and cold payload reads work through the native SQLite adapter;
production workflow wiring, automatic eviction policy and selective history deletion
are not implemented.
The [historical reader](../docs/notes/ai-graph-history.md) reconstructs selected
logical cuts without handlers, using the same validated structure and reducer.
Attempt pages retain full topology and native node facts with explicit continuation
cursors. It accepts a read-only `HistoryStore`; executable recovery still requires
exact handlers. Opening a reader still audits and holds the retained native state.
Declare `DurableLimits.state` and `HistoricalLimits.state` explicitly. Their
[retained record ceilings](../docs/notes/ai-graph-state-bounds.md) reject new run,
attempt or execution allocation atomically while leaving admitted settlement,
adoption and recovery possible. Cancellation and compaction do not free record
allowances; eviction does not free them either. These are logical count limits,
not loaded-payload counts or RSS bounds.
`state.rs` owns canonical run, attempt and execution records. Runs hold current
attempt references; retry history lives in independent attempt rows. Format-4
checkpoint bases retain primary-row commitments and allocation/control scalars.
Formats 2/3 remain readable and audited by native replay. `records.rs` separates
stable identities from optional loaded records with copy-on-write payload ownership.
Transactions share unchanged loaded payloads and load cold rows before mutation.
`attempts.rs` derives a per-run sorted ID
index from primary rows, rebuilding it on decode without changing checkpoint encoding.
Historical pages seek within that index and read only selected attempt rows; node
facts follow current references. The index adds no scheduling or visualization rules.
`executions.rs` indexes producer IDs by immutable reuse key; attempts also index
their immutable execution association. The reducer uses these queries for reuse
and current consumers while retaining eligibility and oldest-producer selection.
Indexes rebuild from primary records and are not separately persisted. An unresolved
execution-ID index narrows resource occupancy, dispatch, cancellation cleanup,
producer recovery and settlement reservations to unfinished producers. Queries
enumerate IDs without dereferencing payloads; required rows use typed access.
The [native record transaction boundary](../docs/notes/ai-graph-record-storage.md)
adds changed primary rows to the same owner commit as checkpoint/publication.
Executable recovery audits bounded, stamp-bound record reads against native replay
before writing recovery state. The workspace SQL adapter and isolated fixtures
implement this contract. Durable adoption loads its three typed native records through
`record_access.rs` and uses the same adoption decision as resident replay.
Durable dispatch also loads the selected owner, producer, shared consumers and
unresolved capacity records through that interface. `dispatch.rs` owns the shared
dispatch decision; invocation uses the loaded work only after the owner commit.
Advance now uses stored reads for dependencies, reuse and admission through the
same native scheduler as replay. `dependencies.rs` separates read errors from
graph availability, and `staged_access.rs` reads records created earlier within
the same atomic event without requiring premature persistence.
Other event decisions now use typed reads too. `consumer_reads.rs` resolves current
membership from loaded records selected by immutable associations. Stored inspection
and output methods use the same native projection/resolution as resident access.
`record_evidence.rs` verifies stored rows against a revision-bound key/digest catalog
rebuilt from validated replay and updated only after acknowledged commits. Reads no
longer require resident payloads for integrity comparison. `evict_records` archives
the event suffix before dropping all primary payloads; it preserves IDs, indexes,
commitments and complete history. `resident_usage` reports loaded counts separately
from `state_usage`. Use `read_inspection` and `read_outputs` after eviction; resident
helpers return `record_not_resident` when required data is unloaded. Subsequent
events load required rows and retain mutated rows until the next explicit eviction.
Startup still replays full history. An aggregate working-set budget, disk-backed
indexes and production integration remain open.
Declare `DurableLimits.record_reads` explicitly. It caps the number of native row
reads and their aggregate encoded bytes across each owner operation, including
mutation and final reservations. The remaining byte allowance reaches the adapter
before body allocation. Budget exhaustion rejects the complete operation; it does
not hide nodes or commit partial fanout. `set_record_read_limits` changes only owner
admission policy and allows an explicit retry. This bounds stored-read work, not
decoded heap/RSS, resident payloads or checkpoint allocation. Startup recovery's
row audit and Recover event share one outer allowance; full replay still precedes it.
The [finite refinement check](../docs/notes/ai-graph-refinement.md) compares the
native reducer with an independent model of two consumers sharing one producer.
It exhausts that lifecycle's reachable states; broader scheduling, durable storage
and production integration remain outside its verified scope.

### Reply-help execution

Transport inputs live in `ai/transport/text_request.rs`. They contain execution
identity and provider inputs, without conversation or Drill publication state.
Conversation dispatch projects those inputs; independent reading and proposal
generation construct them directly. Translation and gloss contracts live in
`language/{translation,gloss}.rs`; speech-input construction lives in `ai/audio.rs`.
Shared execution settings live in `ai/connections/configuration.rs`, and request
identities in `ai/identity.rs`. Transports must not import product workflows.
Speech callers share persistent results, blobs and pending subscriptions in
`ai/results/`, composed with access and transport in `application/speech_results.rs`.
Keys identify exact text, language and model, scoped to the workspace, endpoint,
route and account identity. A local contract marker separates incompatible key
semantics. Product associations do not enter those keys. The unused local voice
hint is not sent to the service and does not affect reuse.
The shared cache defaults to 256 MiB and uses read-touch LRU; Settings exposes its
capacity and logical payload usage. Zero disables retained reuse, while concurrent
callers can still share pending work. Receipts survive eviction and cancellation.
`speech/delivery.rs` is a bounded, consuming mailbox for asynchronous playback,
including output too large for the configured cache; it is not reusable storage.
`speech/stream_delivery.rs` separately holds bounded provisional PCM by shared
execution. Conversation consumers read it with a sample cursor after current
source/session checks; only a validated completed stream reaches reusable storage.
The speech adapter opts into NDJSON delivery and also accepts completed WAV
responses from services without streaming. See the
[implementation and verification note](../docs/notes/streaming-speech-implementation.md).


Cache hits need no network request or secret lookup. A miss sends the existing
speech request directly; caching requires no server protocol change. Saved audio
survives restart and remains reusable until evicted or cleared. A changed input,
model, endpoint or account selects a different key; changing back can reuse the
original result. Unrelated configuration revisions and pause do not invalidate
saved audio. Existing result associations keep their original audio.
The app does not discover hidden server voice or processing changes. To discard
retained audio, set cache capacity to zero, then restore the desired capacity;
execution receipts survive. Accepted text and original recordings keep their
existing durable owners.

Scheduled correction feedback uses the same retained result store
through `application/coaching_results.rs`. Exact request payloads, response schemas,
validation inputs and access identity determine reuse; consumer attempt IDs do not.
Concurrent consumers share one execution and each publishes through its own existing
turn transaction. Successful results remain reusable until eviction; invalid responses
and provider failures remain inspectable but are not reusable. An explicit retry may
make a new request when no usable result exists. This path adds no automatic retry.
Shared execution receipts count provider usage once across associated attempts.
Other scheduled text tasks retain their existing grouped execution behavior.

Generated reading text and transcription also use the shared local repository and
pending subscriptions. Proposal generation uses shared execution receipts while
remaining fresh for each explicit request; proposal text is not reusable cache data.
See the [shared inference audit](../docs/notes/shared-inference-architecture-audit-2026-09-24.md)
for verification and remaining application checks.

Accepted gloss lookup is independent of conversation snapshots and UI lifetime.
`language/reading/saved.rs` defines read-only query/source contracts;
`conversations/saved_reading.rs` projects accepted message and suggestion
annotations from their existing durable owner. The query uses captured language
and variety scope, excludes archived/replaced/invalidated sources, preserves
source/operation/attempt IDs, and creates no inference attempts or cache entries.
Exact matching stays in the shared UI reading domain. The native projection
filters exact surface candidates and bounds input/output without silently
truncating results. Generated glosses are a separate evictable projection from
`language/reading/text_sources.rs`; accepted records keep their durable owners.

`language/reading/preloaded.rs` imports the registered dictionary packages once per
package version during workspace startup. Migration 55 adds indexed imported
records and package provenance. Migration 56 preserves these records while
allowing an explicit editorial-review status distinct from source checking.
The same saved-gloss command queries these rows;
hover and tap do not read package files. Dictionary entries survive inference
cache eviction, create no provider executions or learner credit, and remain
available without AI access. Contextual annotations take precedence in the UI.
See [the package scope](../content/reading/READING_README.md) for supported exact
forms, varieties, references and withheld pronunciation fields.

Workspace execution preferences in `configuration/execution.rs` control assessment,
reply briefs and reading support. Assessment defaults to Automatic; the other two
default to On demand. Coaching feedback always runs automatically beside the reply. New turns capture the values and learner
revision; changes do not backfill or cancel accepted work. Suggested replies and
usage explanations remain explicit requests.

`conversations/execution/optional_help.rs` handles source-bound message requests.
Repeated opens reuse the same operation; failed/unknown work requires an explicit
retry. Requests bind current access authority while preserving captured language and
source context. Assessment also schedules its dependent quote extraction. Store
commands own the transaction, receipt and replay boundary. Settings and the AI panel edit the same preferences. Message feedback, reply-help
and reading controls request missing work through this command.

### Message versions and assessment ownership

Accepted `messages` rows are immutable in text, role and ownership. A fix creates
a new message and turn, linked by `turns.replaces_turn_id`; the predecessor remains
available for inspection. Each version can request `coach_feedback` under its execution policy with
its current text and conversation context. Prior correction results and private
coaching dialogue are excluded from that assessment request. Fix counting retains
the same revision chain and does not assert that an issue was resolved.

`conversations/assessments.rs` persists successful correction, numeric rating and
skill-assessment results in `message_assessments`. Each row references its source
message and producing attempt. Database constraints verify that the attempt's
operation belongs to the same turn and assessment kind. Results and ownership
are immutable; reassessment creates another attempt/result for the same version.
Pending or failed reassessment does not present an older success as current.
`assessment_disclosures` binds mutable disclosure to its producing result by a
foreign key. Learning projections
remain with their existing owners and retain their one-time credit rules.

`get_message_history(conversation_id, message_id)` returns `MessageHistory` for
any learner-message version in the specified conversation, ordered oldest first.
The root message identifies the series and the current message identifies its
latest version. Results include text, predecessor, timestamp, current coaching
and numeric feedback, and operation states/attempt IDs for each version. This
query is independent of chat pagination. The UI adapter is
`platform/ipc/message-history.ts`; a history browsing surface is not yet wired.
Editing an earlier exchange still deletes its dependent later conversation
suffix; this is version history, not an archive of abandoned conversation branches.

Workspace format 45 requires fresh development data. Incompatible workspaces
are refused explicitly; startup does not migrate or silently reset them.

### Subfolder groups

| Area | Current groups |
| --- | --- |
| `application/` | Startup/command registration, shared state and scheduler; `commands/` groups workspace, connection, hosted and persona-generation handlers; existing suites live in `tests/` |
| `learning/` | `coaching/` (requests, observations, policy), `learner/` (state, progression), `rewards/` (rewards, settings) |
| `partners/` | `persona/` (definitions, prompts), `generation.rs` (persona request projection), and reactions |
| `speech/` | `recording/` (capture, commands, transcription), `analysis/` (inspection, fluency); `delivery.rs` holds the one-time playback mailbox |
| `ai/` | `results/` (shared results, blobs, receipts, request identity and subscriptions), `generation/` (proposal registry and receipts), `connections/` (access, credentials, routing), `hosted/` (hosted integration, mobile sign-in), `transport/` (text, speech, grouped responses), `policy/` (admission, refusals, bounded retries) |
| `storage/` | `schemas/` holds database SQL; `store/` groups locking, schema validation, startup, snapshots and transactional commands; reset remains in factory_reset.rs |
| `conversations/execution/` | Admission, holds, connections, turns, snapshots, dispatch, publication, reading retries, speech and recovery; behavior-based tests in `tests/` |
| `ai/transport/provider/` | `keys.rs` verifies credentials; `payload.rs` builds prose/structured requests and enforces input limits; `request.rs` owns HTTP and dispatch routing; `response.rs` decodes completions and validates prose; matching suites and local HTTP fixtures live in `tests/` |

The first folder passes moved whole files. Large-file cleanup has now started with
conversation execution; its implementation and test files are each under 500 lines.
Other large modules await individual cleanup. Some `mod.rs` files still contain
existing implementations. See the source-size policy in root `AGENTS.md`.

The store command coordinator retains session/replay checks and a single receipt
transaction. Domain handlers under `store/commands/` borrow its transaction; they
do not commit independently. Tests under `store/tests/` cover workspace/schema,
preferences, command rollback/replay and record lifecycle. File-size bands are
review guidelines: prefer cohesive code over splitting solely to satisfy a count.

These are responsibility groups, not newly independent crates or a redesigned
layered architecture. Existing cross-domain calls remain. Keep tests with their
module and feature-specific database operations with their feature. General
workspace/schema mechanics belong in storage. Network transport belongs in AI;
conversation execution and speech lifecycle keep their respective domain owners.

## Content and packaging

- [../content/](../content/): app-owned language documents, skill definitions,
  conversation topics and teaching policy. The [content guide](../content/CONTENT_README.md) links prompt code.
- [../content/rust-schemas/](../content/rust-schemas/): generated configuration schemas,
  verified by Rust tests.
- `capabilities/`, `icons/`, Tauri configuration and platform property lists:
  native permissions and packaging.
- `gen/`: platform projects and generated support files; Android customizations
  are tracked, while other generated output follows `.gitignore`.
- `target/`: ignored Cargo build output.

During `npm run tauri dev`, edits under `content/` trigger a native rebuild and
application relaunch through `build.additionalWatchFolders` in `tauri.conf.json`.
Content is embedded at compile time; refreshing the webview alone does not reload
YAML. Restart an already running dev command once after changing watcher settings.

## Linux build dependencies

The Linux credential-store dependency enables `keyring`'s `vendored` feature.
Cargo compiles the bundled D-Bus library automatically, so `libdbus-1-dev` and
a custom `PKG_CONFIG_PATH` are not needed for this dependency. This still uses
the desktop's running D-Bus session and Secret Service for credential storage.
Other Tauri and audio system build dependencies remain required. On Pop!_OS,
Ubuntu and Debian, interactive `npm run tauri dev` checks GTK 3, WebKitGTK 4.1
and ALSA before starting Vite/Cargo, installs missing development packages through
`sudo apt-get`, then verifies them again. Enter your sudo password when requested.
Failed installation stops startup. CI and noninteractive runs report the required
install command without attempting installation. Other distributions receive a
prerequisite error; explicit cross-target runs and other Tauri commands skip setup.

## Verification

From the repository root:

```sh
npm run tauri -- info
cargo fmt --manifest-path native/Cargo.toml -- --check
cargo clippy --manifest-path native/Cargo.toml --lib --tests -- -D warnings
cargo test --manifest-path native/Cargo.toml --lib
cargo check --manifest-path native/Cargo.toml --bins
npm run contracts:check
npm run languages:check
npm test -- tests/architecture
```

Transport tests use local loopback servers; allow localhost binding when running
inside a sandbox. They do not need live AI providers. The UI registration check
reads `src/application/startup.rs`, where the `generate_handler!` list lives.

Native provider submissions use graph-owned execution identity. Uncertain outcomes
remain explicit and require requested retry. `ai/policy/retry.rs` retains retry
metadata reported by the service. Server-side rate-limit retries are bounded by
`server/app/inference/retry.py`; completed siblings and entire groups are not replayed.
Response diagnostics retain attempt timing, refusal reasons and retry history.

The root Tauri launcher selects this directory explicitly. Moving the native
project root can invalidate cached build-script paths;
`cargo clean --manifest-path native/Cargo.toml` clears build output without touching
source or application data. Working notes belong in [docs/notes/](../docs/notes/).

### Linux desktop icon

The running GTK/Wayland window identifies as the executable name
(`skellyspeak`), independently of its D-Bus application identifier.
Tauri's standard Linux package launcher already uses the executable name as
`StartupWMClass` and bundles the configured PNG icons.

`npm run tauri dev` registers a user-local `skellyspeak.desktop` launcher and
256px logo through `tools/linux-desktop.ts`. Its filename and StartupWMClass
match the actual window; Icon uses the absolute local PNG path to avoid
desktop-specific theme-cache lookup differences. This makes the logo available
for the unbundled debug executable too.

The launcher points to this checkout's default
`native/target/debug/skellyspeak`; custom Cargo target directories are not
supported by this convenience launcher. It follows `XDG_DATA_HOME`, defaults to
`~/.local/share`, and refuses to overwrite an unrelated launcher. To retire the
development checkout, remove its `applications/skellyspeak.desktop` and
`icons/hicolor/256x256/apps/skellyspeak.png` from that data directory.

### Explicit reading help

`language/reading/` owns bounded, source-captured word meanings, translation,
explanations and speech requests outside conversation turns.
`application/commands/reading.rs` registers begin, run, cancel and receipt inspection.
Requests reuse conversation aid contracts and validate captured connection/workspace
authority. Accepted fresh text help earns exploration effort, never skill XP;
saved results, speech playback and failed requests earn none. `application/native_reading.rs` owns graph execution; `reading_attempts` records
source-owned product receipts. Graph records retain producing execution evidence;
validated reading payloads use bounded caches. Closing a card cannot cancel
another consumer or abandon accounting for submitted work.

Text identity includes the exact prompt, output contract, model, configuration,
language context and access/workspace scope. Retained results work without a
network request, including while AI is paused. Explicit retries start fresh work;
gloss repair retains accepted spans and fills unresolved intervals. Receipt
inspection cannot make an older result supersede a newer one. Generated glosses
can be queried locally across passages alongside accepted conversation annotations;
accepted annotations keep their own durable ownership and need no AI credentials.
Speech and Drill reference playback use the shared local speech result lifecycle.

### Shared transcription execution

`application/native_transcription.rs` owns recognizer graph admission and settlement.
The shared host supplies credentials, transport and classified response evidence. Exact audio bytes,
model, endpoint/account/workspace, captured language/variety/tag and prompt context
identify a result. No text normalization, audio preprocessing or recognition
settings change accompanies reuse. Validated text and timing are evictable local
payloads. Format 74 also retains the accepted source WAV and original transport
content in the private shared inference cache, independently of normalized results.
Format 75 adds blob-reference indexes without changing retained records. Stream
chunks still commit individually; metadata-only capture events skip cache pruning,
and new blob bytes trigger eviction only when the shared capacity is exceeded.
Hidden server processing changes are not discovered by local request identity.

`speech/recording/voice.rs` retains per-recording inspection, Chat/Drill ownership,
Drill reliability assessment and publication. A recording captures its workspace
before asynchronous work. Duplicate submission of its ID cannot create another
product attempt. A removed owner cannot receive a late result, while submitted
provider work still settles its shared receipt. Continuous listening keeps its
existing accepted-take queue and captured-visit publication behavior.

`transcription_attempts` remains the product receipt, marked as a shared-execution
consumer from reservation. It never independently implies a paid dispatch. Usage
counts the shared execution once globally and once per matching language/partner
scope; Drill-only consumers do not acquire partner attribution. Provider metadata
and timing/confidence diagnostics survive success, failure and cache-write failure.
Views attach the current shared receipt, including settlement after owner departure.
The existing capture/reservation pause and access checks remain in force; reuse
does not authorize starting a new recording while execution is paused.

### Private computation originals

`ai/content.rs` stores original request/response bytes and accepted microphone WAVs
under the learner's inference-cache capacity and LRU policy. Graph execution and
recording identities remain durable when payloads are evicted; inspections disclose
availability, byte count, digest, credential removal and complete/partial/interrupted
state. Content reads validate immutable captured bytes after releasing the store lock.
Transport capture is at the app-to-inference-service boundary. Transcription retains
actual multipart field values and WAV bytes separately. It does not claim to contain
an upstream provider exchange that the app never received. Actual credentials remain
excluded, while prompts, transcripts and response content remain available privately.
Diagnostic logs and exports retain their separate content-free policy. Older missing
originals cannot be reconstructed by this migration. Conversation inputs retain all
contributing recording IDs, including across restart and edited-message acceptance.

### Manual Drill storage

`drill/` owns immutable phrases, attempts with grapheme/word comparisons, and
language-scoped sessions/visits. A recording captures its visit before dispatch;
receipt completion and the attempt's transcript/comparison/pending audio publish
in one transaction. File publication can be retried without retranscribing.

Recording retention defaults to 500 MB, with 100 MB, 2,000 MB and keep-none
options. Native pruning covers pending SQLite bytes and WAV files, oldest first;
transcripts, comparisons and usage receipts remain. References use a separate
16 MiB regenerable cache. Pruned files are marked unavailable before deletion,
and interrupted cleanup is retried on startup. In-progress replay owns loaded
bytes rather than a file handle. SQLite incremental vacuum reclaims freed media
pages after pruning, file publication and phrase deletion. Export includes the
database and `drill-audio`; explicit Factory Reset removes both. Persisted format changes follow the complete workspace migration chain.

### Transcription boundary

`ai/audio.rs` owns the provider-neutral request, language identity, result and
outcome. Recording resolves language and recognizer context from a conversation or
Drill item owner, and captures execution settings once. `ai/transport/` owns language conversion, prompting,
provider HTTP formats and response decoding. Fluency analysis consumes normalized
timing only; inspection shows bounded redacted diagnostic metadata separately.

`speech/recording/results.rs` retains each published recording's normalized
transcription and timing independently of the inference cache. Inspection checks
audio identity before reusing that timing. Explicit Drill attempt deletion removes
its result; audio retention pruning preserves the transcript and timing.
`speech/alignment.rs` retains original and normalized synthesis character timing
with generated audio and derives words for inspection without replacing those
arrays. Missing or incompatible timing leaves visual word alignment unavailable.

Model selection is shared by Hosted sign-in and Custom URL. Both routes use
SkellySpeak service contracts for text, transcription and speech. Provider
credentials and provider-specific transports belong on the server. There is no
model or route fallback after provider failure. New workspaces default to
`whisper-large-v3` transcription and `eleven_v4_turbo` speech. Format 54 restores
saved v3 speech selections to v4 once; later learner choices are preserved.
Shared declared capabilities select alternatives before dispatch when needed.

Workspace formats migrate through the supported consecutive chain. Desktop
credential access and deletion use `com.freemocap.skellyspeak.credentials`.

### Persona generation

Generated persona descriptions use the learner's captured explanatory language
and variety. Target writing guidance applies only to the person's name; pragmatics
informs the described personality, not the description's language. The generation
schema requires a romanized name or JSON null according to the same configured
capability used by persona validation. Bundled default personas remain unchanged.
Language choice is a prompt instruction; structural validation does not identify
the language of generated prose.

### Reviewed phrase generation

Drill and persona proposals use `application/commands/proposal_execution.rs` for
structured completion, admission, cancellation checks and refusal handling.
`ai/generation/` owns their proposal registry and `generation_attempts` consumer
receipts; workspace graph records retain producing execution receipts. Each feature owns
its prompt and response projection. Every Generate action dispatches fresh work;
there is no prompt deduplication or reusable proposal cache. Closing a submitted
proposal cannot abandon its execution accounting or authorize late adoption.
Pause and access changes still stop execution under the existing authority policy.
Activity shows the consumer state alongside the shared execution outcome; usage
counts each submitted execution once, including failed product publication.
Drill task text
is editable under `content/prompts/drill/`; difficulty instructions reuse the
conversation configuration.

`drill/previews.rs` owns durable candidates and transactional, IDs-only acceptance.
`drill/conversation_source.rs` extracts exact sentence spans without inference and
revalidates their source before adoption. Schema 36 uses fresh development data.

### Workspace migrations

Workspace format **72** upgrades supported files from baseline **45**. Format 72 adds prospective daily streak storage without backfilling activity or altering XP. Runtime tracking is app-wide only; the original scope column remains for format compatibility. The format
covers SQL and persisted JSON; application release numbers are independent.
`storage/store/migrations/` owns consecutive steps and frozen historical contracts.
`storage/schemas/` owns the current schema, including settings and result tables
formerly initialized separately during startup.

Under the existing workspace lock, startup validates the source, creates a
consistent database recovery copy in `migration-backups/` beside the workspace,
then runs all required steps inside one immediate transaction. Each step validates
its output before advancing `user_version`; final validation checks current DDL,
integrity, references and the learner-facing snapshot before commit. Normal
execution recovery and cache maintenance happen afterward. A failed step rolls
back the entire chain and reports source/target versions, step, stage and redacted
cause metadata. A commit/rollback failure reports uncertainty rather than claiming
success. Backups include committed WAL records, are retained without automatic
pruning, and are deleted by an explicit Factory Reset. They contain private local
data; they are not uploaded. Database-only steps leave audio files and credentials
untouched. Recovery copies are for manual recovery; there is no automatic restore
or new restore UI in this change.

Pre-45 formats, unknown identity, damaged schemas and newer formats are refused
without automatic reset. Newer-format errors ask for a newer app. Fresh workspaces
are created directly at the current version. Reopening a current workspace does
not run migrations or create another recovery copy.

For each future format change:

1. Increment `SCHEMA_VERSION`, append exactly one `N → N+1` step and retain every
   earlier step. The registry rejects gaps, duplicate steps and unaccompanied bumps.
2. Freeze the step's SQL/JSON conversions and output validator. Do not make old
   conversions depend on current serialized types, current defaults or mutable
   schema files. Update the current fresh-workspace schema separately.
3. Give new fields meaningful defaults, explicit backfills or optional values;
   preserve identities and relationships through restructuring. Review constraint
   changes against real historical fixtures. Preserve unknown information as unknown.
4. Document every deletion: ownership, references and whether its information is
   preserved, reconstructible or explicitly retired. Preserve evidence, awards and
   their policy provenance. Level/XP formula changes require an explicit historical
   credit policy, not a reset or implicit re-award.
5. Test the step, every supported source version to current, repeated startup,
   mid-chain failure rollback, retained product history and fresh/upgraded equivalence.
   Keep frozen historical fixtures independent of current schema generation.

Migration code must not commit independently or modify external files/credentials.
SQLite table rebuilds must preserve references and finish with foreign-key checks;
complex changes requiring another transaction strategy need explicit review.
See [implementation and verification](../docs/notes/workspace-migrations.md).

### Speech configuration and captured routes

`configuration/speech.rs` resolves language/variety preferences, learner model
defaults and the shared speech capability catalog. `ai/connections/speech_routing.rs`
combines that decision with the existing service access route. Recording, automatic
persona speech, explicit message playback and reading/Drill reference speech all
capture the selected model through this boundary; adapters do not select alternatives.
Listed language matches take priority over models configured with
`allow_unlisted_languages`. If no listed match is available, those models can be
selected with reason `unlisted_language_attempt`, preserving the canonical tag.
This is a best-effort request, not a claim of verified language support.

`recording/preflight.rs` checks configured service availability before single or
continuous capture. It holds no workspace lock during network or credential work,
and revalidates the access revision and language context before opening the mic.
The availability check is metadata-only, with no inference or quota reservation.
Read-aloud validates the canonical language before queuing; the service checks its
voice and credentials before inference. Provider failure does not trigger rerouting.
See [speech content](../content/speech/SPEECH_README.md) for authoring and precedence.
