# Native historical graph inspection

Status: native read contract with coach IPC/viewer integration, updated 2026-10-09. Extends the
[retention contract](ai-graph-retention.md) and [inspection protocol](ai-graph-inspection.md).
Historical reads, bounded attempt exports and run-state history are implemented.
The production graph-store adapter and coach activity surface now use these reads.
Cold-state paging and whole-application conversion remain incomplete.

## Run timeline integration — 2026-10-09

`Checkpoint::run_history` replays the same native transitions and exports a
chronological page of state changes for one run. A change is a difference in node
dispositions, reasons, activation policy, latest attempts, active/pause state or single-step permission
and eligibility. Streaming evidence alone does not create a graph-state frame.
Each frame is a complete `InspectionSnapshot`, including nodes that never ran and
the exact retained artifact. This is a presentation sampling rule over native
facts, not another scheduler or an alternative event log.

Pages select revisions strictly before an optional canonical decimal cut. The
response's `before` cursor names the oldest returned frame when earlier frames
remain. Counts and serialized bytes bound each page; older frames remain available
through pagination. An oversized single frame fails explicitly. The reader audits
the entire retained chain and uses the existing history/state limits; this does
not make historical reads constant-cost or disk-paged. No handler is invoked and
no recovery or publication is performed.

The workspace command binds conversation, run and persisted engine/catalog before
reading. Missing executable implementations still permit retained inspection;
execution controls are unavailable for those runs. The UI consumes Rust-generated
contracts and switches structure, live state and recorded frames in the same
renderer. A historical selection receives neither current response text nor live
controls. Node/edge geometry continues to derive only from the artifact.

Verification covers frame equality with native cuts, pagination across compaction,
redaction, read-only storage, invalid bounds/cursors, multi-catalog restart and UI
selection/identity checks. Rendered desktop/phone/pop-out acceptance remains open.

## Shared semantics and separate capabilities

The internal `Plan` contains the exact artifact, its identity and validated
topological order. `Executable` adds the compiled handlers and the source used for
composition. Live compilation and recorded-artifact loading use the same structural
validator. The engine reducer uses `Plan`; invocation additionally requires an
executable binding. A missing binding rejects claim before dispatch or mutation.
Historical inspection neither registers dummy handlers nor resolves a current
operation catalog. It preserves the recorded expanded structure, including captured
composition definitions. Source expansion remains the compiler's responsibility.

`HistoryStore` grants archive reads only. `CommitStore` extends it with the owner
transaction capability. `Checkpoint::historical_inspection` accepts only the read
capability and returns `HistoricalInspection`, whose public API can export pages
but cannot apply events, claim work, adopt results, recover or commit.

For retained events `E` and a requested logical revision `k`, the selected state is:

```text
state(k) = fold(nativeTransition, initialState(exactArtifacts), E[0..k])
page(k, run) = nativeInspection(state(k), run) with a bounded attempt slice
```

Checkpoint formats 1, 2 and 3 use the current version-1 transition semantics;
formats 2 and 3 change storage representation only. Unsupported formats fail explicitly.
A future transition change must preserve the reader for each retained semantic
version or introduce an explicit migration; it cannot silently reinterpret history.

The reader verifies the entire bounded archive chain, including snapshots and events
after `k`, before exposing the selected state. Snapshot comparisons use native replay,
as in durable recovery. Historical reads never synthesize a Recover event: a recorded
Running attempt remains Running at that cut. Storage corruption is not a reason to
invent Unknown or skip to a different cut. Executable recovery remains a different
capability and still requires exact compatible implementations.

The same trusted-schema and content-handling boundary as ordinary inspection applies.
Artifact fingerprints detect changes but do not authenticate imported workspaces.

## Page contract

`InspectionSnapshot<A>` shares all structure, node dispositions, activation, flags
and reasons between live and historical inspection. Its existing default `A` is the
full node-to-attempt map, preserving the live wire shape. Historical pages specialize
`A` as `AttemptPage`; the generated TypeScript uses the same generic Rust type.

Each page contains the entire disclosed artifact and current node facts at the
selected cut. Nodes without attempts remain present. Only attempt records are paged:

- Records are ordered by ascending native attempt ID across the selected run.
  Each record names its node and carries the shared native attempt type with the
  same fault disclosure and decimal-string identity projection used by live export.
- `total` counts every attempt for that run at the cut, including failed, cancelled
  and superseded attempts. `offset` counts preceding records. Both are decimal strings.
- `next` is a cursor when records remain; `null` explicitly marks the final page.
  A run with no attempts returns total/offset zero, an empty record list and no next
  cursor, while still including its complete graph and dispositions.
- The protocol-1 cursor binds engine, artifact, run, absolute revision and the last
  native attempt ID. Protocol 1 fixes the ordering. Noncanonical numeric strings,
  overflow, missing IDs or mismatched identities/cuts are errors, never a restart.
- Byte and attempt limits are explicit. A zero page capacity or an oversized page
  fails intact. The caller can retry with an appropriate budget; the reader does not
  silently omit structure, faults or records to make a response fit.

Reasons may reference the latest attempt outside the selected page. This is deliberate:
node facts describe the complete run at the cut, independently of the attempt slice.
The viewer must not replace those facts with deductions from the displayed rows.

The reader is frozen after validation. Later appends do not change its pages. A
caller can reopen the same logical cut from a later compacted checkpoint and reuse
the cursor. The cursor does not bind to the storage checksum or segment layout,
which change under representation-only compaction. Missing retained history fails
explicitly. No deletion policy is added by this contract.

## Resource scope and next boundary

The active checkpoint is bounded by its decode limits. `HistoryLimits` additionally
bounds archived bytes, events and segments; the store must check byte length before
allocating an archive body. Exports have independent byte/attempt limits.

`HistoricalLimits.state` now enforces [resident record ceilings](ai-graph-state-bounds.md)
during the native replay, separately from `HistoricalLimits.history` archive limits.
These bound accepted record counts, input and output, not exact peak memory. Opening
a reader still audits all retained events, holds native state for all runs/attempts/executions,
and captures the selected state. Pages use a derived per-run sorted attempt-ID index,
validate the cursor against its owned row, and collect only the requested slice.
After locating the run index, counts are constant-time and cursor offsets use binary
search. Each selected row is read from the primary ordered record map. The index
is rebuilt from those rows on decode and is not separately persisted. Pages neither
scan unrelated attempts nor assemble full history merely to obtain node facts.
This is not disk paging or a peak-memory bound; full graph projection still has its
own cost on every page.

Cold-state handling must next preserve late demand, retries, cross-run producer
sharing, exact result reuse and independent adoption authority. An idle run cannot
be discarded as finished. A stored working set needs explicit native loading and
transactional ownership before records can leave resident state. Do not solve this
with frontend caching, inferred missing-node states or history deletion. Independent
abstract-model conformance remains a separate foundation gate.

## Verification

- Final isolated graph suite: **70 passed**. Eight new tests cover every logical
  cut of a retry/sharing trace, native projection parity, complete dormant topology,
  dropped handlers, unchanged evidence and disclosure, cursor continuity across
  compaction/appends, empty pages, nested composition, cursor/output limits,
  corrupted snapshots after the selected cut, missing archives, malformed recorded
  structure, refusal to invoke without handlers and all archive budget dimensions.
- Fast gate, Clippy (`--lib --tests -- -D warnings`), generated-contract verification,
  application TypeScript/Vite build and IPC registration regressions (**2 tests**)
  passed. The build retains Vite's existing bundle-size advisory. UI tooling ran
  with approved elevated execution for Windows ancestor-directory access.
- Documentation entry-point checks, 32 local graph-note links and tracked whitespace
  checks passed. All graph Rust source/test files remain below 500 lines.
- Saved checkpoint formats and artifact fingerprint encoding are unchanged. The live
  inspection wire shape retains its default attempt map; the generated generic
  specialization adds historical pages. No workflow policy, production schema,
  provider adapter, native command or renderer integration was changed.
- No running-app or provider verification, commit, push or deployment was performed.
  This checkpoint's changes remain uncommitted on `graphs`.
- Full native library suite on the final source: **924 passed, 6 failed, 6 ignored**.
  The same five activation/model-metadata cases and one migration-default case
  listed in the inspection contract reproduce their prior failures. They still
  concern additional automatically scheduled reading helpers, missing fixture model
  metadata and migrated `on_demand` versus fresh `automatic` reading defaults.
  No policy assertion or migration was weakened. The full native suite is not green.

The subsequent [independent attempt-record refactor](ai-graph-state-bounds.md)
adds format 3 and changes page iteration as described above. The original protocol,
cursor identity, complete topology and redaction rules remain unchanged.
