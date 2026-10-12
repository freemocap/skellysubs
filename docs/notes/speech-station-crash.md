# Speech station crash — 2026-10-11

Status: reproduced, fixed in source and verified by automated tests. The user's
already-running application has not been replaced or restarted.

## Cause

The AI activity view passed a speech request with `state: waiting_dependencies`
to `turnStations`. Native `graph_runtime/message_view.rs` legitimately projects
`Disposition::Waiting` to that state. The viewer's independently maintained tone
table omitted it and threw `Unknown speech request state: waiting_dependencies`.
The app-wide error boundary replaced the view. Its unwrapped diagnostic JSON
then overflowed its container, as shown in the user's screenshot.

The incomplete table was present in commit `05783658` ("final cleanup"). This is
a presentation contract mismatch, not evidence of database corruption or a failed
speech provider. The screenshot cannot establish which prerequisite was pending.

## Fix and prevention

- Reuse `operationPhase` instead of a separate operation-state interpretation.
  Map its finite phase union exhaustively to presentation tones.
- Handle the native speech projection's `unrequested` fallback explicitly and
  preserve the previously accepted `pending` alias.
- Preserve the exact reported state in inspection details. Unsupported values
  still fail explicitly rather than becoming a false success or silent fallback.
- Reuse the shared wrapping style for crash diagnostics.
- Exercise speech-bearing snapshots through AiView. Earlier activity fixtures
  omitted speech, so they never reached the failing branch.

The new activity regression reproduced both omitted-state crashes before the
fix (2 failures, 22 passes). Afterward, 92 tests passed across nine activity and
conversation-domain suites. The production build and fast gate passed. Domain
tests also cover the accepted aliases and rejection of unsupported states.

## Remaining hardening

`SpeechRequestView.state` is generated as `string`; the native producer does not
currently export its possible product-state values as an enum. A native-owned,
generated finite state contract would make future missing mappings compile-time
errors. This is a recommended follow-up, not implemented or claimed as a guarantee
by this fix. A local activity error boundary could separately reduce the impact
of other viewer defects; the present fix does not change error-boundary scope.
