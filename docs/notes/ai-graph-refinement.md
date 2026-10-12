# Graph transition refinement: shared execution lifecycle

Status: first finite model and conformance explorer implemented, 2026-10-08.
This is a deliberately bounded part of foundation invariant G10, not proof of
the complete runtime or completion of the foundation gate. Verification results
are recorded below separately from the model and future obligations.

## Purpose and ownership

The [foundations](ai-graph-foundations.md) require concrete execution to preserve
the specified transition relation. Before replacing resident maps with storage
access, establish a test oracle that does not call the reducer to compute its
expected answers. This step covers the lifecycle that makes loading one run in
isolation unsafe: two consumers of one shared producer, with independent pause,
cancellation and adoption, plus dispatch, completion and recovery.

The abstract model lives only in `native/src/ai/graph/tests/refinement/model.rs`.
It imports no graph implementation types or helpers. It is a verification oracle,
never a runtime, storage index, UI interpretation or second operation registry.
The sibling test harness translates abstract actions to native events and maps
native records back to abstract observations. Production execution and inspection
continue to use the same native artifact and reducer.

## Finite state and transition relation

Fix two runs, one automatic node in each, equal validated inputs and authority
scope, and exact reuse. Both consumer attempts already exist and share one queued
producer. A second, disabled node and its dependency remain in each artifact but
have no attempts. Initial graph admission and reuse-key construction are outside
this model; the native harness checks that the fixture establishes this state.

Let a state be `s = (p, c[2], paused[2], active[2], capacity)`:

- `p`: queued, running, successful, failed with a cause, or unknown.
- `c[i]`: queued, running, available, adopted, failed with a cause, unknown,
  or cancelled.
- Failure causes: provider failure, abandoned before dispatch, or interrupted
  before dispatch. These distinctions preserve recovery and cancellation evidence.
- `capacity` is zero or one provider slot. There is one producer, so it cannot
  contend with another dispatched producer in this model.

The initial state has queued producer and consumers, both runs active and
unpaused, and one slot. Fourteen actions form the input alphabet: pause/resume,
cancel and adopt for either run; advance with zero/one slot; dispatch; successful
or failed settlement; and recovery.

| Action | Abstract rule |
| --- | --- |
| Pause/resume `i` | Set only that run's pause flag. Pausing does not cancel a request or revoke adoption. |
| Cancel `i` | Revoke that run's authority. Cancel its queued/running/available attempt; retain terminal evidence. Abandon a queued producer only when both runs are inactive, including paused consumers in this decision. |
| Advance | Set capacity. Report queued producer work when an active, unpaused consumer remains. Reporting prepared work is distinct from dispatch authorization. |
| Dispatch | Require queued producer, positive capacity and an active, unpaused consumer. Start the producer once and mark all queued subscribers running, including paused subscribers. |
| Successful settlement | Require running producer. Retain success and make each running consumer available; cancelled consumers remain cancelled. |
| Failed settlement | Require running producer. Retain provider failure for the producer and each running consumer. |
| Adopt `i` | Require that run still active and its attempt available. Adopt only that consumer; adoption exposes its graph output. |
| Recover | Pause both runs. Queued producer/consumers become interrupted failures; running producer/consumers become unknown. Preserve settled, adopted and cancelled evidence. Never dispatch or adopt. |

An unmet precondition rejects the action and leaves state and emitted work
unchanged. Successful no-op actions are permitted; they still append native
events. Repeated recovery can therefore preserve semantic state while changing
the raw engine revision. The durable owner's recovery-write suppression is a
separate contract.

## Conformance algorithm and observations

The harness explores the entire reachable finite abstract state graph with a
breadth-first queue. For every visited state it tries all fourteen actions against
both the independent model and the native engine. There is no random sampling or
depth cutoff. Transition and snapshot-merge mismatches report their shortest
discovered prefix and action.

For each edge, it checks matching acceptance/rejection and:

- On acceptance, the native abstraction equals the model successor. Returned
  work equals the original producer's exact identity, artifact, operation,
  inputs and resource, with the model's expected multiplicity.
- Native revision advances once. On rejection, the complete native snapshot,
  journal and revision remain unchanged.
- Both runs retain the exact original artifact, both nodes and their bindings,
  even when disabled, cancelled, paused, failed or unknown. Disabled nodes have
  no manufactured attempts.
- Native node dispositions match consumer evidence. Each run exposes a graph
  output only after its own adoption; revoking later authority preserves already
  adopted evidence.
- The two attempts continue to point to the single producer. No action in this
  alphabet allocates another attempt or execution.

When different paths reach the same abstract state, their complete native
`RuntimeState` values must be equal. This guards against silently collapsing
differences in fields omitted by the abstraction, including allocator, capacity,
holds and immutable inputs. The state deliberately excludes event journal and
absolute revision. The explorer keeps one representative only after that equality
check. It therefore checks a semantic-state quotient, not unbounded event-history
equivalence. Revision overflow and history budgets need their existing separate
tests and later extended models.

## Boundary and remaining obligations

This finite result does not establish liveness, external exactly-once execution,
all-graph correctness or correctness under arbitrary storage failures. It does
not execute a provider handler. It verifies prepared/dispatched work exposure in
the raw reducer; durable authority and commit-before-invocation remain covered
by their existing transaction tests.

Extend the independent model before claiming coverage of:

1. Begin, on-demand activation, guards, data/control dependencies and fan-in.
2. New subscribers, retained reuse, retries, multiple producers and deterministic
   candidate/producer selection under resource contention.
3. Owner authority changes, atomic publication, commit uncertainty and reload.
4. History/state limits, allocator/revision bounds, historical inspection,
   persistent formats and migration.

For [cold-state loading](ai-graph-state-bounds.md), reuse this action corpus and
observation boundary against the storage-backed interpreter. Add explicit read,
eviction and transaction-failure actions: successful physical reads/evictions
must be abstract stuttering steps; a refused operation must preserve committed
state; an uncertain commit must block further effects until authoritative reload.
That interpreter must share native transition rules rather than implementing the
test model in production. The storage refactor and these additional checks are
not implemented by this checkpoint.

## Verification

- The independent model reached **400 states** and checked **5,600 edges**:
  **3,735 accepted**, **1,865 rejected**. Native behavior matched every edge;
  snapshot equality held at every abstract-state merge. Reviewed counts are
  asserted so accidental reductions in coverage fail the check.
- Isolated graph suite: **79 passed**. Fast gate (including 16 validation-tool
  tests), Clippy (`--lib --tests -- -D warnings`), documentation entry-point links,
  45 local links across eight graph notes and tracked whitespace checks passed.
  Both new Rust files are below 200 lines after formatting.
- Full native library suite: **933 passed, 6 failed, 6 ignored**. The same five
  conversation activation/model-metadata failures and one migration-default
  failure remain: extra automatic reading helpers, missing fixture model metadata,
  and migrated `on_demand` versus fresh `automatic` reading. No assertions were
  weakened. The complete native suite remains non-green.
- This step changes verification and documentation only. It introduces no new
  runtime API, serialized contract, production migration, provider call or UI
  integration. Earlier uncommitted implementation work is preserved. No commit,
  push or deployment was performed.

Prior checkpoint results remain recorded in
[resident state bounds](ai-graph-state-bounds.md). A matching finite model does
not establish that every desired product policy is captured by that model;
review the explicit boundary and transition table when extending it.

The subsequent canonical-state refactor uses `RuntimeState` directly in the
harness instead of capturing a separate snapshot type. The independent model,
action alphabet and expected state/edge counts are unchanged. Its new runtime
verification results are recorded with the state-ownership refactor above.
