# Installation history retention and administration

Status: implemented and locally verified; not deployed or verified against the hosted database.

## Agreed behavior and ownership

Installation records are server reporting metadata, not trusted-device grants or
sessions. Fresh local application data can produce another identity. The former
100-record rejection ran during `/v1/me` and prevented completing sign-in.
The user authorized replacing that failure with automatic eviction and exposing
installation cleanup in administration.

Retain at most 100 installations per account. A new installation atomically
replaces the least recently seen record at capacity. Equal timestamps use the
installation ID as a deterministic tie-breaker; undated legacy metadata is retired
first. Existing check-ins preserve first-seen time and refresh last-seen time and
the existing 90-day inactivity expiry. Firestore TTL remains background cleanup,
not admission logic. Its live configuration was not inspected in this work.

Deletion intentionally retires platform/version and first/last-seen history. It
does not remove accounts, learner evidence, usage, allowances, billing records,
or session credentials. A future check-in recreates the metadata. No workspace
schema or persisted contract migration is needed: record fields are unchanged.

## Implemented administration

Account details show retained count, retention policy and newest check-ins first.
Remove selected and Clear installation history use the existing owner-only,
same-origin change endpoint, confirmation dialog, account revision, operation UUID
and transactional audit receipt. Clear removes records present when the change
executes. Replayed operations cannot delete records recreated after that change.
Audit records retain removal counts. Session revocation remains separate.
The detail panel refreshes after changes so subsequent actions use its current
revision. The busy handler preserves disabled controls, including empty selections.

## Verification

- Original focused test failed with HTTP 409 on the 101st registration before the fix.
- Server suite: 601 passed, 8 skipped. The skipped checks require an explicitly
  enabled Firestore emulator, including the added concurrent eviction check.
- Regression coverage includes last-seen eviction, refreshed installations,
  legacy timestamps, concurrency in the local store, rollback of eviction/deletion,
  audit replay, account isolation, unchanged session/allowance/history, and HTTP
  `/v1/me` success at capacity and after admin cleanup.
- Admin UI: 10 tests passed, including review-before-write and both action payloads.
- `npm run check:fast`, `npm run build`, standalone admin TypeScript check and
  `node ui/tools/admin-build.ts --check` passed. Build retains the existing large
  bundle warning. Generated admin CSS also incorporates previously stale shared
  tokens from its current source; no CSS source was changed here.
- The first full server run had Windows temporary-folder access errors; rerunning
  with a fresh workspace temporary directory passed. That directory was removed.

The pre-existing large quota module remains in place to keep this behavior change
at its established transaction owner; a whole-module split is separate work.
No commit, deployment, live deletion, or live sign-in verification was performed.
