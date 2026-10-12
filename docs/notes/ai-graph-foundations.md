# AI graph foundations and architectural contract

Date: 2026-10-08. Owner: native AI execution architecture.

## Status and authority

**Agreed constraints:** architecture first; precise shared ontology; reusable
operations and graph composition; separation of structure from execution;
one executable source of truth; a short deterministic path from that source to
visualization with no independently maintained semantic model. These constraints
govern subsequent design and implementation work.

**Implementation status:** the native core, application workflow hosts and
artifact-derived activity viewer are connected. The user accepted functional
workflow conversion. Source cleanup, measured efficiency work and final application
acceptance remain in progress. The [core profile](ai-graph-core-semantics.md)
defines the implemented semantic scope; the
[production integration note](ai-graph-production-integration.md) records current
verification and outstanding work. These tests establish their documented cases
and bounds, not a universal proof of the specification.

The graph foundation must be designed and verified independently of a particular
workflow or bug. Existing workflows supply requirements and migration tests;
they do not define the ontology. Workflow rebuilding follows foundation verification.
No temporary product-specific path may become an implicit exception to the model.

## Intellectual basis and limits

Dataflow literature distinguishes an actor's computation from its firings and
connections [@leeParks1995Dataflow]. State-machine reasoning gives us initial
states, permitted transitions, invariants and refinement between abstract and
concrete behavior [@lamport2008StateMachines]. These are conceptual foundations,
not an endorsement of a particular library or a claim of formal proof.

SkellySpeak's proposed model is a finite typed dependency/dataflow DAG with
effectful operations. It is not a Kahn stream-process network: external services
can fail, return nondeterministic values and have irreversible effects. We do not
inherit determinism or liveness theorems from a different model of computation.
The deterministic requirement for inspection concerns representation of recorded
facts, not deterministic AI responses or a unique concurrent execution order.

## Static ontology

A directed graph is vertices and edges. The executable model adds typed ports,
bindings, policies and an interpretation. Use the following distinct identities.

| Term | Definition |
| --- | --- |
| Type contract | Versioned set of valid values plus validation rules, including semantic/source constraints where required. |
| Port | Named input or output with a type contract and cardinality. |
| Operation definition | Versioned reusable computation with input/output ports, implementation binding, declared effects and execution policies. |
| Node | A named occurrence of an operation in a graph; identity is independent of operation type. |
| Data edge | Explicit directed connection from a value-producing port to a consuming port. |
| Control edge | Explicit prerequisite that orders actions without asserting value transfer. |
| Graph definition | Versioned composition of nodes, graph boundary ports, bindings, control edges and activation rules. |
| Executable graph artifact | Validated resolved graph, bound to compatible operation implementations; consumed by execution and exported for inspection. |
| Subgraph | A graph reused through its declared boundary ports with scoped internal identities and preserved internal traceability. |

Let `T` be type contracts and `Val(t)` the values satisfying contract `t`.
An operation `o` has input ports `I(o)`, output ports `O(o)` and a port type map
`tau`. Its effectful implementation is interpreted by the runtime; it need not be
a pure function. Output validation remains mandatory for external computation.

Define a graph as `G = (id, version, V, op, I_G, O_G, B, C, A)`:

- `V`: finite node identities; `op(v)` resolves a versioned operation definition.
- `I_G`, `O_G`: typed public input/output ports of the graph.
- `B`: explicit data bindings from graph inputs, node outputs or typed constants
  to node inputs and graph outputs.
- `C`: explicit control prerequisites between nodes.
- `A`: declared activation policy per node, including dependencies of conditions.

For each binding `s -> d`, require `Val(tau(s))` to be a subset of
`Val(tau(d))` under a declared, checkable compatibility relation. Initially use
exact contract identity unless an explicit compatibility rule has been reviewed.
Do not attempt arbitrary theorem proving about schemas. Conversions are named,
validated operations; no implicit coercion or name-based source selection.

Type compatibility permits a connection; it does not create one. An input has
exactly one producer unless an explicit collection/merge/select operation defines
multiple-source behavior. Fan-out is allowed. Optional ports explicitly represent
absence; absence is not failure, an empty value, or permission to ignore a missing
required input. Graph boundary ports must be validated like node ports.

All data, control and guard prerequisites form an acyclic dependency relation.
Guards may read only declared inputs/configuration; hidden database reads cannot
silently add dependencies. Conditional outputs require explicit optional/branch
contracts and joins that define what happens when a branch does not run.

Composition substitutes a subgraph through compatible boundary ports, with
qualified internal IDs. Expansion preserves bindings, effects, activation and
runtime trace mapping. A collapsed visual subgraph represents that composition;
it is not a different executable graph. General cycles and unbounded dynamic
topology require a separate semantic extension, not an ad hoc scheduler escape.

## Runtime ontology

| Term | Definition |
| --- | --- |
| Run | One graph instantiation with immutable graph reference, bound inputs, captured policy/configuration and domain owner. |
| Node instance | `(runId, nodeId)`, present conceptually for every declared node even if never activated. |
| Demand | Explicit authorization to activate on-demand work; independent of input readiness and inspection. |
| Attempt | One authorized execution effort for a node instance, with a new identity on retry and retained outcome. |
| Shared execution | A producer that may serve several eligible consumers under a declared equivalence/reuse policy. |
| Provider invocation | One external request, including an individually recorded bounded retry if policy allows it. Not synonymous with a node attempt. |
| Result | Immutable validated output value with type/version, source binding and provenance. |
| Adoption | An authorized owner's transactional acceptance of a result into domain state. Separate from successful computation. |

A node attempt may reuse a retained result or subscribe to a shared execution
without a new provider invocation. Every producer/consumer/result association must
be inspectable. Usage belongs to the actual producing execution/invocations;
consumer adoption must not duplicate charges or domain credit.

Result repair creates a new validated value with explicit lineage. It does not
mutate a previous result or silently overwrite accepted domain records. Cached
payload eviction is distinct from deleting durable provenance or accepted results.
Fresh generation, exact reuse and pending sharing are explicit operation policies,
not global assumptions. Source equality includes the contract's full semantic
scope; equal text alone does not establish interchangeability.

**Revision policy (user correction, 2026-10-09):** message edits remove dependent
exchanges from the active conversation and revoke their product publication
authority. Superseded native inputs/results/history may remain. Physical selective
graph erasure is not required for workflow conversion. Whole-conversation and
workspace deletion retain their existing ownership behavior. See the
[decision correction](ai-graph-source-erasure-review.md).

## Execution semantics

Specify a transition system `M = (S, S0, E, Next)` where `S` includes run/node
state, value availability, demands, authority, attempts and execution associations;
`S0` is valid initialization; `E` contains explicit commands and runtime events;
`Next` is the permitted state-event-state relation. Logical event order and
causality must be available without relying solely on wall-clock timestamps.
This does not require full event-sourced persistence.

For a node instance `v` in state `s`, distinguish:

```text
activated(v,s) = automatic(v) OR explicitlyDemanded(v,s)
ready(v,s)     = requiredInputsValid(v,s) AND controlPrerequisitesMet(v,s)
eligible(v,s)  = activated(v,s) AND guardTrue(v,s) AND ready(v,s)
                AND authorityValid(v,s) AND newAttemptAllowed(v,s)
dispatchable  = eligible AND notPaused AND applicableResourceAdmission
```

Effective automatic policy is captured for the run. Unknown guards, absent
inputs, disabled policy, false guards, pause, resource holds and invalid authority
have different recorded reasons. A previous success or active attempt is not
eligible for another attempt merely because its inputs remain valid. Retrying
requires the declared retry policy and does not rewrite graph topology.

The full transition table is a prerequisite to core implementation: it must cover
demand, input publication, admission, dispatch, provisional output, validation,
settlement, adoption, failure, cancellation, invalidation, retry and restart.
For every transition specify preconditions, atomic writes, authority checks,
emitted facts and recovery behavior. No generic `unknown -> ready` fallback.

Independent siblings may complete in any order. Downstream work consumes only
outputs accepted under the producer's declared publication boundary. Provisional
streaming output does not satisfy a completed-value port. Incremental consumption
requires a separately specified stream port/protocol.

Separate local work, cache lookup and pending subscription from acquiring
a provider permit. Consumer cancellation stops its adoption authority; it does not
silently cancel another consumer or erase a dispatched producer's accounting.
Crash recovery records unknown outcomes where evidence is missing. Exactly-once
external execution cannot be assumed; enforce transactional/idempotent local
adoption and preserve uncertain remote outcomes without speculative replay.

**Application scheduling policy (agreed 2026-10-10):** provider-call admission
limits apply to provider calls, not local native work or UI publication. Active
application hosts impose no local slot quota or network-sized dispatch batch.
Ready local work proceeds subject to its actual dependencies and controls;
independent results publish without waiting for sibling completion. Cooperative
executor yielding and atomic database transitions preserve responsiveness and
correctness without adding a timed delay or an artificial concurrency ceiling.
Data-size validation and retained-record storage bounds are separate concerns.

Liveness claims require explicit assumptions: fair scheduling, available capacity,
valid authority and eventual external response. No UI or scheduler may label work
successful merely to make a graph appear complete.

## Invariants and required evidence

These identifiers should appear in architecture review and relevant tests.

| ID | Invariant | Verification obligation |
| --- | --- | --- |
| G1 | One semantic definition drives execution and inspection. | Shared artifact identity; reject incompatible/missing implementation bindings. |
| G2 | Structure is independent of a run's activity. | Node/edge identity equality across policy and execution-state combinations. |
| G3 | Every required input has a valid explicit source. | Port, type, cardinality, cycle and guard-dependency validation; no hidden source lookup. |
| G4 | Reuse of an operation does not merge distinct node identities. | Two nodes using one implementation remain separately bound and inspectable. |
| G5 | Demand, readiness and admission are distinct. | Transition tests prevent unsolicited on-demand work and premature dispatch. |
| G6 | Adoption requires current owner/source authority. | Cancellation, replacement, access change and reset interleaving tests. |
| G7 | Sharing preserves independent consumers and single accounting. | Concurrent subscription, cancellation, retry and billing/credit tests. |
| G8 | Results, attempts and historical graph identity are traceable. | Version/migration/restart tests; no fabricated history or overwritten attempts. |
| G9 | Failure preserves useful bounded redacted evidence. | Validation and publication failures retain metadata without credentials/content leakage. |
| G10 | Concrete runtime refines the specified transition system. | Every tested concrete transition maps to a permitted abstract transition or an internal step with unchanged abstract state. |
| G11 | Visualization preserves semantic identity and facts. | Projection parity and real renderer visibility tests; display changes cannot change execution. |

Core examples must be domain independent: a chain, fan-out/fan-in, two instances
of one operation, a guarded branch/join, on-demand activation, failure and retry,
two consumers sharing a producer, cancellation races and interrupted recovery.
Use property-based/state-machine tests and bounded interleaving exploration where
appropriate. Passing examples is evidence, not a proof of all behaviors. Record
what was verified and the assumptions of any model checking or proof separately.

## Visualization: a deterministic projection, not a second implementation

Required path:

```text
native operation definitions + graph composition
    -> validated executable graph artifact
         -> runtime interprets that artifact
         -> generated inspection contract + authoritative runtime facts
              -> generic UI layout/style/selection -> renderer
```

Let `P(G,R)` be the native inspection projection of artifact `G` and a compatible
run snapshot `R` (or no run). Let `U` be presentation state. Rendering is
`Render(Layout(P(G,R), U))`. Changing `U` cannot change `G`, `R`, eligibility or
provider activity. The projection must be deterministic for the same versioned
inputs. Transport serialization and redaction may omit protected values explicitly;
they must not invent semantics or silently discard node/edge identities.

The executable artifact is resolved once through one path, not separately built
for the viewer. A generic serializer or generated DTO is acceptable; a second
catalog, manually synchronized topology, operation-name switch reconstructing
dependencies, or UI inference of activation is not. Adding an operation must not
require adding it to an independent visualization registry to make it appear.
Optional specialized value renderers may improve inspection; generic inspection
must still work and expose the same identities/contracts without them.

Without explicit user filtering/grouping, projected nodes and edges equal those
of the selected artifact, including unrequested/disabled nodes. User filtering,
subgraph collapse and viewport clipping are presentation operations and must be
disclosed; they do not redefine structure. Run selection changes overlays, not
topology within one graph version. Historical missing evidence is marked unknown.

The thinness criterion is semantic maintenance, not an arbitrary line count or
ban on adapters: changing an executable binding must update the inspected binding
through the same artifact without a second semantic edit. Test that property.
Source links, artifact version and implementation identity should make the path
from any inspected node to working code explicit. Renderer measurement/edge
visibility still needs real UI tests even when projection parity passes.

## Persistence, evolution and implementation discipline

Bind new runs to immutable graph and operation contract identities. Version
compatibility includes implementation binding; do not silently resume an old graph
with incompatible new code. Persist sufficient definition evidence for inspection;
a hash alone cannot reconstruct a removed definition. Historical unknowns stay
unknown. Follow the complete workspace migration chain and preserve existing
evidence, ownership and receipts.

Architecture changes update this specification before dependent workflow changes.
Review each new concept against the ontology; do not reuse one ID or state field
for unrelated entities. Exceptions require a documented semantic extension and
tests, not a product-name conditional. Track draft, agreed, implemented and verified
status separately. A partial implementation must list unmet invariants and must not
be presented as the completed architecture.

Future contributors must consult this contract and its staged plan before changing
graph composition, execution, identity, result sharing or visualization semantics.
Keep runtime and presentation derived from executable artifacts. Workspace upgrades
retain the supported migration chain and frozen persistence encodings.

## Verification scope

The core profile, durability, inspection, retention, history, state-bounds and
record-storage contracts specify separate obligations. Their tests exercise typed
composition, transitions, sharing, authority, settlement, recovery, bounded reads,
retained evidence and explicit payload eviction. The refinement model covers a
finite lifecycle; its reachable-state checks are bounded evidence, not a proof of
all scheduling and storage interleavings.

Application hosts bind current source authority and transactional domain adoption.
Generated inspection contracts preserve executable identities through layout and
selection. Provider-metadata classification, provisional content and publication
are checked through actual application command paths with controlled providers.

The [production integration note](ai-graph-production-integration.md) owns the
workflow inventory, verification results and remaining delivery checklist. Current
performance requires measurement of lock contention, repeated reads, hydration and
rendering, followed by regression checks and running-application acceptance.
