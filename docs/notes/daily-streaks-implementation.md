# App-wide daily streak

Status: global-only source implementation complete; final verification in progress.
The user's latest design supersedes the earlier independent language/app counters.
No language-specific streak is recorded, returned or displayed.

## Agreed behavior

- One workspace-owned app streak, independent of language, XP and proficiency.
- Minimum displayed count is one immediately. The first qualifying day records
  one; the next consecutive qualifying day records two. No 24-hour waiting period.
- Opening or returning to a visible, focused main workspace counts app use.
  A real pointer/keyboard interaction also counts, including after midnight.
  Background operation and diagnostic windows do not award days. No message,
  network response, paid access or earned XP is required.
- The selected language's XP occupies the full height of the left column.
  A divider separates it from the right column: global XP above the app streak.
  There is no full-width streak row and no language-specific counter.
- Preserve the compact XP bubble, larger XP numbers and smaller flame/count.
  Narrow layouts retain the app streak and identity markers. Composer focus
  does not hide the entire header.
- Upward number roll, flame pulse and six confetti particles, without sound.
  Day one never rolls from zero. Reduced motion shows the final number without
  movement or particles; disabling reward effects prevents celebrations.
- No visible explanatory copy or streak tooltips. The XP control exposes the
  count in its translated accessible description. The progress card shows the
  same app count without replaying an animation.

The explicit request authorizes this app-activity counter as an exception to the
older coaching plan's exclusion of login/count rewards. It does not change XP,
proficiency, learner evidence or other reward policies.

## Calendar and persistence

Native SQLite reads the device-local calendar date, UTC timestamp and observed
UTC offset at acceptance. Saved dates are never rebucketed after timezone changes.
Adjacent civil dates extend a run, including DST days shorter/longer than 24 hours.
A run ending today or yesterday stays current; a missed intervening date displays
one. No history displays one without inventing a qualifying date.

Repeated/backward dates preserve the latest count without awarding again. Forward
jumps across a missed date restart at one. No travel grace or invented days are
added. This local-only feature is not an anti-cheat mechanism.

Tracking is prospective. Workspace copies retain activity; Factory Reset clears
it. Conversation changes/deletion and language selection do not affect the app
streak. The main-window session binds one-time presentation claims; a lost response
can skip an animation but cannot erase an earned day. Disabled effects still
consume claims. A follow-up read cannot truncate an active animation.

## Global-only cleanup and compatibility

Removed language parameters/counters from native and generated contracts; removed
language validation and selection-dependent refreshes from streak reads. Removed
all streak additions from message command handling and transaction tests. Removed
the message-event subscription/publisher, dual-counter presentation branches,
language streak labels in all eight locales, and language-specific test/preview
fixtures. App use is owned solely by the foreground/activity path.

The existing format-72 SQL contract is unchanged because workspaces may already
have opened it. Its scope column remains only at the storage boundary: runtime
queries and writes hard-code the app scope (empty string), with no scope parameter.
Old language rows, if present, are inert retained history: never read into app
counts, updated or claimed. A regression explicitly verifies that isolation.
No extra migration, reset or silent data deletion was introduced. Historical
migration SQL and its validation chain remain readable from baseline format 45.

## Checklist

- [x] Simplify tracking.
  - [x] Remove language scope, message hooks and language-dependent state.
- [x] Simplify presentation.
  - [x] One app streak; remove obsolete labels, tests and preview controls.
  - [x] Language XP spans left; global XP and streak stack right with a divider.
- [ ] Verify cleanup.
  - [x] Inspect normal and narrow browser layouts.
  - [x] Pass affected UI tests (22).
  - [ ] Complete final fast gate, builds, native tests, Clippy and full UI suite.

## Verification boundaries and history

Production-component preview: http://127.0.0.1:1438/tools/streak-preview.html.
It imports the actual TopBar/ProgressCounters/StreakCount with fixture data.
The port-1437 prototype is superseded. Port 1420 is reserved for the app.
Native-window control is unavailable in this session; no live desktop foreground
or restart observation is claimed. Native persistence/restart and calendar behavior
are exercised by regression tests.

Before this simplification, the native suite passed 950 tests (five ignored), and
the UI suite passed 2,041 tests with two workers plus two added foreground tests.
The unrestricted UI run had timing failures that passed with bounded concurrency.
Those are prior-state results, not verification of the global-only changes.

The optional all-preview type check has unrelated stale fixtures (AI view,
conversation, localization, progress, skill levels and XP payout). The streak
preview passed its own strict check before this simplification. Existing Vite
bundle-size advisory is unrelated. No checks were weakened.

Earlier work removed the unused Run timeline translation from eight locales after
checking dynamic callers. Other unrelated shared-checkout changes are preserved.
No commit, release, deployment or application version bump is authorized.
