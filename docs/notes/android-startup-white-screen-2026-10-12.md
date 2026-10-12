# Android 4.0.0 startup white screen

Status: Android and iOS startup corrections implemented and verified on the
current build. Final acceptance results are recorded below. Changes remain
uncommitted and unpublished; device coverage is limited to the tested environments.
The initial investigation below is retained as observations, followed by the
implementation findings.

## Observed behavior

On October 12, the attached Pixel 9 Pro XL ran Android 17 with Vanadium WebView
155.0.8059.39.0. The installed SkellySpeak package was version 4.0.0,
versionCode 4000000. Installation succeeded, but a cold launch displayed a blank
light background with no app text or labelled controls.

Repeated cold launches reproduced the blank screen. Sending the app Home and
reopening its existing activity displayed the welcome/language setup screen.
This recovery was verified twice on the original installation. A combined
device check produced:

```text
Cold launch after 5 seconds: FAIL — welcome screen absent
Background and reopen: PASS — welcome screen visible
Blank-screen reproduction confirmed: True
```

The check force-stopped and launched the package, waited five seconds, then used
`uiautomator dump` to check for the welcome screen. It subsequently sent
`KEYCODE_HOME`, reopened the activity, and repeated the assertion. An earlier
assertion that merely checked for WebView child nodes was insufficient: an empty
nested WebView counted as content. The final check uses visible app labels.

## Diagnostic isolation

An isolated, locally signed diagnostic copy of the installed APK reproduced the
blank screen with a fresh workspace. Its ARM64 native library and executable DEX
files were unchanged. Only the manifest identity, app label, provider/permission
identities, callback scheme and debugging flag were changed, and other CPU
architectures were omitted. The original app and its private data were preserved.

The diagnostic WebView loaded its HTML, JavaScript, styles and dynamically
imported window/event modules. The document reached `readyState: complete`, while
the React root remained empty. Native diagnostics recorded an initial
`get_startup_state` request beginning without a corresponding completion at the
time of inspection. A later manual, read-only invocation of that command returned
success with no workspace refusal or cleanup error. Reloading the diagnostic
WebView also reached onboarding, with startup commands completing normally.
These probes can affect timing; they do not establish precisely when or where
the original request was delayed or lost.

The release disables WebView debugging and private-file access through `run-as`.
The separate diagnostic package was used to inspect the bridge without replacing
or clearing the original installation. Android documents automatic WebView
debugging for debuggable apps in recent WebView versions:
<https://developer.android.com/reference/android/webkit/WebView#setWebContentsDebuggingEnabled(boolean)>.

## Initial interpretation (superseded by source findings below)

Evidence points to a startup/lifecycle timing problem involving initial native
communication. Missing frontend assets and corruption of the user's existing
workspace are not necessary causes: assets loaded, and the isolated fresh
workspace reproduced the failure. The exact native/WebView boundary remains
unproven. Do not label this as a confirmed Tauri defect or adopt an arbitrary
startup delay/retry without a regression test.

An older renderer was killed for low memory at 08:48 device time. Bringing that
old process forward later produced Chromium's renderer-termination messages.
Fresh-process blank-screen reproduction and lifecycle recovery are separate
evidence; that historical low-memory event does not explain the current bug.

The UI awaits startup before mounting a screen, leaving this stall blank. A
source fix needs a verified startup readiness/completion contract and a visible,
bounded failure path; diagnostic delivery must not prevent displaying failures.
Any retry must distinguish read-only startup checks from commands with side
effects. Verify cold launches, fresh and existing workspaces, and lifecycle
transitions on a release-equivalent device build.

## Initial investigation scope

No production APK update, reset, release, version change, commit, push or
deployment was performed. No learner setup selections or provider requests were
made. Verification covers reaching onboarding, not conversation or audio flows.
At this initial stage, the checkout already contained version 4.0.1, but only
the installed 4.0.0 executable had been tested. Later source-build verification
is recorded below.


## Source fix

The locked Tao 0.35.3 Android event loop drains its user-event channel only for
`EventSource::User`. Android can consume the wake signal while reporting another
ready event, stranding queued invoke responses. This is the exact upstream bug
fixed by Tao PR 1304 / commit `beb88249`, released in 0.37.0. [@taoAndroidWakeDrain2026]

Tauri runtime 2.11.4 requires Tao 0.35. A local Cargo path patch backports the
upstream unconditional queue drain to the compatible 0.35.3 source, preserving
other platform code and the event-handler API. License and provenance are beside
`native/vendor/tao`; only the Android loop is behaviorally changed. No startup
sleep, repeated commands, forced reload or synthetic lifecycle event is added.

The device regression is `npm run e2e:android:startup`. It checks actual accessible
app content after each cold process launch, without a WebView debugging connection
or recovery action. Against the original 4.0.0 release it failed after the
12-second deadline (13,010 ms including the last hierarchy capture):

```text
FAIL cold launch 1/1: 13010 ms
Expected app surface did not appear without user interaction. No recovery was attempted.
```

The runner has parser regression tests covering empty nested WebViews, labels
from other apps, unavailable hierarchies and XML-escaped labels. These validate
the observer; only the actual device run validates Android startup.


### Second startup race

The first event-loop-only release test reached onboarding on six consecutive
cold starts, then failed on the seventh with a visible native command error:
`state not managed for field state on command get_startup_state`. This is a
separate ordering bug that the formerly stalled IPC could hide.

Tauri 2.11 creates configured windows before calling the application's setup
hook. Our setup registered `Arc<Application>` only after opening/recovering the
workspace. A fast renderer could therefore invoke `get_startup_state` before the
state existed. `native/tauri.conf.json` now defers the main window (`create: false`),
and `application/startup.rs` explicitly opens the configured window after state
registration. A guard rejects opening it without managed state. Existing geometry,
URLs and workflow replay cursor behavior are retained. [@tauriSetupOrdering2026]

The native regression first failed against the automatic-window configuration:
`The frontend must not issue commands before application state is managed`.
It now runs Tauri's real setup path under its mock runtime and checks that no
window exists before application setup. Additional tests check that a window
cannot open before state registration, its first real startup command succeeds,
and a refused workspace can still open a window and report its refusal. These
checks are separate from device scheduling verification.


## Automated verification of both fixes

- `npm run check:fast`: passed.
- `npm run e2e:check`: passed (types and three tooling tests).
- `npm run build`: passed during the Android release build.
- `cargo clippy --locked --manifest-path native/Cargo.toml --lib --tests -- -D warnings`:
  passed; the vendored upstream dependency emits existing dependency warnings.
- `cargo test --locked --manifest-path native/Cargo.toml --lib`: 1,002 passed,
  zero failed, five ignored, including all three new startup regressions.
- ARM64 Android release APK build: passed. The CLI configuration used the
  checkout's existing 4.0.1 version; no source version was changed.

The non-debuggable verification APK uses a separate package identity and the
label **SkellySpeak Startup Test**. Its ARM64 native library is byte-identical to
the newly built release library (SHA-256
`8517be499ae9cd9819987271f9d14787a2337509901cc0414dfe8ecb03a31692`).
The original 4.0.0 app and its private data remain intact.

Device runs interrupted by another foreground app or a locked screen are not
successful tests. The runner wakes the screen before starting each process,
without sending input or lifecycle recovery actions after launch. A 12-second
run of both fixes passed the fresh-install launch, then exceeded its deadline
on the next launch (12,519 ms). A later inspection found onboarding displayed
without recovery. Therefore that run does not prove the short timing target;
subsequent verification uses a 30-second observation deadline. Timings include
ADB launch and UI hierarchy capture, not just application rendering time.


## Device comparison with a 30-second deadline

The original installed 4.0.0 release failed the same cold-start check with the
longer deadline: `FAIL cold launch 1/1: 30412 ms`. No recovery was attempted.
This preserves a failing baseline under the final observation budget.

The fixed release-equivalent package reached onboarding on 18 consecutive cold
launches, with observed times from 7,114 to 12,381 ms. On launch 19 the Android
`uiautomator dump` command failed, stopping the requested 20-run batch. A separate
read-only hierarchy capture then confirmed onboarding was visible with the app
still foreground and the phone unlocked. No recovery action was used. This is
18 automated passes and one observer failure, **not** a successful 20-run batch.


A final fresh installation of the fixed test package passed all three requested
launches: first launch 7,446 ms, then cold starts at 7,294 and 7,328 ms. Combined
with the earlier batch, this is 21 automated passing launches under the
30-second deadline, including a fresh-install launch; it does not erase the
observer failure or establish a strict 12-second startup guarantee.

At this earlier checkpoint, the fixed **SkellySpeak Startup Test** app was installed
at onboarding for inspection (subsequently removed by the user; see final results). The original release installation and data were preserved. No source
commit, release version change, tag, push, publication or deployment was made.
These results cover startup/onboarding on this Android/WebView combination, not
all devices, conversation functionality or audio. The native mock regression
covers refused-workspace startup; that case was not injected into the phone.


## Follow-up after the user reported a first-load error

The user reported an error/reload screen followed by success after retry. They
could not identify which installation produced it. On initial inspection the
original 4.0.0 package was foreground, but the fixed test package also retained
learner setup progress to the AI-access step. Neither fact establishes which app
showed the error. The original release passed several launcher starts during
follow-up, so passing a small batch is not proof that this intermittent defect
is absent.

The fixed test package's six retained native runs included successful startup
IPC and no recorded frontend warning/error. A temporary same-signature manifest
update enabled local diagnostic access without changing executable code or
clearing data; the non-debuggable APK was restored afterward. No original app
private data was read or reset. The user's exact follow-up error remains
unconfirmed; do not invent its text or claim a third Android source defect was
identified.

The Android test now uses the launcher intent, explicitly rejects error/reload
surfaces, requires two consecutive healthy screen observations, and supports
Home/launcher resume checks. A parser test failed before the explicit error
assertion and passed afterward. A connected phone is temporarily kept awake and
the setting is restored in a finally block. Locked-screen interruptions are
reported separately. Testing the configured app with the old welcome expectation
correctly timed out: it was displaying AI access, not a blank/error screen.

With the expectation corrected to its retained setup state, the non-debuggable
fixed build passed five cold launcher starts (7,205–15,680 ms including observer
overhead) and three Home/launcher resumes (6,356–6,589 ms), without retry/reload.
The setup selections were preserved. These tests do not resolve the unidentified
historical error by themselves.

## iOS investigation and implemented corrections

The shared state-before-WebView ordering correction also applies to iOS. The
Android queue-drain correction does not change iOS runtime behavior.

An unsigned ARM64 simulator build initially failed to link on Xcode 27. Installing
its required `llvm-tools` Rust component resolved upstream Swift C exports. The
private diagnostic plugin still failed to export `init_plugin_diagnostic_sharing`
because its Swift module name differed from the product/package name used by
swift-rs's export handling. Aligning those names restored the global symbol and
allowed the app to build. No signing or publication was performed.

The first actual iOS 27 launch then terminated with `EXC_BREAKPOINT` in
`___UIApplicationEvaluateRuntimeIssueForNoSceneLifecycleAdoption_block_invoke`.
The XCTest startup check failed on its first launch. This is a separate confirmed
startup defect: new-SDK apps need scene lifecycle adoption. [@appleSceneLifecycle2026]

`native/Info.ios.plist` now declares a static Tao scene with multiple scenes disabled.
The vendored runtime uses manifest presence, rather than the multi-window flag,
to select scene setup/window attachment, and registers its scene delegate before
UIKit resolves the static class name. This adapts the relevant explicit-manifest
policy from upstream without adding multi-window product behavior.
[@taoSingleSceneStartup2026]
The scene configuration callback now autoreleases its returned object instead of
returning a pointer that is freed at callback exit. [@taoSceneConfigurationLifetime2026]

The retained XCTest suite in `tools/e2e/ios-startup/` checks cold starts, first-render
stability and Home/activate transitions without using reload or clearing data.
Runtime verification of the corrected build follows below.


### Current user-directed acceptance scope

The user subsequently uninstalled both Android installations and requested one
normal installation built from current source. Historical release diagnosis and
requiring the user to distinguish installations are no longer the acceptance
workflow. Rebuild current source, install with its normal package/name, and verify
fresh first launch plus cold starts and reopening. Keep the runtime in release
mode; a local debug signing certificate is only the installation signature, not
an app debug build. Future distributed updates require the release signing key.
The user does not want multiple simultaneous installations.

A physical iPad was offered. The available iOS 27 iPhone and iPad simulators can
exercise startup here; simulator evidence must remain distinct from a physical
iPad check and from coverage of other OS/WebView versions.


## Final current-source verification

The normal `com.freemocap.skellyspeak` package is the only SkellySpeak installation
on the attached Android phone. It was built from current source as a non-debuggable
release APK, locally signed for device testing. The repository's existing 4.0.1
version was preserved. No historical or separate test package was reinstalled.

- Pixel 9 Pro XL, Android 17: fresh-install first launch and four further cold
  launcher starts passed (11,229; 10,321; 10,546; 10,376; 15,560 ms).
- The same installation passed three Home/launcher resumes (6,430; 6,669; 6,821 ms).
  Android timings include two UI hierarchy observations and are not render-time
  measurements. The phone's original stay-awake setting was restored to zero.
- iPhone 18 Pro simulator, iOS 27: five cold launches and three Home/activate
  resumes passed; XCTest completed with zero failures.
- iPad Pro 11-inch (M5) simulator, iOS 27: the same five cold launches and three
  Home/activate resumes passed; XCTest completed with zero failures.

Every final launch check required visible onboarding without an error/reload
surface. No Retry/Reload, reset or provider call was used. iOS checks also verified
that the successful screen remained stable after two seconds. During verification,
a new bibliography entry with an invalid review enum caused a content refusal;
that authoring error was corrected, content validation passed, and both final
platform builds and launch suites ran with the corrected entry.

Validation on the final source included the fast gate, E2E tooling types/tests
(four tests), UI production builds, native Clippy, content validation (21 languages)
and the full native suite (1,002 passed, five ignored, zero failed). Clippy retained
existing capped warnings from the vendored dependency; application checks passed.

The retained test suites make these launch checks repeatable. These results cover
startup/onboarding on one physical Android/WebView combination and two iOS 27
simulators. They do not establish coverage of every phone, older OS versions,
physical iOS hardware, conversations or audio. A physical iPad check remains useful
additional verification, not a completed result. The unidentified earlier user
error is not retroactively attributed to a specific installation or cause.

No commit, tag, push, source version bump, publication or deployment was performed.
