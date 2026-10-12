# Empty start screen with completed opening — 2026-10-11

Status: diagnosed from the running process's logs and read-only workspace
metadata. UI guard implemented and tested; the user's running native process
has not been stopped, replaced or restarted.

## Evidence

- Running PID 63932 resolves to `native/target/debug/skellyspeak.exe`, last written
  at 16:43:37 America/New_York. Its native log manifest identifies that PID.
- In `.local/logs/native-26de32de-9efd-442a-97cf-bdc09e2727d3/diagnostics.jsonl`,
  entries 169–171 and 183–184 record `execute_command` conflicts at 20:57:18,
  20:57:54, 20:57:57, 20:58:02 and 20:58:03, respectively:
  `This conversation has already started.` A UI mount lies between those groups.
- Read-only SQLite queries show the two conversations created around those
  times each contain a partner opening, one succeeded `persona_opening` turn,
  and one saved message. Message text and credentials were not needed or printed.
- The running executable lacks the serialized `contentRevision` field introduced
  by the performance work. The prepared matching native candidate contains it.

## Cause and fix

The updated UI's content-identity optimization assumed every native snapshot
supplied a content revision. Against the older running native process, successive
missing values compared equal (`undefined === undefined`), so advancing workspace
revisions could retain the first empty display. Native had already accepted and
completed the opening, so pressing Start again correctly failed.

This is a development native/UI contract mismatch, not a reason to delete history
or weaken the native duplicate-start protection. Reloading the UI does not replace
the native process. Recovery requires quitting and launching a matching rebuilt
native executable with the existing workspace.

The UI now validates content revisions before observation, merging and identity
reuse. Missing or malformed values produce an explicit incompatibility error with
restart guidance. A stopped reader no longer offers stale conversation-start
controls. Unknown content identity is never interpreted as unchanged content.

Six missing/malformed/later-response regression cases failed before the guard.
All 70 affected observer and conversation-page tests then passed, as did the
application build and fast gate. The running user's workspace was only read.

## Prevention and limits

Keep native and frontend builds paired when introducing IPC fields. TypeScript
types describe a contract but do not validate data from an already-running older
binary. Exercise missing fields at runtime, not just fully populated fixtures.
A general startup protocol compatibility handshake is a possible follow-up; this
change validates the specific contract required for safe content reuse.

The diagnostic events omit conversation IDs, so timestamps plus persisted state
were needed for correlation. More precise allowlisted command identity metadata
would improve future diagnosis without logging message content.

## Restart verification and PR checkpoint

After the user launched `npm run tauri dev`, PID 68924 ran the rebuilt executable
with the content-revision contract. Its session
`native-01afbdd9-91b8-494a-8883-f38915e87632` contains 1,563 frontend records from
21:06:15 through 21:16:49 America/New_York: 1,526 debug and 37 info, no warning or
error level events. Seven speech requests reached playing and finished; seven
microphone auto-sends were recorded. Native logs independently show seven completed
captures (nine starts; completion is not claimed for the other two).

One `run_reading` rejection was recorded at info level. The IPC adapter uses that
level only when its abort signal is set and native reports `reason: cancelled`.
It is an expected cancellation, not evidence of another application crash.
Neither of the reported crashes/conflicts recurred in this observed session.

Final full UI run: **302 suites / 2,100 tests passed** in 130.28 seconds. The final
code state's build, fast gate and recorded desktop replay passed in the preceding
verification. Native source is unchanged since the recorded 1,010-test native run,
Clippy, binary and generated-contract checks. These are local results, not hosted
CI status or an exhaustive new review of the entire branch versus main.

Recommendation: ready to package the uncommitted work into a reviewable PR.
Merge remains subject to commit-level CI and branch review. The branch is large;
the log check does not certify every changed subsystem or physical mobile flow.
