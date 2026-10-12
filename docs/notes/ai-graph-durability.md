# Graph durability and authority boundary

Status: isolated foundation implementation contract, 2026-10-08.
Extends [the core semantics](ai-graph-core-semantics.md). Production workspace
migrations and workflow adoption remain outside this checkpoint.

## Ownership and atomicity

The graph owns legal transitions and captured values. The domain owner owns current
source/access authority, publication and its database transaction. A durable host
must commit a graph transition through that owner before exposing its effects.
The persistence interface is a semantic transaction contract, not permission
inferred from an opaque scope string.

Begin exposes captured graph inputs to the owner for comparison with the trusted
source; dispatch exposes the exact recorded work inputs and operation. The adapter
must validate their provenance and current access/configuration as appropriate,
not merely compare two caller-provided strings. The core validates typed bindings;
it cannot certify that a value came from a particular external source record.

Each checkpoint has a UUID engine identity, format version, ordered event revision,
artifact manifest and integrity checksum. Attempt/execution IDs are qualified by
that engine identity outside the process. The manifest retains the exact artifacts;
recovery requires matching implementations, never whichever version is current.
Checksum validation detects corruption; it is not authentication or authorization.

Every storage commit compares the entire previous checkpoint stamp, including its
engine identity, revision and checksum. Initialization requires absence. Within
the same owner transaction, the adapter checks the required current authority,
publishes any adopted output, upserts changed native records, and replaces the
checkpoint. Any known rejection rolls back every effect. An uncertain commit result poisons the in-memory host: it
cannot issue further commands or invocations until storage has been reloaded.

Dispatch names an exact prepared consumer attempt. At least that consumer must
still be authorized under its captured scope; other subscribers retain their own
adoption checks. Durable dispatch intent commits before the single-use invocation
is returned. Authorization linearizes at that commit. Cancellation after it cannot
promise to retract an external effect. Adapters needing another check after an
async credential lookup must perform that check before submitting, and settle
revoked work explicitly; the core cannot make remote services transactional.

Adoption publishes the exact validated Available output together with its graph
transition. The owner checks current source/access authority inside that same
transaction. Publication failure leaves the attempt Available and unpublished.
Replay reconstructs state without invoking handlers or republishing domain data.
Recovery audits bounded current-record reads against that replay before writing.
Recovery changes are committed before a recovered host becomes usable; an already
recovered state needs no additional event.

## Enlisting an existing command transaction

Provisional-content persistence is specified separately below; it uses the same
commit acknowledgement, authority and rollback rules as other producer evidence.

The production command coordinator stages domain changes, the workspace revision
and the idempotent command receipt before its final `tx.commit()`. The graph's
existing `CommitStore` contract can occupy that same final commit point: transfer
ownership of the open transaction to the adapter, rather than lend it a transaction
and acknowledge an uncommitted write. No new graph preparation/acknowledgement state
machine is required for a command admitting one run.

The concrete admission sequence is:

1. Under workspace serialization, check the action receipt and current authority;
   capture source inputs and stage the domain command in the existing transaction.
2. Construct one `Begin` event from those captured inputs. Finish staging the
   workspace revision and command receipt. No invocation is exposed at admission.
3. Move the transaction into a one-shot adapter and apply `Begin`. Record reads
   use that transaction and verify the current graph stamp. `commit` takes ownership
   of the transaction, checks source/authority, writes graph records/checkpoint,
   then commits everything, including the command's staged effects.
4. Only successful database commit installs the in-memory graph candidate and
   permits returning the command receipt. Rejection rolls back all effects. If
   graph validation fails before calling `commit`, dropping the adapter rolls back
   the still-open transaction. Indeterminate commit poisons the host; reload before
   inspecting it or dispatching. The existing receipt resolves command replay after
   reload without admitting another run or publishing duplicate domain effects.

Releasing a savepoint is not this commit. The adapter must not retain an outer
uncommitted transaction after returning success. No further fallible domain writes
follow graph success. The one-shot adapter must be discarded on error, including
errors that occur before its `commit` method is called.

This works for a guaranteed single graph transition such as new-run admission.
It does not establish atomic multi-engine or multi-transition command batching.
Commands whose graph operation can be a no-op must still explicitly commit their
domain transaction; a no-op `apply` may never call the adapter. Do not make their
receipt depend on an assumed graph callback. Review those command cases during
their conversion instead of hiding a nested commit behind `CommitStore`.

The disposable SQLite enlistment fixture uses the same transaction writer as the
existing owner fixture. It covers admission, rejection after record writes,
validation failure before a commit call, acknowledgement loss/reload, and cold
dispatch reads with no invocation returned after rejection. This demonstrates the
contract with a real enclosing SQLite transaction; production command integration,
receipt replay wiring and migration remain outstanding.

### Atomic first-run admission

`DurableEngine::create_with_run` creates the engine and applies its first ordinary
`Event::Begin` before its only storage commit. It accepts no other event. The
initial checkpoint has revision 1, contains the original Begin event and retains
the native run record; there is no committed empty-engine intermediate state.
The commit uses `expected=None` and the same Begin authority/inputs contract as
admitting a run to an existing engine. An existing checkpoint must reject this
create rather than being replaced.

Creation and first admission share the existing initialization path. Both validate
limits, capture the native checkpoint, reserve recovery/settlement space and build
record commitments before calling storage. The first run uses ordinary transition
validation, including input/policy/artifact checks and retained-state limits. It
does not advance, dispatch, invoke a handler or publish a domain result.

The command coordinator may pass its one-shot owned-transaction adapter to this
entry point, with source, authority and receipt already staged. Validation failure
leaves the transaction for the caller to discard; rejection during commit rolls
back all staged owner and native data. Any error returns no usable host. In
particular, an indeterminate acknowledgment requires checking the durable command
receipt and checkpoint before retrying. Recovery keeps its existing pause rules;
a committed initial run is not automatically dispatched after restart.

The first-admission SQLite fixture covers success, pre-commit validation/budget
failures, current source/authority rejection, failure after a native record write,
lost acknowledgment and a conflicting fresh-create retry. An exact-byte test
reserves the recovery event and verifies that one byte less fails before commit.
This extends the existing transaction contract; production adapter/receipt routing
is still unconnected. Empty-engine `create` remains available for existing callers.

## Provisional-content handoff

Implemented in the isolated core: `EvidenceSnapshot`, `InvocationReport` and the
native `ExecutionEvidence` optionally carry the same protected `ProvisionalCapture`.
`Observe` commits the exact cumulative replacement and its sequence. `SettleObserved`
seals final capture and outcome in one transaction. `complete` means a final report
was retained, not successful computation or accepted publication. Read the committed
execution record to establish its retained-through watermark; a callback snapshot
alone is never an acknowledgement. Old metadata-only encodings omit the new field.

After first retention, the callback session cannot change and the sequence cannot
decrease. At an equal sequence the text must be identical. A higher sequence may
replace the text completely; prior committed versions remain in retained history.
A capture failure may be latched without advancing the text sequence, because no
new text was accepted. Once latched, that entire capture cannot change or disappear.
Finalization cannot clear it or turn it into success. Plain `Settle` cannot bypass
the observed-report boundary. A failed transaction leaves the watermark unchanged
and the borrowed report available to the caller.

Consumer cancellation still does not cancel another consumer's shared producer.
Late text may be retained after recovery marks a producer unknown, but cannot make
that producer adoptable or reissue it. Final reports for recovered unknown work
remain rejected. Eviction, compaction and historical reads use the same native
record; no duplicate preview table or frontend interpretation is introduced.

The existing whole-event settlement ceiling includes metadata, provisional content,
output outcome and serialized overhead. Intermediate captures cannot consume the
remaining final-settlement/adoption/recovery reservation. Capture byte limits bound
UTF-8 source text; they do not imply the entire JSON report fits. Production adapters
must choose compatible budgets and retain rejected reports for remediation.
Format 6 has the same header length as 5; the existing one-time header reservation
also covers a direct upgrade from formats 1–4. Tests cover the exact bound and a
one-byte-short dispatch rejection before an invocation is exposed.

Protected native evidence reads and
[materialized live reads](ai-graph-inspection.md#implemented-protected-materialized-read)
are implemented. UI session ordering, invalidation/reconnect delivery and actual
provider callbacks are not wired yet; the
[inspection design](ai-graph-inspection.md#stream-ordering-production-implementation-design)
remains their contract. Current public inspection excludes all provisional text;
the generated live-read wrapper is explicitly protected content inspection.

## Format and limits

The initial checkpoint uses the existing replay journal plus exact artifact
manifest, serialized under explicit byte/event limits. Unknown versions, corrupt
checksums, missing or changed artifacts, invalid transitions and oversized payloads
are errors. Versioned event and artifact objects also reject unknown fields,
including nested graph definitions; a future field cannot be silently ignored
under the existing format. No events, values or history are dropped to fit a bound.
Limits are resource ceilings, not retention policy. Admission now reserves space
for bounded settlements, consumer adoption and recovery as specified below.
Exhaustion remains explicit backpressure; callers must retain uncommitted outcomes
and surface the failure. Format 2 adds lossless archive compaction as described in
the [retention contract](ai-graph-retention.md). Production use still requires
cold-state handling and a workspace migration. Historical attempt export is now
implemented under explicit read/output limits in the [history contract](ai-graph-history.md).

Checkpoint bytes contain source content and must follow workspace content handling.
They are never diagnostic exports. The [inspection protocol](ai-graph-inspection.md)
exports the same topology with explicit sensitive-value omissions. Rich provider
metadata capture and native persistence now follow the contract below; production
adapter classification and public export remain separate obligations.

## Execution evidence and report settlement

`DurableEngine::claim` binds the returned invocation to its UUID engine namespace.
`record_observations(&EvidenceSnapshot)` and `settle_report(&InvocationReport)`
borrow their arguments, so a rejected transaction cannot consume the caller's only
copy. Both commit through the existing owner/CAS/record transaction boundary.
Engine, producer, artifact and operation identities must match. A report from another
workspace/engine cannot settle a coincidentally equal local execution ID.

`Observe` retains a cumulative ordered prefix on a dispatched execution. It neither
completes the attempt nor publishes values. Existing observations must remain an
exact prefix; a capture failure, once recorded, cannot be cleared or followed by
additional accepted observations. Equal prefixes are harmless repeated facts, but
still consume a journal revision and its resource allowance. Every update is bounded
by the declared serialized settlement-event maximum, checkpoint/history limits and
the existing native record-read budget. Failed writes leave prior evidence intact.

`SettleObserved` seals the final prefix and validates its outcome in the same native
transition. Its execution-row upsert, affected consumer states and checkpoint commit
atomically. Output validation, a provider failure or cancelled consumers cannot erase
the recorded metadata. A recorded capture failure prevents successful settlement,
even if a faulty adapter supplied success; an existing outcome failure remains
primary. Adoption still has its own current-authority and publication transaction.
Rejected adoption leaves the completed producer evidence available for inspection.

`Execution.evidence = None` means no observation record was supplied. It is not
equivalent to a completed report with an explicitly empty observation list. The
`complete` flag means a final report was recorded, not that computation or adoption
succeeded. Capture failures and optional usage/billing fields retain their distinct
meaning; missing observations or counts are never filled with synthetic zeros.

Recovery preserves acknowledged prefixes and changes dispatched unfinished work to
Unknown. Later observations may extend its evidence, but cannot make its outcome
known, publish values or authorize another invocation. A late final settlement is
rejected; explicit retry creates separate work under the existing policy. Sealed
reports cannot be overwritten. Compaction, cold reads and handler-free historical
replay preserve the same evidence field. Protected native reads are available through
`read_execution_evidence` and `HistoricalInspection::execution_evidence`; these are
not redacted public DTOs.

The in-memory capture is not an automatic database writer. The host must deliver and
commit snapshots while the provider is running, and await successful final report
settlement before announcing terminal facts. Rejected writes leave caller-owned
snapshots/reports available for handling; unknown acknowledgements poison the host
and require reload. Nothing guarantees retention of an observation that was never
acknowledged by storage. Real adapter delivery, stream sequencing and enclosing
command-transaction enlistment still require production integration.

## Settlement admission contract — 2026-10-08

The durable owner declares `DurableLimits`: hard
checkpoint byte/event ceilings, total retained archive ceilings and a maximum
serialized settlement-event size.
It also requires independent [retained record ceilings](ai-graph-state-bounds.md).
Those apply before new record admission and throughout replay; existing settlement,
adoption and recovery require no additional record slots.
This is an admission contract, not an operation's output type or a provider limit.
Provider adapters must eventually establish compatible response/metadata bounds.
No finite reservation can guarantee storage for an unbounded response.

For candidate state `s`, let `J(s)` be its complete serialized checkpoint, `U(s)`
the distinct dispatched executions without outcome or recovery uncertainty, and
`A(s)` the current, uncancelled consumers in Running or Available state. Reserve:

```text
events(s) = |U(s)| + |A(s)| + needsRecovery(s)
bytes(s)  = |U(s)| * (maximumSettlementEventBytes + 1)
            + (evidenceFormatHeaderGrowth if U(s) is nonempty else 0)
            + sum(encodedAdopt(a).bytes + 1 for a in A(s))
            + (encodedRecover.bytes + 1 if needsRecovery(s) else 0)
```

Every commit must leave `J(s).bytes + bytes(s)` and
`journal(s).events + events(s)` within their respective hard ceilings. Arithmetic
overflow rejects admission. Each extra byte accounts conservatively for a JSON
array separator. The first observed event upgrades formats 1–4 to an evidence
envelope (format 5 for scalar metadata, 6 for provisional content, 7 for structured
metadata), so
dispatch also reserves the exact additional enclosing header bytes once, before
exposing an invocation. Formats 5–7 share the same header size and need no further
header reservation when advancing between them. Intermediate
observations may exhaust unreserved space and be refused, but may not consume the
remaining settlement/adoption/recovery reserve. All supported formats have an
immutable manifest and fixed-width checksum; snapshot bases remain fixed between
compactions. Beyond the one-time header upgrade, appended-event size is the only
checkpoint growth involved in settlement/adoption. Compaction must
recheck all reservations against its replacement snapshot footprint.
Tests must exercise this assumption against actual checkpoint bytes.

Reservations belong to distinct producers and independent consumer adoptions.
Joining a running producer adds an adoption reservation, not another settlement
reservation. Cancelling all consumers does not release the running producer's
settlement reservation. Successful settlement retains adoption reservations;
failed settlement releases them. Available retained results reserve adoption when
a new consumer is admitted. Prepared work has no external effect; dispatch must
reserve before returning an invocation. Ordinary commands cannot consume these
reservations. Storage/authority/commit failures remain possible and are not
covered by the capacity guarantee.

Settlement checks the complete incoming event against its declared bound before
mutation, including malformed successes and structured failures. Oversized
evidence is rejected intact, never truncated, substituted with a generic failure,
or marked settled. The caller must retain the outcome for explicit remediation.
Recovery records uncertain work using the reserved recovery event. Once replay is
already in recovery's fixed point, another recovery does not append an event;
repeated startup must not consume the bounded journal indefinitely. Subsequent
mutations still use the owner transaction's checkpoint compare-and-swap.

This introduces no persisted fields or workspace migration. Reservations are
derived from replayed state. Old checkpoints are decoded unchanged; recovery
under new or smaller limits can refuse before a write if the resulting state and
remaining obligations do not fit. There is no deletion or invented evidence to
force acceptance. Production adoption still needs a reviewed migration policy.

The [retention contract](ai-graph-retention.md) implements archive compaction
separately from reservations. It preserves original checkpoint bytes, source
bindings and outcomes in one transaction with the new snapshot. Absolute revision
does not reset. Recovery audits the full history and compares native state at
every snapshot cut; arbitrary deserialized state is never installed. Future
cold-state handling must distinguish execution caches from immutable accepted
results. Historical runtime inspection now uses the shared native structure and
reducer without requiring handlers; it grants no execution capability.
Inactive runs and on-demand nodes cannot be removed to make admission pass.

## Required evidence

Use real disposable SQLite transactions to verify compare-and-swap rejection,
rollback after tentative domain publication, current authority changes before
dispatch/adoption, independent shared consumers, uncertain post-commit failure,
and recovery without duplicate effects. Also reject corruption, incompatible
artifacts, future versions and resource-limit violations. No production tables or
workspace format versions change to establish this interface.

## Implemented boundary and remaining work

`native/src/ai/graph/checkpoint.rs` owns checkpoint validation and encoding.
`durable.rs` coordinates the existing reducer with the owner's commit transaction.
The [record storage boundary](ai-graph-record-storage.md) supplies native row
upserts and stamp-bound reads in that same transaction contract. Native mutation,
reservations and inspection now support cold rows after explicit payload eviction.
Format-4 bases retain commitments rather than full record snapshots. Full-history
startup replay and aggregate memory budgeting remain separate work.
`DurableLimits.record_reads` now bounds aggregate stored-row reads across the entire
owner operation. Insufficient read allowance refuses before any commit or returned
invocation, including during final reservation checks. It does not replace settlement
space reservations or guarantee completion under an arbitrarily small allowance;
the owner can explicitly change that policy and retry without changing graph history.
The actual SQLite implementation is confined to test fixtures; no production
database tables, command handlers or AI workflows call this boundary yet.

Remaining foundation work is explicit:

1. Extend the implemented archive compaction and historical attempt pages with
   bounded active state and production migration contracts that preserve accepted
   evidence and unresolved external work.
2. Integrate the implemented [inspection export](ai-graph-inspection.md) without a
   second visualization catalog or UI interpretation of missing execution records.
3. Provider/stream metadata needs typed sensitivity handling and bounded retained
   provenance on success and failure. Raw internal inspection is still unsuitable
   for diagnostic export.
4. A separate abstract transition model and bounded conformance checks must state
   their assumptions and coverage before claiming the foundation gate complete.

## Initial owner-boundary verification — 2026-10-08

- Final targeted graph suite: **30 passed**, including eight durability tests
  using disposable SQLite files. Tests cover source mismatch, authority changes,
  rollback, stale writers, lost acknowledgments, sharing, restart and limits.
- Final Clippy (`--lib --tests -- -D warnings`) and `npm run check:fast` passed.
- Full native suite: **884 passed, 6 failed, 6 ignored**. The six failures are the
  same paused-workflow activation/metadata/migration failures recorded in the
  [initial core report](ai-graph-core-semantics.md#verification-record--2026-10-08).
  That full run preceded the final unknown-field rejection hardening; the changed
  core's targeted suite passed separately on the final state, including rejection
  of unknown nested event and graph fields.
- Documentation entry-point checks, all 38 local links across the four graph
  notes, and tracked diff whitespace checks passed.
- No production migration, running-app verification, deployment or commit.

## Settlement-admission verification — 2026-10-08

Implemented in `settlement_capacity.rs`, used by every durable commit. No
production callers were added. The checkpoint serialization remains format 1;
the generated inspection contract only adds the `settlement_limit` fault code.

- Final graph suite: **46 passed**. Nine new tests use disposable SQLite owners
  to cover exact event exhaustion, byte exhaustion with maximum-size successes in
  both producer orders, shared and retained consumer admission, cancellation,
  maximum-size failures, oversized evidence, failed commit retry, checked arithmetic,
  dispatch rejection, uncertain recovery, repeated startup and changed byte limits.
- Final fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract
  verification and application TypeScript/Vite build passed. Vite retains its
  existing bundle-size advisory. IPC registration regressions: **2 passed**.
- Full native library suite: **899 passed, 6 failed, 6 ignored**. It reproduces
  the same six activation/model-metadata/migration-default failures listed in the
  [inspection checkpoint](ai-graph-inspection.md#verification-checkpoint--2026-10-08).
  This full run preceded the final unchanged-recovery byte validation correction;
  the final 46-test graph run, Clippy and fast gate cover that correction. The
  application suite is not green and production integration remains gated.
- The existing one-event admission fixture now provides two events: one Begin
  plus its required recovery reservation. It still rejects subsequent ordinary
  work atomically; this is the stricter admission contract, not a relaxed limit test.
- Documentation entry-point links, 13 local links across the three changed graph
  notes, and tracked whitespace checks passed.

These tests verify the bounded contract and selected interleavings, not a formal
proof or unlimited storage guarantee. Retention/compaction, provider evidence
bounds, abstract-model conformance, production migration and live viewer acceptance
remain open. No history was pruned, no workflow/default policy changed, and no
commit or deployment was performed.
