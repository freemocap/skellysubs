# Graph retention and compaction contract

Date: 2026-10-08. Status: **isolated implementation contract and remaining
preservation requirements**. Extends [the foundations](ai-graph-foundations.md)
and [the durable owner boundary](ai-graph-durability.md). Historical definition
reading, absolute revisions, formats 2/3/4 snapshots and atomic archive compaction are
implemented in the foundation with a disposable SQLite owner. Handler-free
[historical attempt pages](ai-graph-history.md) now share native replay and projection.
Production workspace
formats and existing workflow ownership remain unchanged.

## Evidence format extension

Checkpoint format **5** adds the observed-event profile: `Observe` and
`SettleObserved`, carrying protected native evidence. It uses the same artifact
manifest and ordered native replay. Its optional base has an explicit
`base_format` discriminator selecting retained inline (2), primary-record (3) or
commitment (4) representation; a journal-only root has no base/discriminator.
This avoids an untagged decoder and preserves numeric record identities.

The first observed event upgrades the active payload to 5. Subsequent captures and
compaction retain that version, even when its current suffix is empty. Formats 1–4
retain their exact historical encoding, reject observed events/extended execution
payloads and cannot resume a format-5 evidence-bearing prefix under an older label.
The existing archive chain remains readable and its original bytes are preserved
when archived. Unknown base representations, versions and fields are errors.

The optional native execution evidence field is omitted when absent. Explicit null
is rejected rather than silently normalized away from a historical record. Present
evidence uses physical execution-row format **2**, participates in the same native
record commitments and survives compaction, eviction, replay and recovery. Other
rows keep format 1. These numbers are independent of workspace format 56; this
checkpoint does not change production SQL or the application version.

See [report settlement](ai-graph-durability.md#execution-evidence-and-report-settlement)
for partial-prefix, finalization, identity and recovery rules.

### Provisional-content extension

Checkpoint format **6** extends format 5's explicitly selected base representation
with optional protected provisional capture on observed events and execution
evidence. The first such event upgrades the payload; later captures/compaction keep
6 even with an empty suffix. Formats 1–5 retain their exact encoding when the field
is absent. They reject provisional-bearing events or inline/native bases; archive
replay also rejects relabeling a provisional-bearing commitment base as format 5.
Explicit null provisional fields are rejected rather than silently omitted.

Execution rows containing provisional capture use physical format **3**. Metadata-only
rows remain format 2 and no-evidence rows remain format 1. The sequence acknowledges
an exact source-text replacement, not a text prefix or accepted domain result.
Correction history remains in the native event/archive chain. No production SQL or
workspace format change is made here; workspace integration still needs its explicit
consecutive migration. See the [handoff contract](ai-graph-durability.md#provisional-content-handoff).

## Ownership and observable behavior

Compaction changes where execution evidence is stored and how current state is
loaded. It must not change activation, source identity, reuse, results, adoption,
accounting, retry authority or the graph shown for a historical run. A run with no
currently running work can still contain on-demand nodes; idle is not finished.
Storage pressure cannot cancel that run or make its nodes disappear.

The graph owns the transition state and evidence identities. The domain owner
retains authority over accepted results and their publication transaction. The
workspace owner owns storage locking, recovery copies, migration and explicit
deletion. The viewer consumes native projections; it owns neither retention nor a
replacement interpretation of old events.

The initial retention policy is lossless. Preserve graph definitions, commands,
attempts, execution/consumer relationships, source bindings, outcomes, adoption
records and uncertainty. No age-based deletion, synthetic success, revision reset,
or payload eviction is authorized by this plan. A later cache-eviction policy must
distinguish evictable generated payloads from adopted results and durable evidence.

Finite memory and bounded working records do not imply bounded lifetime history.
When the workspace cannot retain required evidence, admission must fail explicitly.
Compaction is not permission to erase history to make a write succeed.

## Audit before compaction (historical baseline)

| Current implementation | Consequence for compaction |
| --- | --- |
| `Engine` stores the entire journal and all runs/executions in memory. | Removing events alone neither bounds live state nor unloads historical runs. |
| `Inspection.revision` and `Stamp.revision` equal journal length. | Truncation would reset logical time and break snapshot ordering and stale-writer checks. |
| Checkpoint format 1 contains the exact artifact manifest and complete journal. | An event prefix cannot be removed without a new, versioned recovery representation. |
| Replay applies transitions but requires matching `Executable` bindings. | Removed handlers prevent runtime reconstruction even when historical evidence remains valid. |
| `CommitStore` atomically replaces one checkpoint with domain publication. | It has no contract for atomically retaining an archive segment before shortening active history. |
| Settlement reservations assume a fixed manifest plus append-only serialized events. | A replacement format needs a fresh size argument; the current proof must not be reused implicitly. |
| Inspection exports all attempts under explicit byte/attempt limits. | Long histories require native pages and completeness metadata, not dropped attempts or missing graph nodes. |

These are shared-runtime concerns. No conversation, gloss, translation or provider
operation needs a special retention path.

## Separate logical identity from physical representation

The checkpoint formats use these distinct concepts:

- **Logical revision:** monotonic position in the engine's event sequence.
  A compaction is not a new execution event and does not change this revision.
- **Base revision:** the last event already incorporated in a state snapshot.
- **Suffix:** events after that base. Its length is a storage quantity, not the
  engine's logical revision. Require `revision = base + suffix.length` with checked
  arithmetic and lossless serialization to clients.
- **Checkpoint stamp:** engine identity, logical revision and exact representation
  digest. Compaction changes the digest, so pre-compaction writers still fail CAS
  even when the logical revision is unchanged.
- **Archive segment:** an immutable, versioned range of original events and its
  integrity/linkage metadata. Use contiguous half-open ranges `[start, end)` with
  event `i` representing the transition from revision `i` to `i + 1`.

Attempt/execution IDs remain qualified by the original engine identity. Their
allocation high-water mark survives every snapshot, even if a later cache policy
unloads payloads. Do not renumber records or create a new engine to evade limits.

The snapshot is the core's state after the retained prefix, not a UI graph or
materialized collection of operation-name-specific records. Preserve captured
inputs/scope/policy, demand/retry/cancellation sets, run flags, attempt histories,
execution work/keys/outcomes/uncertainty/dispatch state, resource capacity, holds
and the ID allocator. A field may be omitted only with a demonstrated deterministic
reconstruction that preserves the full observation and transition behavior.

## Atomic compaction transition

Define `Obs(s)` as the artifact identities, complete topology, run/attempt facts,
values available for adoption, outputs, sharing associations, eligibility and
recovery obligations of state `s`. Layout and storage byte counts are not part of
`Obs`. Compaction must satisfy:

```text
Obs(before) = Obs(after)
logicalRevision(before) = logicalRevision(after)
retainedEvents(before) = retainedEvents(after)
```

Implementation sequence under the existing workspace ownership discipline:

1. Freeze a candidate at one exact checkpoint stamp. Record the versioned native
   state at the proposed cut and the original event prefix to be retained.
2. Validate the candidate snapshot against the same native state and its full
   replay in tests/audit checks. Reject gaps, overlaps, changed artifacts and broken
   execution/attempt/value references. A checksum is integrity evidence, not proof
   of semantic correctness or authentication.
3. Check the replacement working checkpoint's limits and all outstanding settlement,
   adoption and recovery reservations. A larger snapshot can legitimately make
   compaction fail; the old state and reservations then remain unchanged.
4. In **one owner transaction**, compare the old stamp, insert the immutable archive
   segment and its references, publish the snapshot/replacement checkpoint, then
   commit. This transition performs no domain adoption and no provider work.
5. Only after known commit success replace the in-memory representation. An uncertain
   acknowledgment poisons the host until reload, using the existing durable rule.

Do not implement this as “write an archive file, then delete rows.” The first
adapter uses the same transactional store as the checkpoint. External file/blob
storage would require a separately reviewed recovery protocol. Archive insertion
must reject conflicting content under an existing identity. Repeating a completed
compaction must neither duplicate evidence nor reapply adoption.

An in-flight execution does not become completed, cancelled or unknown merely
because its history is compacted. Existing single-use invocations retain their
identities. A real restart still applies the native recovery transition and records
unknown outcomes without invoking handlers.

## Historical reading and executable capabilities

Historical definition access reads the exact saved artifact and uses the same
constant-disclosure projection as live definitions. It must not register dummy
handlers, resolve current operation implementations, recompile a replacement graph,
or infer edges from events. Executable recovery still requires compatible handlers;
read access does not grant the capability to execute.

**Implemented prerequisite:** `Checkpoint::artifact_ids` enumerates the retained
manifest and `Checkpoint::inspection_definition` returns the shared
`DefinitionSnapshot`. The compiler and checkpoint validator share the original
artifact fingerprint algorithm. Decoding rejects a mismatched manifest identity
even if the envelope checksum is internally consistent. The stored checkpoint is
unchanged by reading it. Live and retained exports share one projection function.

Definition reading alone does not reconstruct runtime state. The implemented
[historical reader](ai-graph-history.md) now uses the same native transition semantics
without handlers and preserves native reasons at a selected logical cut. Unsupported
formats fail. Future semantic changes must retain explicit version compatibility;
they cannot silently reinterpret old records with changed transitions.

Checkpoint integrity does not authenticate a workspace, and a definition read is
not a new proof of graph validity. Historical records describe what was stored.
Resume validation and authorization remain stricter, separate operations. Structural
identifiers retain the trusted-native-schema boundary of the inspection protocol;
arbitrary imported user schemas are not a supported source for this API.

History pagination must bind cursors to engine, artifact/run, logical cut and
ordering. Export complete graph structure independently of the selected attempt
page. Return explicit range/completeness and next-cursor information; no viewer
inference from absent rows. Representation-only compaction must not invalidate the
meaning of an already selected logical cut. Deletion and incompatible history
versions must fail explicitly rather than quietly restart a cursor.

## Consecutive migration and staged implementation

1. **Historical definitions:** implemented as described above; no persisted fields
   changed. Active execution continues to require exact executable artifacts.
2. **Revision and snapshot semantics:** separate absolute revision from suffix
   length, specify the complete versioned snapshot and validate references/state.
   Prove identity, observation and continuation equivalence on synthetic graphs.
3. **Checkpoint format 2 and archive transaction:** add the consecutive format-1
   reader/upgrade, immutable segments and atomic compaction in the disposable SQLite
   owner. Preserve format 1 source bytes and event order as retained evidence.
   Existing format 1 fixtures remain unchanged; reject future/unsupported formats.
4. **Historical runtime pages and working-state bounds:** implement versioned native
   reads and explicit completeness, then separate bounded active state from cold
   run/attempt history. Keep on-demand and cross-run sharing behavior intact when
   records are loaded on demand. A snapshot of every lifetime record is only a
   replay optimization, not the completed long-lived storage design.
5. **Production ownership/migration:** only after the independent contracts pass,
   add the explicit consecutive workspace migration with the established recovery
   copy and one-transaction chain. Do not overwrite released migration steps.

Steps 1–3 and step 4's historical attempt export are implemented in the isolated
foundation. [Resident record admission](ai-graph-state-bounds.md) now enforces
explicit count ceilings without eviction. Cold-state loading, strict peak-memory
bounds and step 5 remain open.
Checkpoint format versions, workspace format versions and application releases are
different identities. Production migration, cache eviction and history deletion
are not implemented.

## Implemented snapshot formats and recovery contract

Creation still writes format 1. Explicit `DurableEngine::compact` now writes format 4,
retaining the exact old checkpoint bytes as an immutable archive segment in the
same transaction as the replacement root. Formats 2/3 remain supported historical
encodings. Later compactions retain the exact preceding checkpoint of any snapshot
format. Each archived
checkpoint contributes its suffix range `[baseRevision, revision)`; snapshots can
repeat state, but event ranges are contiguous and original events are not rewritten.
Empty format-4 suffixes make compaction a no-op. An empty legacy snapshot is repacked
against its existing archive parent, preserving every event without introducing an
empty archive segment. Logical revisions use checked `u64`
arithmetic; the resident journal exposes only its suffix after compaction.

The format 2 base records runs with embedded attempt histories, executions, allocator,
capacities and holds. Format 3 uses the canonical runtime state directly: independent
run, attempt and execution tables, with one current attempt reference per run/node.
Format 4 contains canonical primary-row SHA-256 commitments and allocation/control
scalars, without primary payloads. All record their absolute revision and exact parent
archive stamp. Between cuts,
this base and the artifact manifest are immutable. Recovery loads the bounded
archive chain, verifies stamps and checksums, and replays oldest to newest using
the native reducer. At each cut it compares the complete native state with the
stored snapshot before rebasing. Format 2 uses a frozen physical-layout adapter
with no scheduling or execution logic; format 3 compares canonical state directly.
Format 4 recomputes commitments from the replayed native rows and compares every
identity/digest, allocator, capacity and hold. Hashes are private content-derived
evidence, not diagnostics or a substitute for archived source records.
The version-specific decoder preserves original serialization for integrity hashes.
It never installs an unvalidated
deserialized state. Missing history, changed artifacts, malformed snapshots,
unknown versions and differing replay state are errors before a recovery write.
Handlers are required for executable recovery but are never invoked by replay.

This deliberately retains full-history replay on restart. It shortens the active
event suffix; it does not yet accelerate startup or bound peak working memory.
The [record boundary](ai-graph-record-storage.md) now supports explicit payload
eviction and cold reads. Identity/index/commitment metadata still grows with retained
records; logical record ceilings are unchanged.

`HistoryLimits` bounds total archived bytes, events and segment count separately
from the active checkpoint. `CommitStore::read_archive` must check the stored
length before allocating a body. Recovery and compaction reject budget exhaustion
without deletion. Compaction checks the replacement checkpoint plus every reserved
settlement, adoption and recovery event before committing. Its larger snapshot can
therefore cause a valid refusal. Ordinary snapshot appends retain the fixed base
and its format, including an old format-2 base until the next compaction,
so only serialized suffix events and separators grow under those reservations.

The SQLite fixture inserts the immutable archive and replaces the root in one CAS
transaction. Conflicting archive content is an error. A rollback keeps both old
storage and memory intact; a lost acknowledgment freezes the host until reload.
The unchanged logical revision and changed checksum reject pre-compaction writers.
Neither compaction nor replay publishes domain results or issues provider work.

Format 4 is an isolated foundation checkpoint format, not a workspace database
format bump. Production adoption still requires the explicit consecutive workspace
migration. Refer to the [attempt-record verification](ai-graph-state-bounds.md)
for the legacy format-1/2/3 checks and the [record boundary](ai-graph-record-storage.md)
for format-4 commitment, continuation and eviction verification.

## Acceptance evidence required before enabling compaction

| Case | Required evidence |
| --- | --- |
| Every cut of bounded synthetic traces | Pre/post observation equality and identical legal continuation outcomes. |
| Prepared, running, available, adopted, failed, unknown and cancelled attempts | No lifecycle change due to compaction; no invocation/adoption replay. |
| Shared producers, retained reuse and independent consumers | One producer identity, unchanged keys/results, independent current authority. |
| Disabled/on-demand nodes and guarded joins | Full topology and native reasons retained; later demand still works. |
| Nested composition and changed/removed handlers | Exact historical structure; no current-code substitution. |
| Snapshot validation and format upgrade | Malformed references, mismatched identities, invalid state and unsupported versions rejected. |
| Concurrent writer, rollback and lost acknowledgment | Archive and checkpoint commit together or not at all; uncertain hosts freeze. |
| Saturated byte/event bounds and compaction failure | Existing settlement/adoption/recovery obligations still fit; no data dropped. |
| Repeated restart/compaction | Stable logical revisions and IDs; no duplicate archive or domain publication. |
| History pages across compaction | Same logical records/order; explicit missing/unsupported evidence. |
| Every supported production starting format | History, earned credit, source lineage and accepted results preserved transactionally. |

Enumerated tests are bounded evidence. The separate abstract-model/refinement gate
remains required; this proposal does not claim a formal proof.

## Verification of the implemented prerequisite — 2026-10-08

- Final isolated graph suite: **50 passed**. Four new tests cover handler removal,
  changed current implementation/bindings, nested topology and sensitive-value
  projection parity, bounded export, unchanged source evidence, missing executable
  recovery bindings, and a false manifest identity under a recomputed envelope hash.
- Full native library suite on the final source: **904 passed, 6 failed, 6 ignored**.
  The six failures reproduce the existing activation/model-metadata/default-migration
  failures documented in the [inspection checkpoint](ai-graph-inspection.md#verification-checkpoint--2026-10-08).
  No workflow-policy assertions were changed. The full application suite is not green.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  documentation entry-point links and tracked whitespace checks passed. Some
  PowerShell-launched checks stalled before producing output; those processes were
  interrupted and the same commands completed through the Windows command shell.
- This step changes no generated wire shape, UI behavior, provider integration,
  production workspace schema, application version or saved checkpoint encoding.
  No live provider or renderer verification is claimed.

At that prerequisite checkpoint, the snapshot/segment format remained design work.
The subsequent implementation is described above; history pages and production
migration in the acceptance matrix remain unimplemented.

## Snapshot/archive verification — 2026-10-08

- Final isolated graph suite: **62 passed**. Twelve new tests cover every cut of
  a synthetic sharing/cancellation/retry/recovery trace; continued invocations and
  later demand; repeated compaction and absolute revisions; revision overflow;
  transactional rollback, lost acknowledgments and stale writers; archive budgets;
  oversized replacement snapshots; uncertain dispatch recovery; byte/event saturation
  with settlement/adoption/recovery; forged snapshot state; missing/corrupt/wrong
  archives; bounded reads and unsupported formats/fields. These are bounded
  implementation tests, not the independent abstract-model conformance gate.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification
  and the application TypeScript/Vite build passed. IPC registration regressions:
  **2 passed**. Vite retains its existing bundle-size advisory. Its initial sandboxed
  build could not read a Windows ancestor directory; the approved unsandboxed rerun
  passed. No checks were weakened.
- Documentation entry-point links, 20 local links across the four graph notes and
  tracked whitespace checks passed. All graph Rust source/test files remain below
  500 lines. The generated TypeScript change adds the native compaction fault codes;
  it does not introduce a separate visualization interpretation.
- Production database schemas, provider adapters, workflow policies and the live
  viewer are unchanged. No production migration, live-provider verification, commit
  or deployment was performed. Changes remain uncommitted on `graphs`.
- Full native library suite on the final source: **916 passed, 6 failed, 6 ignored**.
  The failures are the same five activation/model-metadata cases and one migration
  default-equivalence case documented in the
  [inspection checkpoint](ai-graph-inspection.md#verification-checkpoint--2026-10-08).
  Their failure details remain unchanged: additional automatically scheduled reading
  helpers, absent fixture model metadata, and migrated `on_demand` reading versus
  fresh `automatic` reading. No workflow-policy assertion or migration was changed
  to obtain a pass. The complete native suite remains non-green.
