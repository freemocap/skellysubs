# Native graph inspection export

Status: implementation contract for the isolated foundation, protocol 1,
2026-10-08. Extends [the foundations](ai-graph-foundations.md) and
[durable owner boundary](ai-graph-durability.md). No production workflow, IPC
command or activity viewer is connected by this change.

The [invocation capture contract](ai-graph-core-semantics.md#invocation-identity-and-evidence-capture)
now keeps typed, bounded response observations separately from output values.
Those observations are native-only adapter assertions, not part of protocol 1.
Their [native persistence](ai-graph-durability.md#execution-evidence-and-report-settlement)
is implemented. Production sensitivity validation and snapshot/stream export remain open;
do not serialize the report into this protocol as an unreviewed extension.

## One structural representation

`Artifact<V>`, `Definition<V>`, `Node<V>`, `Boundary<V>` and `Source<V>` are the
same Rust types used by execution and inspection. Execution uses JSON constants;
inspection substitutes `ConstantDisclosure` through a total structural mapping.
Only constant values change. Ports, bindings, controls, guards, composition
boundaries, implementation IDs and type contracts remain identical. New fields
and enum variants require exhaustive mapping changes in the native owner, not a
viewer catalog. The existing executable/checkpoint serialization is unchanged by
these type parameters.

Attempts, attempt states and reasons similarly use shared parameterized types.
The exporter copies native dispositions, effective activation and reasons; it does
not reevaluate scheduling or infer state from absent records. Explicit paused and
active flags retain run-level facts even when a node is already prepared/running.

`DurableEngine::inspection_snapshot` produces one immutable snapshot containing
protocol, engine UUID, revision, run identity, artifact identity, full structure
and runtime facts. Revision must match the committed checkpoint. A host with an
uncertain commit refuses export until it reloads. Engine, revision and artifact
identity are required when comparing snapshots; an older revision must not replace
a newer one in a future client. A changed artifact is a different structure.

`Executable::inspection_definition` exports the identical artifact without any
run fields, even before an engine/run exists. It does not create a synthetic run
or label dormant definitions as successful. Both entry points use the same
structural projection; changing run selection cannot change that artifact.

`Checkpoint::inspection_definition` uses that same projection on the exact saved
artifact. No registry or executable handlers are needed, even after the original
implementation has been removed. `artifact_ids` lists the retained manifest; decode
checks each artifact's identity with the compiler's unchanged fingerprint algorithm.
This entry point is historical definition access, not permission to resume old
work. The separate [historical reader](ai-graph-history.md) now reconstructs native
runtime facts without handlers and exports bounded attempt pages. Executable
recovery still requires exact bindings. See the [retention contract](ai-graph-retention.md).

All attempt/execution IDs and the revision are decimal strings in this protocol,
avoiding JavaScript integer rounding. Internal checkpoint IDs remain unchanged.
Snapshots do not contain checkpoint checksums, run inputs, result values, scopes
or credentials. The artifact ID still identifies the original executable artifact,
not a newly hashed redacted graph. A disclosed artifact is not executable.

## Disclosure and trust boundary

Graph/operation/type/port identities, shape field names and implementation IDs are
authored public schema. Run IDs must be opaque owner-assigned identities. These
structural fields must never contain learner content, credentials or private host
paths. This is a construction contract for trusted native definitions, not a
sanitizer for arbitrary user-authored schemas. Redacting those identities would
break parity; sensitive values belong in typed inputs or explicit constants.

Every constant, including values in captured subgraph definitions and bindings,
is replaced with an explicit `Content` omission. This includes boolean and numeric
constants; runtime facts already report guard outcomes. Source content is not
copied into the export to let the UI reconstruct eligibility.

Native faults now use one closed `CoreFaultCode` vocabulary in the implementation.
Export preserves recognized codes and known structural paths. Arbitrary adapter
code/path fields receive explicit `Unclassified` omissions, including in reasons;
their original records remain unchanged internally. There is no guess that a
short string or a provider message is safe merely because it is called a code.

This closes disclosure for the current code/path-only core fault model. It does
**not** implement provider metadata retention or license replacing richer provider
errors with these fields. Provider integration remains gated on typed bounded
metadata and the shared diagnostic sensitivity policy, preserving request IDs,
models, finish reasons, timing, retry/rate-limit facts, billing/usage provenance,
validation details and explicit redaction/truncation. Existing provider diagnostics
are not changed or routed through this exporter.

Export limits fail explicitly on excessive attempts or bytes. They never return
half a graph, drop edges, or truncate history without disclosure. Historical pages
retain complete structure and node facts with explicit attempt totals, offsets
and continuation cursors in `InspectionSnapshot<AttemptPage>`.

## Stream ordering: production implementation design

Status: bounded capture, its versioned durable watermark and materialized native
live reads are implemented in the isolated core. **Host delivery, IPC and the UI
are not connected.** The content-free graph protocol remains unchanged; protected
live reads have a separate generated wrapper. The source audit found that `useReplyStream` currently tests text
prefixes to infer whether a terminal snapshot has retained the live text. That
heuristic must disappear with the converted workflow. Cleaned accepted output and
original streamed text can differ without either being incomplete.

There are two independent orders: committed engine revision and provisional
producer sequence. Neither substitutes for the other. A token arriving after a
cancellation is a later producer observation, not permission to undo cancellation.

The following contract uses the existing ontology:

1. A producer stream is identified by engine UUID and execution ID, plus an opaque
   host-session identity for ephemeral delivery. One claimed invocation exists per
   execution in the current semantic profile; do not invent an independently
   scheduled invocation entity. Retry creates another execution. Workspace reset
   or host restart invalidates the old session; identity changes are established by
   an authoritative read, never by accepting an arbitrary event as a new workspace.
2. The native capture owns a checked monotonic sequence and bounded cumulative
   provisional text. Equal text is a no-op. Replacements need not be prefixes:
   source adapters can correct provisional output. Bounds fail explicitly and keep
   the previously accepted content; they do not silently truncate it. Sequence
   exhaustion is an error, not wrapping arithmetic. A capture failure retains the
   last accepted text sequence rather than inventing another text replacement.
   The native callback-lifetime UUID is distinct from the UI delivery session.
   Preview wire counters are decimal
   strings, as with existing graph IDs and revisions.
3. Consumers refer to that producer through the existing native attempt records.
   No stream supplies topology or an operation-kind-to-node lookup. The native
   projection uses current consumer authority/state to decide which preview can be
   presented as live. A cancelled consumer never becomes live because a sibling
   still needs the same producer. Late text can remain protected execution evidence.
4. Persisted preview content has an explicit retained-through sequence. The native
   owner advances that watermark only in the transaction that retains the exact
   capture, including a complete replacement if the adapter corrected text. This
   is protected source content, separate from redacted response metadata. It must
   not be added to the public graph inspection export or diagnostics by default.
5. Final report capture closes the producer callback capability. A final snapshot
   carries the retained capture watermark and durable outcome after settlement
   commits. Consumer cancellation can precede producer closure; these are separate
   facts. `Available` still means computed and unadopted. Only committed adoption
   supplies accepted domain output. The accepted text can differ from retained
   source text after validation/cleanup; no prefix comparison establishes handoff.
6. Native reads assemble committed graph facts with the eligible provisional
   overlay under the same serialized workspace owner. The read must not combine
   an older committed revision with a newer authority interpretation. Transport
   notifications invalidate a read; they do not carry an independent terminal
   state machine. Coalesce high-frequency notifications. A window serializes its
   refreshes and rejects responses from an obsolete selection/session, rendering
   only native-projected state. It never calculates readiness or adoption.
7. Subscribe before the initial read and refresh after subscription/reconnection.
   A bounded refresh while native state reports outstanding execution handles a
   dropped final notification. Retained state is authoritative even if the process
   died without delivering its final event. After restart, only acknowledged
   capture is recoverable; missing tail text is unknown, not an empty successful
   result. Historical inspection never mixes in today's ephemeral streams.

This chooses native materialized reads plus invalidation notifications for the
converted path, instead of reproducing graph/consumer reconciliation in TypeScript.
The client still owns ordinary subscription, refresh and rendering mechanics.
The immutable artifact and its complete nodes/edges are unchanged by every case
above. Selecting a run selects native overlays within that artifact version.

Implementation must test reordered/duplicate invalidations and reads, corrected
text, missing final delivery, separate windows, reset/restart, cancelled shared
consumers, settlement/adoption rollback and acknowledgement loss. A combined
snapshot must fail explicitly on its declared bounds; it cannot hide nodes or
silently discard response information to fit. Extending persisted preview content
requires explicit checkpoint/record format evolution and capacity reservations.
The production workspace migration must include that representation. Capture,
watermark, versioning and native recovery now have tests described in the
[handoff contract](ai-graph-durability.md#provisional-content-handoff). Host
transport/UI cases remain implementation obligations.

## Implemented protected materialized read

`DurableEngine::read_live_inspection` returns `LiveInspection`: the unchanged
`InspectionSnapshot` plus a node-keyed map of `AttemptPreview` values. It reads the
run's actual current-attempt references and the referenced execution records under
the same committed stamp. It never reconstructs a current attempt by sorting IDs
or matching operation names. It does not write an event, start work or adopt output.
Dormant nodes and all artifact connections remain in the exact shared projection.

Each preview carries native attempt/execution IDs, a callback-session identity,
decimal-string sequence, source text and a disclosed capture fault. The native
read also supplies `retained_sequence`, `uncommitted`, `live` and `complete`:

- `live` means this current consumer is eligible to show running source text. It
  requires a running, noncancelled active consumer and a dispatched producer with
  no outcome, unknown flag or complete report. Pausing prevents new work but does
  not revoke an already-running consumer. This is not a claim that network bytes
  are arriving or that the provider connection is healthy.
- A live consumer can use its latest native capture. An older same-session capture
  cannot replace a newer retained replacement. Equal-sequence conflicting text,
  changed callback session or changes after a latched failure are errors.
- Cancelled, failed, available, adopted and recovered-unknown consumers use retained
  capture only. They never receive uncommitted tail text from a shared producer.
  `complete` means final report retention, independently of adoption or success.
- `uncommitted` compares the whole selected capture with retained capture, including
  its failure. A newly latched failure can therefore be uncommitted even at the
  retained text sequence. No text-prefix comparison determines handoff.
- A retried node uses its new current attempt; an old execution's cached text cannot
  become its live preview. Old source remains available through protected evidence
  reads and history, rather than being attached to the new attempt.

The host supplies a bounded batch of native `EvidenceSnapshot` values. All entries
must belong to this engine and have unique execution IDs. Entries for other current
or historical executions in the same host cache are not exported for the selected
run. Selected producer artifact/operation identities must match. Response metadata
in those inputs is not exported; capture faults use the existing disclosure policy.
The wrapper deliberately contains source text and is **not a diagnostic export**.
An authorized content-inspection command must opt into it; the public graph snapshot
and definition exporters remain content-free. Rust generates the wrapper's client
types; no IPC command is registered by this foundation change.

`LiveReadLimits` bounds capture count, encoded input bytes and the complete encoded
response. The existing attempt export bound still applies. One native record-read
budget covers graph projection and the preview join, including cold reads. Corrupt,
missing, stale or over-budget records fail the whole read; no partial graph or
resident fallback is returned. These bounds are not an aggregate RSS guarantee.

The host must serialize workspace/reset authority with native capture collection
and this synchronous read. The core's engine identity is not a UI subscription
epoch. Host-session invalidation, selection changes, refresh coalescing, reconnects
and notification-loss recovery remain host/IPC/UI integration work. Arbitrary
external source/access changes must already have been reflected through the owner;
this read does not invent a second domain authorization check or scheduler.

## Generated client contract and verification

`npm run contracts` generates `ui/src/generated/graph-contracts.ts` directly from
these Rust types; `npm run contracts:check` verifies it. The generic value/failure/ID
arguments identify the export specialization without duplicating the ontology in
TypeScript. `InspectionSnapshot` and `DefinitionSnapshot` are the transport types;
generic defaults describe the internal model and are not additional IPC endpoints.
No operation-specific frontend registrations are introduced.

Native regression tests compare the entire nested exported artifact with the
executable artifact after independently substituting only constant values. Tests
also check dormant/disabled nodes, authoritative reasons, unchanged raw data,
large IDs, classified/unclassified failure disclosure, limits and uncertain-host
rejection. Renderer visibility and workflow integration remain subsequent gates.

## Verification checkpoint — 2026-10-08

On branch `graphs`, with changes left uncommitted:

- Final isolated graph suite: 37 passed, including definition-only/run artifact
  parity and disclosure tests.
- Fast gate, native Clippy, application TypeScript/Vite build and generated
  contract check passed. Vite retains its bundle-size advisory.
- IPC registration and settings contract UI regressions: 40 passed. The Windows
  sandbox initially prevented esbuild from reading an ancestor directory; the
  same tests passed with approved elevated execution.
- Full native library suite: 890 passed, 6 failed, 6 ignored. This run preceded
  the final definition-only export and raw-ID TypeScript annotations; the final
  37-test graph suite, Clippy and application build cover those final changes.
- Documentation entry-point links and whitespace checks passed.

The six native failures reproduce the prior checkpoint: five conversation
activation tests (`defaults_schedule_reply_coaching_and_assessment_without_backfill`,
`late_assessment_shares_pending_work_preserves_source_and_credits_once`,
`late_helpers_bind_current_access_and_reject_unavailable_sources`,
`opening_failed_help_is_a_read_and_retry_is_explicit`, and
`requesting_reading_before_reply_does_not_schedule_other_helpers`) and the storage
migration test
`baseline_upgrade_preserves_history_settings_and_has_a_recovery_copy`.
They concern automatic reading scheduling, current model metadata and migrated
versus fresh defaults. No policy assertions were weakened to obtain a pass.
The full application suite is not green; workflow reconstruction remains gated.

Contract generation also exposed a committed source/generated mismatch:
native `DEFAULT_EXECUTION.reading` is `automatic`, while the previous generated
TypeScript said `on_demand`. Regeneration now reflects `automatic`. This can affect
UI defaults and is distinct from the graph inspection implementation; the native
policy was not changed here. The settings contract regressions passed with this
alignment. Migration/default-policy consistency remains unresolved as above.

No live provider calls, production graph ownership, workspace migration, IPC
wiring or activity renderer were exercised or introduced by this checkpoint.
