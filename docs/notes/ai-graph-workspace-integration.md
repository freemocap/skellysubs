# Graph workspace integration

Updated 2026-10-10. Implemented application storage and ownership contract.
The [production checklist](ai-graph-production-integration.md) records verification
and acceptance. Formal semantics remain in the [foundations](ai-graph-foundations.md),
[durability contract](ai-graph-durability.md) and [retention contract](ai-graph-retention.md).

## Ownership

The workspace owns locking, migration and the database transaction. Conversations
own turns, messages, source authority and result publication. Learning owns accepted
observations, disclosure and stable credit identities. The executable graph owns
readiness, attempts, producing executions, sharing, uncertainty and adoption.

A storage partition belongs to a conversation or workspace and an exact executable
catalog. Catalog identity is the digest of the sorted artifact identities bound to
the engine. Reuse follows operation policy, captured inputs and scope within that
partition. Source deletion revokes product ownership; retained graph history does
not authorize another publication. Conversation deletion cascades its partition.
Workspace-owned runs retain their explicitly captured domain owner and acceptance
checks. A catalog is not a rollover counter.

## Records

| Record | Responsibility |
| --- | --- |
| `graph_engines`, `workspace_graph_engines` | Owner/catalog identity, current stamp and checkpoint. |
| `graph_records`, `workspace_graph_records` | Canonical native keys, envelopes and committed bytes. |
| `graph_archives`, `workspace_graph_archives` | Immutable checkpoint history and verified chains. |
| `graph_conversation_runs`, `workspace_graph_runs` | Run/artifact association and captured domain owner. |
| `turn_execution_owners` | Turn channel and graph engine/run/artifact. |
| `graph_transport_identities` | Immutable wire identities for dispatched producers. |
| `conversation_graph_effects`, accepted publications | Stable reply effect, reserved source and adopted provenance. |
| `conversation_graph_assessments`, disclosures | Exact source, attempt, producing execution and assessment disclosure. |
| Graph audio receipts, playback requests and delivery | Durable execution evidence, requested playback ownership and bounded audio delivery. |

Protected reads use a transaction and native stamp/integrity checks. The adapter
consumes one owner transaction. Publication callbacks borrow it, recheck current
source/access authority and stage domain effects before commit. Storage uncertainty
is surfaced explicitly; recovery never silently resubmits a provider request.

## Product projections

Message status, helper controls and assessment history consume source-bound graph
state. Learner credit projects accepted domain observations and reward ledgers;
live evidence views add the graph's assessment status. Transaction-time credit
calculation never reads an engine while committing that engine's adoption.

Activity displays the executable artifact, native dispositions and revision-bound
attempts. The viewer supplies layout and presentation. Selecting history performs
reads. Deliberate inspection credit is a separate idempotent command after evidence
is displayed, with language and conversation resolved from native ownership.

## Persistence compatibility

The complete workspace migration chain starts at format 45. Released migration
steps, frozen schemas and checkpoint encodings remain unchanged. Format 71 retires
execution-cache tables while preserving current graph records, domain results,
learner evidence, earned awards and shared blobs. Retained relational history tables
and discriminator values are persistence contracts; no active command dispatches
work through them. Removing retained information requires a separate consecutive
migration and ownership review. See [workspace migrations](workspace-migrations.md).

## Verification

Application command tests cover admission, independent branch completion, requested
helpers, retry, cancellation, restart, publication and revisions. Storage tests cover
all supported starting formats, rollback, repeated startup and fresh-schema
equivalence. The production checklist records final gate outcomes and the limits of
running-app verification.
