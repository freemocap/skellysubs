# Native graph record transaction boundary

Status: isolated foundation implementation, 2026-10-08. Extends the
[durability contract](ai-graph-durability.md) and
[resident-state contract](ai-graph-state-bounds.md). This implements atomic primary
record upserts, bounded audited reads, typed stored transition/inspection reads,
format-4 integrity commitments, explicit payload eviction with cold continuation
and aggregate native record-read budgets
in the shared core and disposable SQLite owner fixture. The workspace SQL adapter
and format-57 storage migration are now implemented in `ai/graph_store/`; see the
[storage checkpoint](ai-graph-production-integration.md#workspace-graph-storage-checkpoint).
Production domain/scheduler wiring, automatic eviction policy and an aggregate
working-set budget remain unfinished.

## One native representation

`record_store.rs` borrows the interpreter's actual `Run`, `AttemptRecord` and
`Execution` values. `RecordChanges` selects new or changed primary rows by native
key, comparing shared record pointers first and values when pointers differ.
It contains no scheduling or alternate snapshot model. Unchanged loaded values produce
no upsert even when a command appends an event. Rows loaded for mutation are upserted
when the previous payload is cold; equality does not require reloading the previous
value. Compaction has no record changes.
There is no deletion transition or implicit history pruning.

`RecordWrite::bytes` serializes a bounded envelope containing its key
and native value. The key identifies a run, attempt or execution within the owner
engine. Derived indexes are omitted. This is private, content-bearing persistence
data and must never be used as an inspection DTO or logged. No frontend catalog or
renderer model is introduced.

Runs without step permission, attempts and executions without evidence retain physical format 1 and its
exact encoding. Executions with the native optional evidence field use format 2.
The same changed-row transaction, hash verification and typed decoder apply; there
is no independently maintained evidence table. Explicit null evidence is invalid;
absent evidence retains its historical unknown meaning. Format-5 checkpoints commit
partial observations and final reports through this same boundary. The
[durability contract](ai-graph-durability.md#execution-evidence-and-report-settlement)
defines those events and protected read APIs.

Executions with protected provisional source capture use physical format 3 and
checkpoint format 6. They still use this same native record, changed-row selection
and integrity checks. Metadata-only format 2 and absent-evidence format 1 retain
their exact encodings. The optional capture cannot be silently erased with null.

Executions with adapter-classified structured metadata use physical format 4 and
checkpoint format 7, including when provisional capture is also present. Scalar
metadata and absent-evidence records retain their previous encodings. Structured
evidence remains protected native data and does not enter graph inspection DTOs.
See the [structured evidence contract](ai-graph-core-semantics.md#structured-response-evidence-extension--2026-10-09).

Runs with a single-step permission use physical run format 2; checkpoint format 8
introduces the Step event. Omitted permission decodes as none and preserves old
record bytes. Replay checks the permission against the event history; recovery
revokes it. These are native envelope upgrades, with no SQL rewrite or change to
released workspace migrations. See the [step contract](ai-graph-core-semantics.md#single-step-extension-2026-10-09).

The row envelope version is independent of checkpoint formats 1/2/3/4/5/6/7/8 and production
workspace format versions. The fixture's `graph_record` table is not a production
migration. Production adoption requires its own consecutive workspace migration.
Future physical row changes must preserve this format or introduce a versioned
conversion; do not silently reinterpret saved values.

## Atomic write ownership

`CommitRequest.records` is mandatory. `CommitStore` commits every supplied upsert
in the same transaction as stamp comparison, current owner authority checks,
publication, checkpoint replacement and optional archive retention. An adapter
must roll back partial record writes on `Rejected`; `Indeterminate` still poisons
the host until reload. Provider invocation is exposed only after known commit
success. The test adapter injects failure after a row has already been written,
including during adoption after publication insertion.

The core validates each changed envelope against `DurableLimits.checkpoint.bytes`
before calling the adapter. This existing byte ceiling also bounds each row read
at recovery. It is a per-row limit, not an aggregate transaction/RSS guarantee.
The fixture additionally caps encoded writes at two million bytes. State record
counts continue to constrain candidate sizes; settlement reservations still cover
the checkpoint rather than guaranteeing storage success.

The allocator, capacity, holds, exact artifacts and event history still belong to
the checkpoint. The three primary tables alone are not a restartable engine.
Generating a write set visits only loaded primary maps; cold unchanged rows stay
in storage. The write set itself owns keys and borrowed
references, and adapters serialize rows individually.

## Fallible reads and recovery audit

`RecordStore` exposes `record_count` and `read_record`, each bound to the complete
expected checkpoint stamp. Each adapter operation checks that stamp in its own
read transaction. Missing rows, stale stamps, read failures and byte-limit failures
are explicit errors. The adapter must check stored length before allocating the
body and prevent a concurrent change from bypassing that bound.

Recovery first performs the existing full native checkpoint/archive replay. Before
any Recover event or write, it compares the stored row count and every bounded row
envelope with that replay's exact native records. Extra, missing or changed evidence
fails; it is never silently repaired or replaced. Byte equality also rejects an
unsupported envelope, wrong identity or differently encoded body. The core checks
returned length as well, although the adapter must enforce the pre-allocation bound.
No untrusted row is installed as executable state.

Stamp-bound reads cannot silently mix revisions: an intervening commit makes a
later read fail. If a commit follows the final read, the existing commit CAS rejects
the stale host before effects. This is the final owner-transaction contract, not
temporary protection for application use during the refactor.

Current-record verification is required for executable recovery. Historical
inspection still uses its read-only archive capability and native replay, so
inspection of retained checkpoints does not require current record rows. Owners
adopting this new isolated storage contract must explicitly populate and verify
rows from native replay; recovery does not invent missing rows. No released
production workspace has been converted by this implementation.

## Record access and loading

### Typed adoption reads (implemented)

`record_access.rs` exposes fallible, typed run/attempt/execution reads using the
native record structs. Resident reads share immutable `Arc` records. Stored reads
check the complete checkpoint stamp and byte ceiling, verify committed record
digests derived from native state, and decode the common format-1 envelope
into those same native structs. They return the decoded stored payload; they do
not substitute resident data after a failed read. The common generic envelope is
used by serialization and decoding, and rejects unknown fields.

`adoption.rs` contains one adoption decision for both access implementations. It
checks the artifact/node, active ownership, cancellation, current attempt identity,
attempt ownership/state and successful producer result. Durable adoption reads
its run, current attempt and producer, then borrows the loaded values for
the existing atomic owner-publication commit. Missing, stale, oversized, unreadable
or changed rows fail before publication or checkpoint mutation. Replay and the
native Adopt transition use the same decision with resident access. The core
`available` helper now returns owned values; an adoption decision itself retains
shared record handles and does not copy its outcome payload.

The integrity catalog below replaces full resident payloads as stored-read evidence.
Native mutation loads the required cold rows; reservation may additionally read
other cold runs and current attempts. Authority at
publication remains the owner transaction's responsibility, even after successful
reads. No alternate adoption policy exists in the storage adapter.

The loaded handles live through the adoption transaction and are then released;
there is no additional long-lived payload cache. Each read has a byte ceiling,
but decoded records, candidate payloads and suffix encoding can coexist. Stored
reads no longer serialize a resident payload for comparison. This step does not
establish a peak-memory or working-set bound.

### Typed dispatch reads (implemented)

`dispatch.rs` owns one producer dispatch decision, used with resident access by
the native Dispatch transition and stored access by the durable host. The host
first loads its selected run/current attempt and checks that consumer's active,
unpaused, uncancelled Prepared state. The producer decision loads the execution,
all current consumer attempts and their runs, and the other unresolved executions
used to calculate occupied resource capacity. It checks producer state, consumer
eligibility and admission, and identifies every Prepared consumer to transition.
Paused subscribers still share an eligible producer's transition, as before.
Scheduling, cancellation and dispatch share the same consumer-eligibility
predicate on the native attempt record.

The invocation binds the decoded stored producer's captured work to its executable.
It is returned only after the native Dispatch event and owner transaction succeed.
Failed reads return no invocation and perform no commit. The native candidate
rechecks the same producer decision using loaded rows or verified cold reads;
storage has no parallel
dispatch policy. Current domain authority remains a final transaction check.
A stale host now fails with `record_stamp` at the first read, before the existing
commit CAS could report `stale_checkpoint`.

Current-consumer enumeration uses the resident immutable association index to select
IDs, then typed reads to check current pointers and eligibility. Unresolved-execution
enumeration still uses a resident index. Capacity is still checkpoint-owned scalar state.
The selected owner/attempt is read again during consumer validation; there is no
new cache. This establishes fallible payload reads, not disk-backed query indexes
or an aggregate memory bound. Unresolved rows for other resources are read too;
resource partitioning remains a query optimization, not a second admission policy.

### Typed scheduling and dependency reads (implemented)

`dependencies.rs` owns the single topological node-state pass and input/guard
resolution. These functions now return `Result`: a failed read is an error, never
`Absent`, `Waiting`, `Blocked` or `Skipped`. A successfully adopted producer must
have a successful result; a missing result is an integrity error. An omitted
optional port within a valid result is still legitimate absence. The same functions
serve resident inspection/output queries and stored scheduling. No renderer or
adapter computes its own dependency semantics.

Durable Advance uses the native event-candidate builder with typed stored reads.
It reads run policy/input records, current attempts, adopted results, exact-reuse
candidates, unresolved resource occupants and consumer eligibility. Native ordering,
oldest eligible producer reuse, explicit retries and resource holds are preserved.
Storage errors abort the event before any owner commit or returned work.

`staged_access.rs` provides reads within that one candidate. Unchanged records use
the supplied reader. Records created or changed by earlier scheduling decisions in
the same event use the candidate's native records. This distinction is determined
by shared record identity against the event's immutable base, not by a failed read.
Every run is loaded before scheduling can change its current-attempt pointers;
new attempts/producers cannot yet exist in storage. There is no read-error fallback,
second state schema or independent scheduler. In particular, a second consumer can
subscribe to a producer created earlier in the same Advance without an intermediate
commit. A later read failure discards earlier allocations, holds and subscriptions,
including their IDs, together with the candidate's capacity change.

The candidate builder also supplies the resident path with the same event logic.
The durable host commits the candidate, its row changes and checkpoint once. No
preview scheduler is maintained alongside the native reducer. The subsequent
transition-read step below extends the same boundary to the other event reducers.

Candidate selection, exact-key/unresolved/consumer-association indexes and
allocation/control scalars remain resident. Queries enumerate identities, without
dereferencing cold payloads. Read requests can repeat within a pass; no aggregate
working-set bound is claimed. Stored values are verified against committed digests.

### Remaining transition and inspection reads (implemented)

Demand, pause, cancellation, retry, settlement and recovery now use typed records
for their decisions. `consumer_reads.rs` selects IDs from immutable execution
associations and checks current membership from loaded attempts/runs. Dispatch,
scheduling, cancellation and settlement share this query and the existing native
eligibility predicate. Stale historical attempts remain retained but do not regain
current membership. The primary-scan test oracle now checks this typed query.

Cancellation reads its newly staged cancellation state when deciding whether a
queued producer has any remaining eligible consumers. Settlement and recovery
can fail after earlier candidate mutations; the complete event is still discarded
before any commit. Repeated recovery checks relevant stored records even when it
will be a no-op, and does not detach unchanged paused records. Begin checks identity
membership and authored artifacts without requiring an existing payload.

`DurableEngine.read_inspection` and `read_outputs` use stored records with the same
native state/reason calculation, projection, disclosure and output resolution used
by resident inspection. They return an error instead of a partial view on any failed
required read. The complete artifact remains in every successful snapshot, including
dormant nodes. Resident helpers remain available for the current in-memory engine;
these are access methods, not different graph definitions or visualization modes.
Historical inspection still reconstructs its immutable cut from native replay.

### Revision-bound integrity catalog (implemented)

`record_evidence.rs` stores only record keys, SHA-256 digests of canonical format-1
row envelopes and the complete committed checkpoint stamp. Recovery builds it from
validated native replay after the existing exact row audit. Each commit candidate
updates it from the same native `RecordChanges` passed to the owner transaction;
bounded serialization supplies both integrity evidence and row-size validation.
Only acknowledged commit success installs it. Rejection retains the old catalog;
uncertain acknowledgment freezes the host until recovery. Compaction changes the
catalog stamp without changing row digests.

Stored reads no longer require a `RuntimeState` reference or serialize an expected
resident record. They check known identity, read under the catalog stamp, enforce
the byte ceiling, verify the digest, then decode/check the native envelope. The
catalog's source-derived hashes are private and never enter inspection diagnostics.
Format-4 bases persist the commitments at each compaction cut; current evidence is
still derived by native replay and updated through the same write set. Metadata
grows with record count, and candidate updates copy its key/digest map. This is not
a working-set guarantee or a production workspace migration.

### Explicit eviction and cold continuation (implemented)

For committed logical revision `r`, let `D_r` be its complete native record map,
`I = domain(D_r)` its identities, `L` its loaded payload map and `C` its integrity
catalog. The storage invariants are:

```text
domain(L) is a subset of I
L[k] = D_r[k] for every loaded k
C[k] = SHA256(canonical_format_1_envelope(k, D_r[k])) for every k in I
```

Native indexes remain projections of `D_r`; they select IDs and cannot override
the reducer's eligibility rules. A candidate can temporarily contain new/changed
rows; its row writes, control state, event and owner effects commit atomically
before it replaces revision `r`. Eviction changes `L` to the empty map while
preserving `I`, `D_r`, indexes, logical revision, full graph topology and history.
Compaction changes the physical checkpoint stamp, not that logical revision.

`Records` retains ordered identities separately from optional loaded `Arc` payloads.
Exact reuse, consumer-association and unresolved indexes contain IDs, not payload
references. Mutation installs a verified row only in the event candidate before
copy-on-write changes. Rejection drops that entire candidate, including loaded rows.
Stored decision reads remain mandatory; `LoadedAccess` lets native effect rechecks
and capacity reservations use already trusted resident/candidate values, loading
missing rows with the same verified reader. No failed read falls back to memory.

`DurableEngine::evict_records` first compacts the suffix, then clears every loaded
primary table. Compaction writes format 4: ordered `(RecordKey, SHA-256)` commitments,
allocator, capacity and holds. The active checkpoint no longer owns a second full
record snapshot. Its empty event suffix also releases the engine's retained input
and settlement event copies. Artifacts, indexes and digest metadata remain resident;
returned invocations/results are owned by their callers and are not reclaimed.

The old checkpoint is archived atomically. Failed compaction leaves the loaded host
intact; uncertain acknowledgment poisons it. Empty legacy bases are repacked against
their existing archive parent without creating an empty archive or changing logical
revision. Formats 1/2/3 retain their original decoders and integrity encodings. Replay
compares format-4 commitments with actual reconstructed native records before the
current-row audit. A valid outer checksum cannot legitimize forged commitments.

`state_usage` continues to count retained identities; `resident_usage` counts loaded
primary payloads. Eviction does not reset admission allowances or delete any record.
Stored inspection and output reads leave payload tables cold and return the same
native topology, facts and values. Resident-only helpers explicitly report
`record_not_resident` when data is unloaded. Subsequent events retain their mutated
rows until another explicit eviction; read-only dependencies are not cached.

### Aggregate record-read admission (implemented)

`DurableLimits.record_reads` declares a `RecordReadLimits` pair: maximum row-read
calls and maximum aggregate encoded row-envelope bytes per public owner operation.
The allowance is mandatory and independent of logical retained-record counts,
individual-row size limits and checkpoint/history budgets. Zero is valid and
refuses operations that need a row read. Repeated reads count again; this measures
read work rather than distinct records or simultaneous residency.

For the sequence of successful stored reads `b_1 ... b_n`, admission requires:

```text
number of row-read calls <= record_reads.records
sum(length(b_i)) <= record_reads.bytes
next adapter byte ceiling = min(per_row_ceiling, remaining_aggregate_bytes)
```

`read_budget.rs` wraps the owner's store for the complete apply, claim, adopt,
compact, stored-inspection, stored-output or recovery operation. Reader construction
does not reset that ledger: decisions, native mutation and final reservation scans
share it. Executable recovery's row audit and subsequent Recover event also share
the enclosing allowance. Eviction uses compact's allowance. Native resident-only
queries require no stored reads; artifact/history reads retain their separate bounds.
Storage-internal authority SQL and writes are not counted as native row reads.

Count exhaustion refuses before the adapter call. Byte exhaustion constrains the
adapter before body allocation, then checks the returned length again. If the
aggregate remainder is the tighter bound, its byte refusal is
`record_read_byte_limit`; otherwise the individual row limit remains
`record_byte_limit`. Count refusal is `record_read_count_limit`. Other storage
faults retain their code/path. These errors never become missing inputs, skipped
nodes, partial snapshots or successful partial events.

A budget refusal discards the whole candidate and exposes no invocation or
publication. The caller may explicitly change this non-persisted owner policy with
`set_record_read_limits` and retry; no automatic retry occurs. A poisoned host still
requires reload. Checkpoint settlement reservations reserve encoded event space;
they do not promise success under an insufficient read allowance. No completed
evidence is deleted and no identity is consumed to satisfy these limits.

This is **not** an aggregate allocation or peak-memory guarantee. It excludes
decoded representation expansion, resident/candidate payloads, newly produced
values, index/catalog copies and checkpoint serialization. Startup still performs
full replay before the current-row audit. A genuine working-set budget must account
for those owners rather than treating serialized bytes as heap usage.

`durable.rs` remains cohesive at 505 lines: it coordinates the single native owner
transaction and effect boundary. The budget adapter and its tests have separate
owners; splitting the coordinator solely to cross the size guideline would obscure
the complete operation boundary. Further responsibilities should trigger review.

### Remaining memory and integration work

Startup and historical readers still reconstruct the complete native history.
Indexes and integrity catalogs remain resident and grow with retained identities.
There is no automatic eviction policy or aggregate allocation/working-set budget.
Aggregate read-work, per-row, logical count, checkpoint, settlement and archive bounds apply, but
do not establish peak RSS. An aggregate budget must cover the entire event,
including fanout, candidates and checkpoint work, and reject it atomically if it
cannot fit. Paging must preserve one event and one owner transaction.

Production workflows/storage/IPC/viewer integration and the corresponding workspace
migration remain separate work. The fixture implementation is not a running-app fix.

Full topology and node facts remain the native inspection projection. Storage
failures must not appear as absent graph inputs, skipped nodes or an incomplete graph.

## Verification

- Isolated graph suite: **92 passed**. Three new tests cover changed-row selection,
  shared dispatch, rollback after partial record writes and publication insertion,
  compaction without row rewrites, bounded reads, missing/extra/changed/oversized
  rows, stale stamps, no-op events, lost acknowledgments and reopening from disk.
  Existing recovery/authority tests now exercise mandatory record auditing too.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  application TypeScript/Vite build and IPC registration regressions (**2 tests**)
  passed. UI tooling used approved elevated execution for Windows ancestor access;
  Vite retains its existing bundle-size advisory.
- Documentation entry-point checks, local graph-note links and whitespace checks
  passed. All graph Rust files remain below 500 lines; the largest is 454 lines.
- The SQLite adapter is an isolated fixture. No live-app/provider verification,
  production migration, commit, push or deployment was performed. Changes remain
  uncommitted.
- Full native library suite on the final source: **946 passed, 6 failed, 6 ignored**.
  The same five conversation activation/model-metadata failures and one
  migration-default failure recorded in the resident-state note remain. The full
  suite is not green; no assertions or product policies were weakened.

### Typed adoption access verification

- Isolated graph suite: **95 passed**. New tests assert the exact three-record
  dependency read sequence, failure before commit at each read, successful retry
  after a read failure, rejection of missing/oversized/altered run, attempt and
  producer records, unchanged publication/checkpoint state on refusal, native type
  equality and separate decoded storage ownership. Existing lifecycle and owner
  transaction tests exercise the shared adoption decision.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  documentation entry-point links and whitespace checks passed. Physical record/checkpoint formats and
  inspection wire types are unchanged. No production integration, live-app
  verification, commit, push or deployment was performed.
- Full native library suite on the final native source: **949 passed, 6 failed,
  6 ignored**. The same five conversation activation/model-metadata failures and
  one migration-default failure remain; no existing assertions or policies were
  weakened. Concurrent microphone/UI edits were preserved and are outside this
  native change's verification scope.

### Typed dispatch access verification

- Final isolated graph suite: **97 passed**. Final Clippy (`--lib --tests -- -D
  warnings`), binary checks and fast gate passed; contract verification,
  documentation links and whitespace checks also passed.
- Full native library run: **951 passed, 6 failed, 6 ignored** (342.59 seconds).
  The five known conversation activation/model-metadata failures and migration
  reading-default mismatch remain. That run preceded the final extraction of the
  shared eligibility predicate; the complete graph suite and Clippy passed after
  that extraction. A focused rebuild initially collided with the running Windows
  test executable; it passed once the full run released the file.
- New shared-producer regressions inject read failures at every distinct owner,
  attempt, producer, subscriber and occupied-capacity dependency. They verify
  no commit, unchanged Prepared consumers, successful retry, atomic shared Running
  state and exactly one invocation using the captured input.
- Missing, oversized and changed records are rejected across those dependencies
  before writes. The stale-host regression now expects the earlier `record_stamp`
  failure, while retaining its unchanged-checkpoint assertion.
- Physical checkpoint/row formats and inspection contracts are unchanged.
  Production workflows, graph-input scheduling reads and record unloading are
  not implemented by this step. Concurrent microphone/UI work remains untouched.

### Typed scheduling access verification

- Final isolated graph suite: **102 passed**, including the existing finite
  lifecycle model. Five new tests cover staged producer sharing, read failures
  after earlier allocations, rollback without ID consumption, optional absence,
  missing/oversized/changed dependency rows, false guards, queued/running/retained
  reuse and occupied resource admission. Successful stored Advance events match
  resident replay's work and checkpoint bytes. The optional-input case also
  executes the downstream handler after restoring the failed record.
- Clippy (`--lib --tests -- -D warnings`), fast gate, generated-contract
  verification, binary checks, documentation entry-point links and whitespace
  checks passed. The new dependency, staged-access and scheduling-test files are
  176, 57 and 355 lines respectively; scheduling is 222 lines.
- Full native library suite on the final source: **956 passed, 6 failed,
  6 ignored** (355.43 seconds). The same five conversation activation/model-metadata
  failures and migration reading-default mismatch remain. No existing assertions
  or product policies were weakened; the full suite is not green.
- No production migration, live-app/provider verification, commit, push or
  deployment occurred. Unrelated microphone/UI changes were preserved and are
  outside this native step's verification scope.

### Transition, inspection and integrity-catalog verification

- Final isolated graph suite: **107 passed**. New mutation tests require the
  expected dependency reads for demand, pause, cancellation, settlement, retry
  and recovery, then inject failure at every distinct observed record. They verify
  no commit or state change and compare successful retry with native replay's
  checkpoint bytes. The independent consumer-query oracle still runs throughout
  the finite lifecycle model.
- Stored inspection/output tests compare native projections, retain dormant
  topology, reject partial views on read failure and retrieve adopted outputs.
  Repeated recovery checks persisted records without committing another event.
  Existing record-sharing tests caught and now guard against needless detachment
  of already-paused runs during recovery.
- Integrity tests drop the engine and checkpoint before reading records with only
  the catalog, verify unknown identities without storage reads, retain old evidence
  after rejection, advance it through successful commits and compaction, and require
  recovery after uncertain acknowledgment. Existing corruption tests continue to
  reject altered, missing and oversized rows.
- Full native library suite on the final source: **961 passed, 6 failed,
  6 ignored** (326.09 seconds). The same five conversation activation/model-metadata
  failures and migration reading-default mismatch remain. Their assertions and
  policies were not weakened; the full suite is not green.
- Final Clippy (`--lib --tests -- -D warnings`), fast gate, generated-contract
  verification, binary checks, documentation entry-point links and whitespace
  checks passed. Persistence formats and inspection wire types are unchanged.
  No production migration, live-app verification, commit, push or deployment was
  performed. Unrelated microphone/UI edits remain untouched.

### Explicit eviction verification

- Isolated graph suite: **114 passed**. Seven new tests cover actual primary-payload
  release, retained identity counts, serialization refusal for partial resident
  state, continuation with eviction between events, shared dispatch, cancellation,
  paused adoption, later demand, retained-result reuse, fresh retry IDs and queued/
  running recovery. Cold projections and outputs match full resident replay.
- Format-4 snapshots contain commitments/control scalars and no primary payloads
  or suffix events after eviction. Formats 2/3 with empty suffixes repack without
  empty archives or lost history. Forged digests, identities, omissions, duplicates
  and ordering fail replay even with recomputed outer checksums.
- Failed cold reservation reads and rejected writes preserve the prior evicted
  host and publication state. Rejected eviction retains loaded payloads; uncertain
  archive acknowledgment requires reload. Repeated eviction is a no-op.
- Clippy (`--lib --tests -- -D warnings`), fast gate, generated contracts, binary
  checks, TypeScript/Vite build and IPC registration regressions (**2 tests**) pass.
  The build required approved Windows ancestor-directory access for esbuild;
  its existing bundle-size advisory remains. No live provider/application,
  production migration, commit, push or deployment was performed.
- Final full native library suite: **968 passed, 6 failed, 6 ignored**
  (363.22 seconds). The same five conversation activation/model-metadata failures
  and the migration reading-default mismatch remain. No existing assertion or
  product policy was weakened; the full suite is not green. Final fast validation,
  documentation entry-point links and whitespace checks pass. Unrelated
  microphone/UI changes were preserved.

### Aggregate record-read budget verification

- Isolated graph suite: **117 passed**. The operation matrix covers claim, adopt,
  settle, advance, pause, cancel, recover, inspection, outputs, compaction and
  executable startup in resident and evicted states. Exact measured byte/count
  limits succeed with the same native snapshot. One-unit-short limits refuse
  atomically, retaining checkpoint, revision, loaded-state counts and publication;
  an explicit retry with adequate allowance produces the expected result.
- Adapter tests verify that repeated reads consume aggregate allowance and that
  each call receives the reduced byte ceiling before allocation. Zero limits refuse
  before row access. Per-row size, missing-row and stale-stamp faults remain distinct;
  a poisoned host cannot be revived by changing its budget.
- Clippy (`--lib --tests -- -D warnings`), final fast gate, generated-contract
  verification, native binary checks, TypeScript/Vite build and IPC registration
  regressions (**2 tests**) pass. Vite uses the previously required Windows
  ancestor-directory access and retains its existing bundle-size advisory.
- Persistence formats remain row format 1 and checkpoint formats 1/2/3/4. No
  production migration, live-app/provider verification, commit, push or deployment
  occurred. The concurrent microphone/UI changes remain untouched.
- Final full native library suite: **971 passed, 6 failed, 6 ignored**
  (330.22 seconds). The same five conversation activation/model-metadata failures
  and migration reading-default mismatch remain. Their assertions/policies were
  not weakened; the full suite is not green. Documentation entry-point links and
  whitespace checks pass.


### Protected node-output reads

The host can read one current adopted node's outputs through the same native
source resolver used for whole-run outputs, with stamp-bound record integrity and
read budgets. Pending independent branches do not hide that adopted value. This
read does not demand, dispatch, adopt or mutate any node; missing, available,
failed and unrequested attempts return no adopted value. Whole-run output
completion semantics remain unchanged. Coverage includes cold records, compaction
and recovery.
