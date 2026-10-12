# Executable AI graphs: architecture and delivery plan

Updated 2026-10-10. The [foundations](ai-graph-foundations.md) own the formal
ontology and invariants. The [production integration note](ai-graph-production-integration.md)
owns the current workflow inventory, sequential checklist and verification evidence.

## Architectural order

1. Specify typed graph boundaries, operation contracts, explicit bindings,
   composition, activation and distinct execution/adoption identities.
2. Define transitions, authority, cancellation, retries, sharing, evidence,
   publication transactions, recovery and authoritative inspection.
3. Verify the shared core with domain-independent operations, guarded branches,
   joins, explicit demands, concurrent consumers and interrupted execution.
4. Compose product workflows from reusable operations; connect commands,
   provider capabilities, domain validation and atomic result publication.
5. Render the executable artifact with run/attempt timeline overlays and
   authoritative interrogation. Preserve each node and edge independently of
   whether it executes.

These are architectural constraints. Product bugs supply regression cases and
must not define the graph ontology. Changes to the ontology require explicit
specification review and invariant tests.

## Current delivery sequence

Functional conversion has user acceptance. Activity inspection is connected.
Source cleanup is in progress, followed by a measured efficiency pass and final
application acceptance. The production note's checklist is executed linearly.

Efficiency work measures provider overlap, local dispatch, store-lock wait time,
repeated history reads, snapshot size, rendering and time to visible results.
Use observations to choose bounded changes. Ready local work must not wait for
provider capacity. A read-only product interaction must perform only work needed
to obtain and present its requested information.

## Verification obligations

| Scenario | Required observation |
| --- | --- |
| Automatic, on-demand, disabled and skipped nodes | Artifact structure is unchanged; execution reasons remain explicit. |
| Shared work and closing consumers | Each consumer has distinct authority; provider execution is counted once. |
| Malformed output and partial metadata | Failure is visible and useful redacted evidence survives. |
| Unicode source and repair | Exact source identity and accepted spans survive validation and adoption. |
| Revisions, cancellation and late completion | Replaced or deleted sources cannot acquire late results. |
| Speech cache hit, eviction and restart | Receipts remain inspectable; synthesis requires a request. |
| Provider saturation | Independent local work proceeds; network requests remain bounded. |
| Retry and restart | Unknown outcomes stay explicit; no automatic provider replay. |
| Adoption and credit | Result publication and stable domain effects commit atomically. |
| Timeline and layout changes | Recorded identities, complete structure and selection remain consistent. |

At coherent milestones run the fast gate and affected regressions. Before final
acceptance run native library tests, Clippy with warnings denied, binary checks,
generated-contract checks, UI tests/build and applicable documentation checks.
Report source implementation, automated results and running-app acceptance
separately. Keep changes uncommitted unless explicitly instructed otherwise.
