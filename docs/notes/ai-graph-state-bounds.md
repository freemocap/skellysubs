# Graph resident-state admission and loading boundary

Status: record admission, canonical runtime state and independent attempt records implemented in
the isolated foundation, 2026-10-08. The subsequent [record-storage work](ai-graph-record-storage.md)
adds explicit eviction and verified cold reads. An aggregate working-set budget remains
unfinished. Extends the
[retention contract](ai-graph-retention.md), [durability contract](ai-graph-durability.md)
and [historical inspection contract](ai-graph-history.md).

## Implemented record ceilings

`StateLimits` requires separate maximum counts for retained run, attempt and execution
identities. These remain logical ceilings after eviction, not loaded-payload counts.
`DurableLimits.state` is mandatory. Historical reconstruction declares
`HistoricalLimits`, with independent archive and state limits. Budgets are owner
admission policy; they are not graph topology, activation policy, execution identity,
an eviction heuristic or a new persisted field.

`state_usage` derives counts directly from native identity sets; `resident_usage`
reports loaded primary payloads separately. All retained records count, including inactive runs,
superseded attempts, cancelled consumers and settled/retained producers. A run that
has produced its outputs can still accept later on-demand requests. Cancellation
and journal compaction or payload eviction do not release record allowances because none removes
those records. Historical evidence is never deleted to satisfy a budget.

For native state `s`, admission requires:

```text
|runs(s)| <= limits.runs
|attempts(s)| <= limits.attempts
|executions(s)| <= limits.executions
```

Checks occur before inserting a run or allocating an attempt/producer. The complete
event candidate is also checked before replacing the current engine. An Advance
that cannot admit all selected records fails atomically: earlier candidates, allocated
IDs and holds from that rejected event do not leak into memory, storage or returned
work. The previous committed state remains inspectable. The fault vocabulary exposes
`run_limit`, `attempt_limit` and `execution_limit` explicitly.

Resource eligibility is decided before record allocation. A concurrency-held node
creates no records and consumes no record allowance. A pending subscription or
retained result needs one new consumer attempt and no new execution/resource slot.
Deterministic candidate ordering, oldest eligible producer selection and existing
allocator-overflow errors remain intact. Retry requires a fresh attempt and producer;
it cannot overwrite old failures to make room.

Dispatch, settlement, adoption and recovery mutate existing records and need no
additional record slots. The separate checkpoint byte/event reservations still
protect their encoded growth; record counts are not substitutes for those reservations.
Current domain authority and transactional publication checks remain mandatory at
adoption, including when the producer was shared or retained.

Replay installs limits before applying any event, for all supported checkpoint formats.
Lower limits can reject an old workspace state or historical read before any recovery
write. Historical reads audit the whole retained chain, including after the selected
cut, so that entire audit must fit. Callers must surface the refusal or explicitly
choose adequate limits; there is no silent reset, history omission or weaker fallback.

The in-memory `Engine::new` remains the unrestricted reference/test entry point.
`with_state_limits` supplies explicit record admission for direct core use; durable
owners and historical readers always supply limits. Changing ceilings does not alter
checkpoint encoding, artifact fingerprints or operation reuse keys.

## Canonical state and resident record ownership

The interpreter owns one `RuntimeState` from `ai/graph/state.rs`: runs, attempts,
executions, the identity allocator, capacity and admission holds. Record definitions
live beside that state. Artifacts and bound executable handlers,
owner admission budgets, and the event journal remain separate engine concerns.

Format-3 checkpoint bases contain this exact `RuntimeState`. Separating attempts from
runs introduced that physical encoding. New compactions now write **format 4**:
primary-record commitments and allocation/control scalars, without record payloads.
Format 2
is retained as a frozen inline-history reader in `checkpoint_legacy.rs`; its adapter
has no transition logic and compares retained evidence against current native replay.
Format 4 contains no alternate transition model; native replay recomputes and checks
its commitments. Deserializing a
state does not authorize installing it in a live engine. These isolated checkpoint
versions are separate from production workspace versions; no production schema
change or migration is introduced here.

Each `AttemptRecord` has its immutable run/node ownership and the shared native
`Attempt` value, keyed by its engine-qualified attempt ID. A run stores only a
`current` node-to-attempt-ID map. Retry appends a new attempt record and replaces
that node's current reference; earlier rows remain available and unchanged. Runtime
dependency evaluation, adoption, consumer selection and reasons follow the current
reference directly. They do not load or traverse that node's retry history.

The normalized-record invariants are explicit: each attempt-table key equals its
attempt ID; its owner run and node exist; a run's current reference points to the
latest attempt belonging to that same run/node; and the allocator covers every
allocated attempt and execution ID. Older retry rows retain their own identities,
execution associations and terminal evidence. Reducer construction maintains these
relations; checkpoint replay equality rejects missing/extra rows, wrong ownership,
wrong node/ID and cross-run current references before any state is installed.

`ai/graph/records.rs` owns ordered resident record storage. Transaction candidates
and frozen historical states share immutable run/attempt/execution payloads through `Arc`.
Taking a mutable record copies that record only when it is shared. The record
container owns no scheduling, activation, sharing-equivalence or adoption rules;
those remain in the native interpreter. Pointer sharing between candidate states
is a memory representation detail, distinct from graph consumers sharing one
execution identity.

Dispatch and settlement select their affected current attempts before taking
mutable records. They do not copy run metadata or earlier attempt rows. Adoption
updates one attempt row; pausing changes run metadata without copying attempt history.
Recovery similarly selects unresolved executions, current attempts and runs whose
recovery fields change. A second recovery preserves record ownership as well as
semantic state, although direct raw-engine recovery still appends an event.

This is a concrete change to the interpreter's state ownership, not disk paging.
Map keys/reference tables, holds and the journal still have copying costs. Historical
pages use a derived per-run sorted ID index and allocate only their selected attempt
slice. Full inspection explicitly assembles the run's complete history, while facts-only
inspection does not. A database-backed record loader and
bounded working sets remain future implementation.
Do not describe the resident record container as a storage
adapter or assume that `Arc` imposes a memory ceiling.

`ai/graph/attempts.rs` owns the attempt table and its derived run-to-ID index.
The index is a deterministic function of primary row keys and ownership, rebuilt
on decode and omitted from serialization; format-3 bytes are unchanged. Insertion
updates both in the event candidate. The mutation API exposes only attempt state,
so callers cannot invalidate indexed ownership or identity. Per-run ID vectors
share storage between candidates and detach on insertion; appending still copies
that run's ID vector when shared. After the run lookup, count is constant-time,
cursor rank uses binary search and pages look up only selected rows. This index
contains no activation, scheduling or visualization rules.

`executions.rs` similarly derives an ordered reuse-key-to-producer-ID index. Its
write API changes only dispatch, outcome and uncertainty fields, keeping identity,
work and reuse keys immutable. Scheduling looks up that exact key, then applies
the existing native eligibility predicate in ascending execution-ID order. Failed
and unknown producers remain in the index and retained evidence but cannot be
reused. Explicit retries still bypass reuse. No index independently classifies a
producer as reusable.

The attempt table also indexes immutable execution associations. Native current
consumer lookup visits those attempts and checks their owning run's current
reference. Superseded attempts stay available as history but cannot become current
consumers again. Run activation, pause, cancellation and attempt state remain
reducer predicates. Dispatch and settlement use that same current-consumer query;
they no longer scan all runs to select affected consumers. Both indexes rebuild
from primary rows on decode, are omitted from checkpoint encoding and share ID
vectors between transaction candidates until insertion changes a membership.

These queries remove unrelated records from reuse and consumer lookups. They do
not bound the number of same-key producers or consumers of one producer, and do
not unload payloads. Global run/node candidate enumeration, run recovery
and adoption reservations still traverse resident records. Disk-backed reads
and atomic multi-record writes remain required before the loading boundary is met.

The execution table also derives an ordered unresolved-ID set:
`{ id | executions[id].outcome = None and executions[id].unknown = false }`.
Insertion, outcome settlement and uncertainty marking maintain this set in the
same candidate state as the primary row. Dispatch changes no membership. Decode
reconstructs the set from rows; it has no serialized authority or new format.
Candidate clones share the set until membership changes, at which point its ID
tree detaches. This still has a copying cost proportional to unresolved IDs.

Resource occupancy, prepared dispatch collection, cancellation cleanup, producer
recovery and settlement reservations now visit this set instead of completed
execution history. Resource kind, dispatched state and current-consumer eligibility
remain native predicates. Cancellation cannot free a running producer's resource;
settlement or recovery does. Invalid success settles as a validation failure and
also leaves the unresolved set. Failed, successful and unknown rows remain in
primary history and the reuse-key index. Resource queries still scan unresolved
producers across resource kinds; this is not a constant-time resource counter or
a disk loader.

The internal native `Inspection<A>` makes attempt payload presence a type choice:
full inspection borrows the actual attempt records, while facts-only inspection
uses `A = ()`. It does not return an empty history map that could be mistaken for
evidence that no attempts exist. The shared exported snapshot retains its existing
full-history/page specialization and wire shape.

The application may be temporarily broken during this authorized architecture
rebuild. There is no requirement for simultaneous old/new runtime operation or
temporary compatibility wiring. Required final behavior and preservation of
durable user evidence remain separate obligations.

## What this bounds

The record-storage boundary also requires aggregate encoded row-read/count limits
per owner operation. Those cap stored-read work, including repeated reads and final
reservation scans. They are separate from the retained identity ceilings below
and do not establish a peak-memory bound. See the [read-admission contract](ai-graph-record-storage.md#aggregate-record-read-admission-implemented).

The implemented guarantee is the number of retained identities in each accepted native
state. It prevents admission/reconstruction from growing those sets indefinitely.
It does not claim an exact heap/RSS bound. Payload sizes, decoded snapshot buffers,
artifacts, temporary candidate clones and serialization buffers have separate costs;
existing byte limits remain necessary. Strict peak-memory budgeting needs a separate
allocation/storage argument. The current record ceilings cap lifetime records
per engine even after explicit eviction; they were not silently reinterpreted as
a loaded-record budget. See the record-storage note for the implemented eviction API.

Reaching a record ceiling can stop new work even when disk space is available. This
is explicit backpressure, not the completed long-lived storage solution. Increasing
the declared budget is a caller decision; compaction must not pretend to reclaim
resident runs or results when it only moves event history. Explicit eviction now
releases primary payloads while preserving all retained identities and evidence.

## Audit of records needed by native transitions

| Operation | Required evidence and global decisions |
| --- | --- |
| Begin | Global run identity uniqueness, exact artifact and trusted owner input/access checks |
| Demand, retry, pause, cancellation | Target run, current attempt state, captured policy; cancellation also checks remaining consumers before abandoning a producer |
| Advance | All candidates in stable run/node order; adopted predecessor values; exact reuse lookup across retained producers; global resource occupancy and eligible consumers |
| Dispatch | Exact prepared attempt, current owner authority, all eligible subscribers and global resource occupancy |
| Settle | Exact producer, typed outcome validation and every current Running subscriber; preserve independent cancelled/adopted consumers |
| Adopt | Exact Available attempt/result, current source/access authority and atomic domain publication |
| Recover | All unresolved producers and current Prepared/Running attempts; pause all runs, with no reissue or republishing |
| Inspection | Complete artifact and native node facts independently of attempt pages; values needed to evaluate native dependencies and guards |

This rules out dropping an idle run or loading only the directly named record.
Examples: a new run can reuse an old producer; a cancelled producer's other consumer
can still be eligible; an adopted predecessor can feed a later demand. These relations
must remain queryable even when their payloads leave RAM.

## Loading protocol and remaining aggregate budget

The subsequent [record transaction boundary](ai-graph-record-storage.md) implements
atomic upserts, bounded current-row auditing, typed interpreter reads, mutable
loading and explicit eviction. The requirements below remain the contract for
production integration and aggregate working-set admission.

The durable owner must provide transaction-consistent native record access and derived
indexes for run identity, exact producer reuse, current execution consumers and resource
occupancy. Storage indexes may accelerate those queries but must not become a second
implementation of node semantics. The native reducer continues to own states, ordering,
identity allocation and legal transitions; the viewer continues to consume its projection.

Reads need an explicit missing/corrupt/unsupported/over-budget result, not a value that
looks like an absent graph input or a skipped node. Writes and index maintenance belong
to the same owner transaction as graph advancement and domain adoption. Loaded records
must retain original engine/run/attempt/execution identities, source scope, typed values,
faults and provenance. Uncertain commits still freeze the host until reload.

Advance and settlement can touch many consumers. Paging their queries must preserve
one logical event and one atomic commit, plus existing candidate/producer ordering.
Silently committing the first page would change the state machine. If the complete
operation cannot fit its declared working budget, refuse it; a future staged write
protocol must prove the same observation and continuation semantics before replacing
that refusal. Provider work is exposed only after known commit success.

A new physical storage representation needs its own versioned checkpoint/record
contract and, at production adoption, a consecutive workspace migration. Formats 1,
2, 3 and 4 and their audit readers retain the same transition semantics. These
foundation formats do not change a production schema or workspace version.

## Verification

- Final isolated graph suite: **78 passed**. Eight new tests cover each record
  ceiling and whole-event rollback, resource-held work and allocator overflow,
  shared/retained producers with independent adoption authority, zero budgets,
  settlement/adoption/compaction/recovery at exact ceilings, lower replay budgets
  for both checkpoint formats and historical reads, lost acknowledgments, and
  crash recovery without reissuing work. No earlier policy assertion was weakened.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  application TypeScript/Vite build and IPC registration regressions (**2 tests**)
  passed. Vite retains its existing bundle-size advisory. Build/test tooling used
  approved elevated execution for Windows ancestor-directory access.
- Documentation entry-point checks, 39 local graph-note links and tracked whitespace
  checks passed. All graph Rust source/test files remain below 500 lines.
- Existing historical-inspection changes were preserved in the shared checkout.
  No commit, push, deployment, production migration or live-provider/UI verification
  was performed. The changes remain uncommitted on `graphs`.
- Full native library suite on the final source: **932 passed, 6 failed, 6 ignored**.
  The same five activation/model-metadata failures and one migration-default failure
  documented at the preceding checkpoints remain: extra automatically scheduled
  reading helpers, absent fixture model metadata, and migrated `on_demand` versus
  fresh `automatic` reading. The complete native suite remains non-green.

These are implementation tests and resource-admission checks. Cold-state paging,
production migration and the independent refinement gate remain incomplete.

The subsequent [finite shared-lifecycle model](ai-graph-refinement.md) begins
independent transition-conformance verification before storage access changes.
It does not add loading/unloading or change the resident admission guarantees.

### Canonical-state refactor verification

- Isolated graph suite: **82 passed**, including the unchanged 400-state,
  5,600-edge lifecycle model. Three new tests exercise record ownership across
  shared dispatch, independent cancellation, settlement and adoption; plain state
  serialization/decoding; and repeated recovery/rejected-event isolation.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract check,
  documentation entry-point links and tracked whitespace checks passed.
- Full native library suite: **936 passed, 6 failed, 6 ignored**. The same five
  conversation activation/model-metadata failures and one migration-default
  failure remain. Their assertions and policies were not weakened; the complete
  native suite is still non-green.
- The refactor introduces no production schema or wire-contract change. No
  commit, push, deployment or running-application verification was performed.
  All earlier uncommitted changes were retained.

The earlier verification section describes the admission checkpoint; this
refactor changes the actual interpreter and checkpoint state ownership.

### Independent attempt-record refactor verification

- Isolated graph suite: **85 passed**, including the unchanged finite lifecycle
  model. New checks exercise 33 attempts across three-record pages, sharing and
  retry-row independence, metadata-only mutation without history copies, mixed
  format-1/2/3 archive replay and historical reads, and rejection of forged owner,
  node, ID, current-reference, missing-row and extra-row evidence.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  documentation entry-point checks, 47 local graph-note links and whitespace checks
  passed. All graph Rust files remain below 500 lines; largest is 454 lines.
- Full native library suite: **939 passed, 6 failed, 6 ignored**. The same five
  conversation activation/model-metadata cases and one migration-default case
  reproduce their previous failures. No assertions or policies were weakened;
  the full native suite remains non-green.
- This step normalizes attempts, changes new snapshot encoding to format 3, and
  removes full-history collection/sorting from historical page reads. Production
  workspace schemas and inspection wire contracts are unchanged. No running-app
  verification, commit, push or deployment was performed. Changes remain uncommitted.

### Attempt ownership index verification

- Isolated graph suite: **86 passed**. The added test checks exact format-3 row-map
  serialization, reconstruction of each run's sorted IDs from primary rows, empty
  ownership lookups, and candidate isolation across state changes and insertion.
  Existing paging, cursor, forged-evidence, mixed-format replay and finite lifecycle
  tests also pass.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  documentation entry-point links and tracked whitespace checks passed.
- Full native library suite on the final source: **940 passed, 6 failed, 6 ignored**.
  The same five activation/model-metadata failures and migration-default failure
  listed above remain; the full native suite is not green. No policies or assertions
  were weakened.
- Checkpoint snapshot payloads are now boxed to keep the internal enum compact;
  serialization and supported formats are unchanged. There is no new production
  storage or viewer integration, and no commit, push or deployment was performed.

### Producer and consumer query verification

- Isolated graph suite: **88 passed**. Two new traces cover multiple producers
  for one exact key, failed and unknown producer exclusion, fresh retries alongside
  retained successes, oldest eligible producer selection, superseded consumer
  references, independent cancellation, pause, recovery and immutable candidate
  snapshots. State serialization reconstructs all indexes after every tested event.
- A primary-record scan oracle independently checks indexed key membership/order,
  current-consumer membership and pause-aware eligibility. It runs in both new
  traces and throughout the existing 400-state, 5,600-edge finite lifecycle model.
  This verifies these queries within those scenarios; it does not complete the
  broader storage/refinement proof.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  documentation entry-point links and tracked whitespace checks passed. Persisted record encoding and
  inspection types are unchanged. Disk loading, production wiring and resource
  occupancy indexing remain unfinished. Changes remain uncommitted.
- Full native library suite on the final source: **942 passed, 6 failed, 6 ignored**.
  The same five conversation activation/model-metadata failures and one
  migration-default failure remain. The full suite is not green; no existing
  assertions or policies were weakened. No live-app verification was performed.

### Unresolved-execution query verification

- Isolated graph suite: **89 passed**. The new regression verifies that cancelling
  all consumers retains a running producer's occupancy, malformed success releases
  it through terminal validation failure, rejected dispatch preserves state and
  journal, queued recovery removes unresolved membership, and older snapshots
  retain their own index. Completed records remain in primary history.
- The independent primary-record query oracle now checks exact unresolved IDs
  and ordering throughout the existing lifecycle model and execution-query traces.
  Serialization checks reconstruct indexes after each trace event.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  documentation entry-point links and whitespace checks passed. No persisted format,
  inspection wire contract, production workflow or viewer was changed by this step.
  Disk loading remains unfinished; changes remain uncommitted.
- Full native library suite on the final source: **943 passed, 6 failed, 6 ignored**.
  The same five activation/model-metadata failures and one migration-default
  failure remain. The complete suite is not green; no assertions or product
  policies were weakened. No live-app verification, commit or deployment occurred.
