# Workspace migrations

Status: implemented; verification results below. Approved direction: preserve
workspace history through consecutive upgrades beginning at format 45. This
supersedes the previous development reset policy for supported workspaces.

Current format: **73**. Format 72 → 73 marks compatibility for the learner-owned Appearance motion preference. Existing JSON remains unchanged; an absent motion value means Follow system. The marker prevents older builds from opening a workspace after the new field is saved. Format 71 → 72 adds workspace-owned daily streak facts and presentation claims. It starts empty, retains activity after source-message deletion, and changes no existing XP or evidence. Format 70 → 71 makes cache payload ownership graph-only. Format 69 → 70 gives native assessment receipts explicit producer engine/run ownership. Format 68 → 69 adds message-owned native helper requests for explicit regeneration. Format 67 → 68 adds workspace-owned native engines and bounded reading projections. Format 66 → 67 adds message-owned native playback requests and a read-only union of conversation run owners. Format 65 → 66 is a no-op compatibility marker retained for development workspaces that may already have opened at 66. It changes no data, checkpoint contracts or SQL ownership. Format 64 → 65 adds native consumer-owned audio delivery
evidence: the delivered audio digest, exact source message, adopted attempt and
receipt. It adds no permanent audio copy and preserves all historical rows.
Delivery records follow native engine, turn, message and receipt deletion; cache
eviction leaves them intact so inspection can verify already-delivered audio.

 Format 63 → 64 adds native audio receipts and evictable
cache associations under the existing cache budget. Format 62 → 63 adds native assessment receipts and
receipt-owned disclosure decisions without rewriting historical assessment data.

Format 64 is additive: it does not convert or delete legacy execution receipts,
cached audio, recordings, messages or learning evidence. Native audio receipts
belong to their graph engine/execution and retain original/normalized alignment
as protected source data. Cache eviction removes only the payload association;
receipt deletion follows graph ownership, under the current revision-retention
policy when that host integration is completed. Audio bytes share the existing
blob store and capacity setting. Provider response and billing evidence stay in
the native execution log. Fresh SQL and the frozen migration are kept identical.

Format 61 → 62 adds optional effect-owned reply source
reservations. New graph reply publication uses the identity admitted for downstream
reading; existing messages and effects are unchanged. See the format-62 section
below for ownership and validation details.

Format 60 → 61 extends native reply effect roles to partner
replies and openings. It transactionally rebuilds the effects/publications tables,
preserving every effect ID, publication, message link, captured attribution and
award. At that historical format, publications are the only foreign-key children of effects and are copied
before either table is replaced. Historical attribution is restored without
rechecking admission against mutable current turn settings; new inserts retain
the owner/channel/attribution checks. Existing coach records are not reclassified.
This enables publication for subsequent workflow conversion; it does not switch
partner execution to the graph runtime by itself.

Format 59 → 60 adds initially empty native producer-to-wire
identity bindings. Engine/execution, artifact and operation contract identify the
producer; immutable unique wire attempt/operation IDs are staged in its Dispatch
transaction. Existing graph bytes, publications, messages and learner credit remain
unchanged. Historical requests are not inferred or backfilled. Bindings share the
engine's lifetime and conversation deletion cascade; they are transport provenance,
not independently earned learner evidence. See the
[wire identity checkpoint](ai-graph-production-integration.md#producer-wire-identity-checkpoint).

Format 58 → 59 adds initially empty graph effect and
publication tables. Stable domain effects capture owner, output binding and credit
attribution independently of attempts. Publication provenance binds the accepted
message to exact native attempt/producer IDs. Legacy messages and awards are
unchanged; migration does not manufacture historical graph results. See the
[publication checkpoint](ai-graph-production-integration.md#accepted-publication-checkpoint).

Format 57 → 58 adds immutable turn executor/channel
ownership. Existing turns remain legacy; retained primary operations establish
their channels, missing evidence becomes unknown, and conflicting evidence rejects
the migration. Original turns, attempts and credit are unchanged. New turn admission
writes ownership in its command transaction, and startup rejects missing ownership.
Graph references are deferred until commit so first engine admission remains atomic;
constraints reject cross-conversation ownership. Graph workflow routing remains
subsequent work. See the
[turn ownership checkpoint](ai-graph-production-integration.md#turn-execution-ownership-checkpoint).

Format 56 → 57 adds initially empty native graph engine,
record and archive tables. It does not convert historical turns or execution rows,
alter preferences, rewrite learner evidence or award credit. Each engine belongs
to one conversation and exact artifact catalog; conversation deletion cascades to
its protected native records and archives. See the
[graph storage checkpoint](ai-graph-production-integration.md#workspace-graph-storage-checkpoint).
This is a workspace format change, not an application release version change.

Format 54 → 55 adds imported-reading records; 55 → 56
preserves both imported tables while admitting explicit editorial review alongside
source checking. See [the reading expansion](reading-preload-plan.md#expansion-checkpoint-2026-10-07). Format 53 → 54 restores v4 Turbo for saved v3 selections after listening tests reproduced an accent regression; plain-source synthesis remains in place. See the format-54 section below. The 51 → 52 step adopts the bundled v4 Turbo synthesis default while preserving historical receipts and custom model IDs. The preceding format 51 introduced explicit skill/subskill conversation targets. The 50 → 51 step preserves records and expands the closed direction contract. The 49 → 50 step removes only the coaching activation
preference: coaching is required on new turns. Captured turn policy, operations,
learner revision, evidence and awards remain unchanged. The removed preference
is intentionally retired; the three optional-work preferences retain their values.

 The 48 → 49 step adds the four execution preferences to
learner preferences. It preserves learner revision, accepted operations, captured
turn policy and saved results. Defaults match new workspaces; malformed existing
values fail validation and roll back.

The 47 → 48 step cancels unfinished assessment operations
while retaining completed results, source messages, awards and attempt metadata.
Interrupted running attempts retain unknown provider/billing outcomes. New turns
use the combined ten-question assessment. See the
[ownership and verification record](eight-group-learning/content-implementation.md#workspace-format-48-ownership-treatment).

The 46 → 47 step adds an initially empty
`skill_level_events` table. Existing XP, evidence, settings and award claim flags
are unchanged. Explicit runtime initialization derives catch-up from eligible
credits; ordinary reads and the migration do not award or rewrite XP. Receipts
survive source-conversation deletion to prevent repeat celebrations and contain
only identifiers and levels, not message text. Factory Reset removes them with
the workspace. The released 45 → 46 step remains unchanged.

Verified with the full native suite: real skill credits and XP claim flags survive
both supported starting formats, migration failures roll back, repeated startup
does not repeat upgrades, and fresh/migrated schemas match. See the
[native/UI handoff](skill-radar-levels-handoff-2026-09-30.md) for integration details.

## Scope and behavior

The local native SQLite workspace owns learner history. This change does not
migrate the hosted service or change XP formulas, award amounts or level thresholds.
Application release numbers remain independent of database formats.

Format 46 is the first destination. Its 45 → 46 step makes existing optional startup
tables part of the required schema: skill choices, reward preferences, playback rate,
microphone choice, inference results, recording results and audio analysis caches.
Existing values survive; absent tables/default rows are created. Schema validation
accepts source-file LF/CRLF differences while continuing to reject changed table,
index and trigger definitions. Unknown extra objects are retained, as before.

The frozen version-45 contract contains the original core and generation DDL.
An additional frozen script records optional startup tables and their defaults.
Historical validation never executes these scripts against the user's database;
it builds in-memory reference databases. The migration applies only its frozen
optional-table script and reviewed retirement statements. Future steps must freeze
their own SQL/JSON transformations and destination validators. The current fresh
schema is maintained separately and compared against upgraded databases in tests.

Startup retains the workspace lock, verifies the migration chain and source
identity/integrity/schema, and creates a consistent SQLite recovery image before
migration writes. One immediate transaction encompasses every pending step, its
version update and output validation. Current schema and learner-facing snapshot
validation run before commit. Snapshot validation covers preferences, personas,
conversation settings and saved topics; it is not an exhaustive semantic audit of
every historical result JSON. Each future migration must validate the records it
transforms, including any newly required JSON shapes and meaning.

Ordinary interrupted-work recovery, cache pruning and session-receipt cleanup run
after successful migration. They retain their existing behavior, including the
workspace revision increment at startup. Runtime schema creation has been removed
from those recovery paths; small feature bootstrap helpers remain test-only.

Pre-45 files are refused without modification. Newer files ask for a newer app.
Unknown, damaged, failed-to-migrate and unsupported files are never automatically
reset. A missing/duplicate registry step or a format bump without a corresponding
step fails explicitly. Current workspaces skip the migration and backup path.

## Retirement review

The first step moves the existing retirement of `inference_holds`,
`drill_references` and `inference_profiles` out of ordinary startup. These are
retired access lockouts and regenerable discovery/reference caches, not learner
evidence, awards, conversation records, recording ownership or durable execution
receipts. Integrity and foreign-key checks run before and after the step. No
other product records are deleted by this migration. The recovery image also
retains these tables when present in the source.

Historical effort awards retain their source identity, policy, timestamps and
claimed state. This change neither recalculates XP nor issues new awards. Future
formula changes need an explicit product decision about derived level displays
and historical credit; the migration framework does not make that decision.

## Recovery ownership and limitations

Recovery files live in `migration-backups/` beside the database, named with source
and destination versions plus a unique token. SQLite `VACUUM INTO` includes
committed WAL pages. A private staging file is checked for database integrity,
flushed and renamed before any upgrade. Failures report useful typed diagnostics;
partial-copy removal errors are included rather than ignored. On Unix the recovery
directory and files use private permissions and the directory is flushed after
publication. Windows uses the enclosing application's data-directory access policy.

Copies contain private local database records. They are never uploaded or
automatically pruned. Explicit Factory Reset clears them with the other workspace
data. Files and credential stores are outside the migration transaction and are
not modified by migration steps. These database images are not full backups of
audio files or credentials; recovery is manual, with no new restore UI. Stop the
application and retain the original workspace and sidecars before manual recovery;
do not replace a live SQLite database or combine an old image with newer WAL files.

Failed steps explicitly roll back the complete chain and retain step, phase,
source/target versions and nested redacted cause diagnostics. Failed commit or
rollback operations report uncertainty instead of asserting that data is unchanged.
Retry after a rolled-back failure starts from the original persisted version.

## Verification

Automated tests exercise a frozen baseline independently of current schema
creation, populated history/settings preservation, recovery-copy contents and WAL
records, fresh/upgraded schema equivalence, current-version reopening, unsupported
and damaged sources, backup refusal, JSON validation rollback, registry gaps and
duplicates, multi-step upgrades from each source version, failure after earlier DDL
and data changes, and successful retry after rollback. A supplementary application
flow verifies that conversation ownership, original messages and continued reading
survive the first migration.

Verification on 2026-09-30:

- `cargo test --manifest-path native/Cargo.toml --lib --quiet`: 738 passed,
  2 failed, 5 ignored. All ten migration tests and the existing startup/storage
  suites passed. The two failures are unrelated Arabic content fixture mismatches:
  `drill::bundled::tests::previews_preserve_scripts_and_selected_variety_without_writes`
  and `language::languages::citation_tests::arabic_gloss_prompt_snapshot_has_explicit_scheme_and_preserves_source`.
  Neither those tests nor their content sources were changed here.
- `cargo clippy --manifest-path native/Cargo.toml --lib --tests -- -D warnings`:
  failed on four findings outside this change: `chunks_exact_to_as_chunks` in
  `ai/transport/speech_stream.rs`, `collapsible_if` in
  `application/commands/workspace.rs`, `manual_is_multiple_of` in
  `speech/stream_delivery.rs`, and `cloned_ref_to_slice_refs` in
  `learning/effort/tests.rs`. No migration-file findings were reported.
- `cargo fmt --manifest-path native/Cargo.toml -- --check`: passed.
- `git diff --check`: passed.

After the final startup ordering review,
`cargo test --manifest-path native/Cargo.toml --lib storage:: --quiet` passed all
51 storage tests, including all ten migration tests.
No live learner workspace was opened or upgraded during these tests; no deployment
was performed. All work remains uncommitted.

## Format 51: skill-directed conversations

The conversation direction's closed `topic` union admits a `skill` value with a
main `skillId` and optional `subskillId`. Format 50 → 51 validates the source
contract and expands the permitted values without rewriting any settings,
messages, captured turns, assessment records, receipts or earned awards. No SQL
objects, audio files or credentials change. The migration runner owns the recovery
copy, version write and transaction. A format-50 application rejects a format-51
workspace before attempting to interpret its skill targets.

Guide attachments are optional metadata in new turn contexts; existing contexts
need no backfill. Guide translation cache payloads keep their existing shape;
source action references are derived at retrieval time. Migration tests cover
all supported starting formats, recovery-copy reuse, populated-record preservation,
rollback and repeated startup. No live learner workspace was migrated by the agent.

## Format 53: speech reliability default

Format 52 → 53 changes only `ai_config.audio_settings.speech.model` from
`eleven_v4_turbo` to `eleven_v3`, incrementing the connection revision when changed.
Other model selections and settings remain intact. The released format-52 step
is preserved, so older supported formats still traverse the complete chain.
At format 53, fresh workspaces also started with v3. Shared declared capabilities select v4 when
v3 cannot serve the language; there is no retry after a provider failure.

The existing setting has no provenance distinguishing the former default from
an explicit v4 choice. Both switch to v3 once under the approved reliability
policy; a subsequent explicit learner selection remains respected on reopen.
This is a workspace format increment, not an application release version change.
No conversation, receipt, learner evidence, audio blob, cache, or file migration
is involved. The ordinary migration runner provides transaction rollback and a
recovery copy. Tests use temporary workspaces only.

## Format 54: restore speech fidelity

Format 53 → 54 changes saved `eleven_v3` speech selections to `eleven_v4_turbo`
once and increments the connection revision only for changed rows. The learner
explicitly approved including deliberate v3 selections because format 53 retained
no provenance distinguishing them from automatic changes. Later explicit model
choices remain respected; other models and transcription settings are preserved.
Historical migration steps remain unchanged.

Fresh workspaces and the server's advertised default prefer v4 Turbo. Shared
capability routing retains v3 for languages without listed v4 support, including
Irish. Exact-source synthesis without accent instructions remains in place.
The [listening investigation](arabic-speech-investigation-2026-10-06.md) records
the reproduced v3 accent failure and accepted v4 Arabic/repetition controls.

No conversation, audio, cache, evidence or receipt is deleted or rewritten by
this migration. The existing transaction and recovery-copy protocol applies.
Tests cover every supported start, all non-settings tables across the new step,
unchanged custom models, rollback, fresh-default equivalence, repeated startup
and learner changes after migration. Tests use temporary workspaces only.


## Format 62: graph reply source reservations

Implemented: format 61 → 62 adds `conversation_graph_reply_sources` and guards
linking each optional reservation to its declared reply effect. The message ID is
reserved at native Begin and used by publication at Adopt, so downstream graph
reading inputs and the actual conversation message refer to the same source.
The reservation is not an execution ID, an award ID, or a placeholder message.

Ownership review: the effect owns the reservation and deletes it by cascade.
No message, effect, publication, learner evidence or award is rewritten or removed.
Existing effects have no reservation and retain their established publication
behavior; previously published IDs remain unchanged. New declarations derive
reservations from the executable's actual reply-source bindings, not node names
maintained by the viewer. Conflicting source bindings and already-used message IDs
are rejected within the owner transaction. Publication cannot use another ID.

This is an additive workspace-format migration, not an application release bump.
Historical migration SQL remains unchanged. The normal recovery-copy and atomic
chain protocol applies. Tests cover rollback, preservation of existing publications,
fresh-schema equivalence, immutable reservations and exact-source publication.
Broader verification and running-application conversion remain separately tracked
in [production integration](ai-graph-production-integration.md).

## Format 63: native assessment receipts

Implemented: format 62 → 63 adds `conversation_graph_assessments` and
`conversation_graph_disclosures`. A receipt binds the domain result to its exact
learner message, graph turn owner, node, native attempt and execution. Its opaque
ID is used by existing product receipt fields, including the historically named
`inferenceAttemptId`; it does not identify or create a legacy attempt. Native
history decides whether a receipt is current; these tables do not mirror runtime
state or scheduling. Disclosure choices belong to that receipt, never to the next
assessment of the same message.

Ownership review: source deletion cascades through its native assessment receipts
and their disclosures. This follows domain ownership and does
not remove earned lifetime awards. Existing legacy assessment/disclosure tables,
messages, learning evidence and awards are unchanged. Historical results remain
readable through their existing ownership route. No backfill invents native
provenance for old attempts. Current graph and historical domain reads share one
database transaction; invalid native history fails rather than falling back to a
legacy result.

The migration has rollback, preservation, source-ownership, immutability and
cascade tests. Product publication tests exercise real application captures and
native adoption, including rejected adoption, disclosure and restart without
duplicate credit. Full verification is tracked in the production integration note.


## Format 67: requested speech ownership

`graph_speech_requests` binds an explicit playback run to its existing assistant
message, turn, native engine/artifact and scope. It contains no runtime state or
audio bytes. `graph_conversation_runs` is a SQL union of these owners and existing
turn owners for host routing; executable artifacts still own topology and state.
Engine ownership is deferred until the same Begin transaction commits, matching
existing native admission. Immutable ownership and source/conversation checks
prevent retargeting requests.

Ownership review: deleting a source message or its turn removes its playback
ownership and deliveries through existing foreign keys. Graph execution history
may remain, as approved; no selective history-erasure mechanism is introduced.
Existing messages, receipts, learner evidence, awards, cached audio and execution
history are unchanged by migration. There is no backfill or fabricated legacy row.
The new request flag `regenerate` is optional and omitted when absent, preserving
old serialized command receipts. Migration failure rollback and preservation are
covered by the format-67 test; supported-start and repeated-startup coverage is
part of the full native suite.


## Format 68: workspace native execution

Adds workspace-owned native engines, records, archives, transport identities and
immutable product run ownership. These use the same graph storage adapter and
native commit/replay contracts as conversation engines. Existing conversation
engine identities and records are unchanged. No historical execution is rewritten
or synthesized. The owner is the existing learner/workspace, without creating a
fake conversation for standalone work.

`workspace_reading_cache` is an evictable projection linked to its native run.
Its payload uses the existing shared blob LRU and capacity setting. It can be
removed without deleting native provenance, reading receipts or exploration
awards. It is not a second execution ledger. Fresh requests get a new reuse scope;
cache eviction ends reuse of that cache generation. Provider results already
submitted may settle after their consumer closes, without publishing or crediting
the closed consumer.

The consecutive migration is additive and transaction-only. It retires no data
and touches no audio files or credentials. The additive upgrade/rollback test
preserves messages and checks schema equivalence; the supported-format chain and
repeated-startup checks remain required before step-5 closure.

## Format 69: fresh helper ownership

Implemented and verified in step 5.11. The additive
`graph_helper_requests` table binds each explicit fresh helper run to its native
engine/artifact, existing turn/message, reuse scope and operation contract.
The conversation-run view includes these owners, so scheduling, source checks,
usage and source deletion use the existing native machinery. Ownership is
immutable and constrained to the engine's conversation and message's turn.

Ownership review: no message, result, evidence, award or execution is deleted or
rewritten. The previous conversation-run view is reconstructible from its owner
tables; replacing that view adds helper owners while preserving its earlier rows.
Fresh and upgraded databases use the same definition. The migration is one
transaction; rollback, all supported starting versions, history preservation and
fresh/upgraded schema equivalence passed. See the production-integration note for
the test-run evidence and corrected historical-target fixtures.

## Format 70: explicit assessment producer ownership

Implementation under verification in step 5.12. Assessment receipts now name the
producing engine and run, so feedback from an explicit helper is not attributed
to the original conversation graph. Attempt uniqueness is scoped to the native
engine. Receipt IDs, sources, result JSON and disclosure IDs remain unchanged.

Ownership review: the migration copies both assessment and disclosure tables in
its transaction, reconstructs them with explicit ownership, and restores all
rows. Existing producers are backfilled from their immutable turn execution
owners. No result, disclosure, evidence or award is intentionally retired. Any
missing/inconsistent ownership or uniqueness violation fails the migration and
rolls back. The historical format-63 schema is retained for migration tests;
fresh databases use the format-70 definition. Preservation, rollback and supported
upgrade/restart checks are required before closure.

## Format 71: graph-owned cache payloads

The current cache stores shared blobs and learner-selected capacity separately
from graph execution records. Reading and transcription projections belong to
workspace graph runs; audio projections belong to graph audio receipts. Cache
pruning cannot remove execution evidence.

Ownership review: the user explicitly retired support for inference execution,
consumer and result records. This step drops those three tables and their indexes.
It preserves graph data, product messages, learning evidence, awards, settings and
shared blobs. Normal cache maintenance reclaims unreferenced blobs after commit.
The step changes transactional database structures only. Its regression verifies
failure rollback and byte-for-byte preservation of graph cache rows and settings.
The migration, rollback, schema-equivalence and supported-start regressions passed.

## Format 75: cache reference indexes

Implementation: five additive indexes support blob reachability lookups for
original-content chunks, conversation/workspace speech, reading and transcription.
The consecutive 74-to-75 step changes no row, serialized contract, identity,
ownership relationship, source bytes, evidence or awards. Format 74's schema and
all earlier migration steps remain unchanged. Fresh databases create the same
indexes. This is a workspace format change, not an application release bump.

Verification is recorded with the performance batch in
[the efficiency audit](ai-graph-efficiency-audit.md). The regression covers
transaction rollback, preservation of original records/chunks and reopening;
the existing migration suite covers all supported starting formats and schema
equivalence.
