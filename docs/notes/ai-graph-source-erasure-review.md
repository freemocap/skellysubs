# Revision retention policy correction

Date: 2026-10-09. Status: **current user-approved policy**.

The user superseded the earlier selective-erasure proposal:

- Editing an earlier message removes dependent later exchanges from the active conversation.
- Deleted/replaced product ownership cannot authorize provider dispatch or result publication.
- Superseded graph inputs, results and execution history may remain.
- Preserve retained coach history and existing award behavior.
- Physical selective graph-history erasure is deferred and is not a migration prerequisite.

The unused selective-erasure implementation and its checkpoint extensions were
removed. The ordinary native reducer, source ownership checks and existing domain
transaction remain the mechanisms used for workflow conversion. See the
[production checklist](ai-graph-production-integration.md) for verification and
remaining workflow wiring. No running-app acceptance is implied by this decision.
