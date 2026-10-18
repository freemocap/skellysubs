# SkellySpeak

A convivial tool for learning languages through welcoming conversations,
useful assistance and understandable progress.

## Shared language behavior

Follow the repository's [language-independent behavior rule](AGENTS.md#language-independent-behavior).
Implement one general policy using Unicode properties and shared capabilities;
do not add language-specific code paths or character lists to solve a general
problem. Declarative language-config overrides are a documented last resort,
only after the shared approach has been shown insufficient. Preserve original
text; normalization belongs only to the operation that requires it.

## Repository layout

| Folder | Responsibility |
| --- | --- |
| [ui/](ui/) | React/TypeScript interface and frontend build configuration |
| [native/](native/) | Rust/Tauri application running on the user's device |
| [server/](server/) | Python/FastAPI hosted service |
| [content/](content/) | Editable language, learning-policy, topic and prompt data; AI behavior index |
| [redaction-policy/](redaction-policy/) | Shared diagnostic-data redaction policy and cross-runtime conformance cases |
| [docs/](docs/) | Documentation status, guides and existing website |
| [tools/](tools/) | Development, verification and release tooling |
| [Historical archive](https://github.com/freemocap/skellyspeak/tree/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/) | Removed from the checkout; linked snapshot is historical reference only |

The root npm package coordinates tools and the UI workspace. Dependencies are
installed with `npm ci`; the UI owns its own package manifest. Internal module
reorganization will follow this top-level move.

The website now contains task-based learner guides and interactive app-component
examples. The detailed developer descriptions below have not all been re-audited.
Historical references are not current specifications. See [documentation](docs/)
for the website build, source coupling and retained planning material.

**Current source implementation: immediate chat, recording/transcription and speech
playback, a separate coach thread, Google sign-in and custom-server execution.** Rust persists
messages, conversation settings and validated replies. The integrated conversation UI
reads those native records and uses native commands for sends and settings changes.

The interface centers the conversation in a continuous chat/coach workspace.
The global target-language selector switches language independently of the
partner picker in the conversation header. Record and Send share the composer;
coaching and evidence occupy a collapsible side pane. Narrow windows expose Chat
and Coach separately. History, progress, settings and secondary tools remain
available from the top bar. Unsent drafts are session-only. See the
[UX implementation and verification notes](docs/notes/conversation-ux-rebuild.md).
AI activity remains available under More → AI activity & tools. Its graph is
inspection-only; native Pause/Step/Cancel controls still need UI wiring.

Saved partner-reply translation and whole-message word glosses are implemented through
the scheduler. Structured coaching and source-derived XP now feed the skill map and practice
statistics. Vibe computation and measured garden rendering remain planned. Standard handles partner replies;
Fast has no active assignments until evaluated. Hosted access uses the service's
approved Gemini 2.5 Flash model.

## Development coordination

Follow [the working agreement](AGENTS.md). Coordinate native app runs because
only one runs at a time: it owns the app identity and local data.

## Run locally

Use Node.js 24, npm, Rust and the platform's Tauri prerequisites. Install and run:

```sh
nvm install
nvm use
npm ci
npm ci --prefix docs/docs-site
npm run macos:dev
```

On Linux, run `npm run tauri dev`. In an interactive terminal on Pop!_OS,
Ubuntu or Debian, this installs missing GTK/WebKit/ALSA development packages
automatically through apt; sudo may ask for your password. See
[Linux build dependencies](native/README.md#linux-build-dependencies).

The repository's `.nvmrc` selects Node 24. Run `nvm use` when opening a new
terminal here. If a launcher reports `ERR_UNKNOWN_FILE_EXTENSION` for a `.ts`
file, check `node --version`: an older system Node may be taking precedence.

On macOS, `npm run macos:dev` runs **Tauri dev**, with terminal log capture.
Tauri starts Vite, watches native inputs and rebuilds/restarts Rust; Vite handles
frontend HMR. Before Cargo executes each macOS build, our executable runner signs
and verifies it using an existing **Apple Development** certificate for the
FreeMoCap team (`U8LBJLBYPR`). Both the app requirement and Keychain partition
identity are checked: the latter must use the stable Apple Team ID, not a binary
hash. Self-signed certificates fail this second requirement. Automatic AI
connection checks remain enabled. Native restarts still end active recordings.

Restart an already-running dev launcher once to pick up this runner. macOS may ask
once to authorize the certificate-signed identity for an existing credential;
“Always Allow” can then match subsequent builds signed with the same certificate.
The runner fails if signing or verification fails; it never silently launches an
ad-hoc executable. It uses Cargo's executable runner and replaces its own process,
so Tauri retains native watcher, shutdown and restart ownership.

The signing identity must already exist in Keychain Access → My Certificates,
including its private key. To use another certificate, run
`SKELLYSPEAK_SIGNING_IDENTITY="certificate name or SHA-1 fingerprint" npm run macos:dev`.
Self-signed and ad-hoc certificates are rejected. No certificate trust or credential
access controls are changed. Another team can be selected with
`APPLE_DEVELOPMENT_TEAM`; multiple matching identities require an explicit certificate.

For a separate signed-bundle permissions test, use `npm run macos:dev-signed`.
It builds **SkellySpeak Dev.app** using the same identifier and signing requirement.
That standalone bundle route has no native watcher; restart it after Rust changes.
`npm run macos:dev-bundle` and `npm run macos:dev-sign` remain available separately
after `cargo build --manifest-path native/Cargo.toml --bin skellyspeak`.

`npm run macos:dev:check` checks launcher types; `npm run macos:dev:test` tests
argument forwarding. On macOS, `SKELLYSPEAK_TEST_SIGNING=1 npm run macos:dev:test`
also signs and runs two disposable Cargo builds, verifies identical designated
requirements and Team IDs despite different code hashes, and checks signature
reuse. It also creates a disposable Keychain item and reads it from a rebuilt
executable with system interaction disabled, then deletes that test item. It
never reads or changes existing app credentials.

Launcher logs record signing start/completion, PID, certificate fingerprint, Team ID,
code hash and expected Keychain partition. Native logs record Keychain operation
start/completion (1 read, 2 write, 3 delete), PID, duration and success, plus cache
hits. Credential identifiers, values and lengths are excluded. For OS authorization
reasons, inspect macOS `securityd` logs; app logs cannot observe the dialog itself.

On macOS, successful credential reads are reused in native process memory, so
frontend reloads do not repeatedly read the same Keychain entry. Concurrent reads
share the first authorization. Cached secrets are never written to disk or logs;
saving/removing a credential invalidates its cached value. Restart the app after
editing a credential directly in Keychain Access. A native-process restart may
still require an initial Keychain authorization if access was not remembered.

Run this from the repository root. `npm run dev` alone starts frontend assets;
local storage requires the native Tauri application. Quit an already-running
SkellySpeak workspace before launching another instance of the same database.
The app fails explicitly if the workspace is locked, invalid or incompatible.

The authoritative application version is in `native/Cargo.toml`. Dependencies
are locked in `package-lock.json` and `native/Cargo.lock`.

Build an unsigned local macOS inspection bundle (use the signed launcher above
when testing saved credentials):

```sh
npm run tauri -- build --debug --bundles app --config '{"productName":"SkellySpeak Workspace","bundle":{"active":true}}'
open 'native/target/debug/bundle/macos/SkellySpeak Workspace.app'
```

The distinct inspection bundle name makes the running workspace identifiable.
This is a local debug build, not a signed release or deployment.

### Android device development

With USB debugging enabled and the device authorized by `adb`, run:

```sh
npm run tauri -- android dev
```

This builds, installs, and opens a locally signed debug app on the connected
device without GitHub Actions or a release signing key. It uses the production
package identity, so Android requires uninstalling a released copy first: their
signing certificates differ. Keep the command running for frontend reloads. For
a local SkellySpeak server, start `npm run server:local` in a second terminal and
run `adb reverse tcp:8765 tcp:8765`; the device may then use
`http://127.0.0.1:8765/v1`.

For a standalone source-built APK, run:

```sh
npm run tauri -- android build --debug --apk --target aarch64 --ci --config '{"version":"2.1.1"}'
adb install -r native/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Keep the config version equal to `native/Cargo.toml`. This installs **SkellySpeak**
using `com.freemocap.skellyspeak` and the normal `skellyspeak://auth` callback.
Uninstall a differently signed release before the first local install; subsequent
local builds update the same app. The APK bundles the interface and works without
USB, while AI access still requires a reachable service.

### Share Android diagnostic logs

Open **More → Share logs** to attach one ZIP through Android's normal share sheet
(ChatGPT, email, messaging, or a file destination offered by the phone). The action
also appears in app error views. Choose the destination yourself; opening the
sheet does not send anything automatically.

The ZIP includes all retained app diagnostic runs, app/device/WebView versions,
and recognizable graphics failures from the recent app-only system-log buffer.
It excludes conversations, recordings and credentials. System-log access limits
are recorded in the manifest. Sharing preserves the original logs.

## Publish a release

The user authorizes a release version or bump; agents may perform the Git writes.
For a full release, require green CI before merging into `main`, then confirm CI
on the merged commit. The explicit development mode below bypasses that release gate.
From a clean, current `main` checkout, replace `X.Y.Z` with that chosen version:

```sh
nvm use # selects Node 24; older Node versions cannot run these TypeScript tools
next_version="X.Y.Z"
npm run release -- "$next_version" --dry-run
npm run release -- "$next_version"
```

The script updates the Cargo versions, commits, tags and atomically pushes to
`origin`. It refuses an existing tag, unsupported prerelease versions, or local
commits not yet on `origin/main`. Check CI yourself before invoking it; the script
does not query GitHub checks. Merging alone does not publish.

The tag starts the Release workflow for checks, signed desktop installers,
updater artifacts and signed Android APK/AAB. These publish as Latest when their
jobs succeed. The independent **iOS distribute** workflow builds a signed IPA
and attaches it to the same release. TestFlight upload is disabled. An iOS build
failure remains visible but does not block desktop or Android publication. PR CI also builds an
Android debug ARM64 APK and an unsigned iOS simulator app without release secrets.
The website rebuilds after a successful Release run.

### Faster development releases

For an explicit development release that bypasses the pre-release CI suite:

```sh
npm run release -- patch --skip-tests --dry-run
npm run release -- patch --skip-tests
```

This creates an annotated version tag recording the bypass. It skips the reusable
CI gate (tests, lint, docs and redundant platform preflight builds), while still
requiring tag/version agreement, main ancestry, every desktop/Android release
build, signing and artifact verification. Build failures still block publication.
It publishes a normal Latest release and updater feed, **not** a GitHub prerelease.
Branch and PR CI continue to run independently; development releases do not wait
for those results. The default command without `--skip-tests` retains full CI.

In GitHub's manual Release workflow, select an existing version tag and enable
`skip_tests` for the same bypass. Manual dispatch defaults to full checks even for
an annotated development tag. These options require the updated workflow on the
tagged commit; rerunning an old tag uses that tag's old workflow. Do not move tags.

Rust CI now allows 15 minutes for library compilation/tests and 30 minutes for its
whole job. A timeout is a failure, never a passing or skipped result. Release runs
remain serialized, so an already-running older release must finish or be explicitly
cancelled before the next one starts. No total release-time guarantee is implied:
compilation, signing and runner availability still take time.

### iOS distribution

The v0 iOS distribution workflow has been restored at the user's request. It
stages a temporary signing keychain and provisioning profile, injects manual Xcode
signing settings, writes ExportOptions.plist, and runs Tauri's App Store export.
It verifies the signature, bundle identity, build number, microphone permission,
debugging entitlement and signing team before retaining `ios-ipa` for 14 days.
A separate job attaches `SkellySpeak_X.Y.Z.ipa` to the matching release.
The TestFlight job and its manual input are commented out while App Store Connect
configuration remains a known gap. Manual dispatch can build without release
attachment by leaving `release_tag` empty.

Signing uses `IOS_CERTIFICATE_P12`, `IOS_CERTIFICATE_PASSWORD` and
`IOS_PROVISION_PROFILE` for team `U8LBJLBYPR`, app `com.freemocap.skellyspeak`.
The disabled TestFlight job would require repository variables `APPSTORE_ISSUER_ID`, `APPSTORE_API_KEY_ID`,
`APPSTORE_USES_NON_EXEMPT_ENCRYPTION` and secret `APPSTORE_API_PRIVATE_KEY`.
Active signing configuration remains required. Signing credentials are removed in an
always-run cleanup step. Cargo supplies the marketing version; the standalone
workflow run number plus attempt supplies the iOS build number.

The v1.21.1 signed archive and export succeeded; its added verification helper
failed on a dotted entitlement key. The restored workflow uses v0's codesign
team check instead. Local verification does not establish that the restored
workflow has passed on GitHub or that Apple has accepted a new build. See
[the restoration report](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/workflow/reports/ios-v0-workflow-restoration.md).

Desktop release builds install signed updates through the app. Android opens
https://docs.freemocap.org/skellyspeak/download for APK installation. Debug builds
do not install updates. Release builds use `native/tauri.release.conf.json` to
retain the distributed application's identity and signing continuity.

For subsequent releases, run `npm run release -- patch` on a clean, current
`main` checkout with Node 24, Cargo and authenticated Git access. It bumps both
Cargo files, commits, tags and pushes. `--dry-run` skips writes except fetching
remote state; `--no-push` performs local Git writes only.

## Start talking

Opening the app resumes the most recent active conversation. A fresh workspace
creates a Spanish partner and conversation automatically. The composer is immediately
available; no title or setup form is required. The New button in the conversation header starts another
conversation with copied preferences. The partner chooser opens a partner's latest
active chat or creates one. Names and settings remain editable afterward.

Record starts microphone capture for the current conversation. Stop transcribes
through the selected AI route and automatically sends the transcript when Auto-send is enabled (default on).
When disabled, the transcript stays in the composer for review and manual Send.
Discard cancels capture. Audio stays in memory, is capped at two minutes, and is
uploaded only on Stop. Hosted uses Google sign-in. Custom URL and Hosted audio use the service audio contract; new workspaces
select `whisper-large-v3` for STT and `eleven_v4_turbo` for TTS. Format 54
restores saved v3 speech selections to v4 once; later learner choices are preserved.
See [speech routing](content/speech/SPEECH_README.md).
Desktop capture uses native audio; Android/iOS use browser capture connected to the
same native transcription lifecycle. Automatic reading defaults on; both voice
preferences save per conversation. Desktop voice interaction has prior user
verification. Mobile capture code and Android build checks do not establish device
login/voice/update behavior; those checks and general speech fidelity remain separate.

The right pane starts with Coach and Experience tabs. Partner details open from the header picker. Coach exchanges
persist separately from partner messages and use the same gated execution
machinery. Partner prompts never include coach messages. The coach can explain
or suggest phrasing; it cannot apply settings changes. Saved word glosses
remain available in the conversation.

## Configure and use AI

Open **Settings → AI access → Hosted sign-in**, then choose
**Sign in with Google**. Complete authentication in your system browser and return
to the app. The account panel reports daily tokens, requests, monetary allowance and
its reset time. Request/token amounts remaining are estimates; money is authoritative.
The session stays in the platform credential store. There is no transcript sync.

Choose **Custom URL** to use your own SkellySpeak server. Save its address and,
if authentication is enabled, its server session token. Provider keys belong on
the server; the desktop app has no direct-provider access route. Standard/Fast,
transcription and speech model defaults are configured in Models. Shared speech
capabilities and optional language/variety preferences resolve the model actually
used; recording checks configured service availability before opening the microphone.

Settings save automatically with visible pending and failure states. Token entry
is masked; a blank field retains the saved token. Connection checks validate the
server protocol and report its provider checks without requesting inference.
Saving and verification are separate. Send uses the selected route and captured
model; responses are validated before publication.

The native execution controls are implemented; their UI wiring is pending.
At the command layer, Pause all prevents new starts; it does not revoke running work. Pause a turn and
Step to admit one operation while keeping that turn paused. The app-wide gate must
be resumed to Step. Cancel revokes publication and drops the local HTTP request;
remote execution and billing may continue. Eligible provider failures follow the shared bounded retry policy, with roughly 1, 2 and 4 second
delays plus jitter. Provider `Retry-After` is respected within a 30-second total
wait budget; longer waits require an explicit retry. Each refusal is retained in
the operation's diagnostic metadata. Responses that already produced text,
validation failures and ambiguous network outcomes are not automatically retried.
Other retries are explicit and may incur another charge. Restarted in-flight
requests show unknown outcomes and never auto-retry.

AI configuration changes invalidate affected pending work; it is never automatically
resent. Accepted messages and conversation preferences remain independently owned.

## Check this slice

1. Open the partner chooser, choose a language and create a partner. Edit the name, background or avatar.
2. Create two conversations. Change difficulty and Translation in the first.
   The next conversation starts with a copy; subsequent edits are independent.
3. Switch between the conversations and restart the app. Accepted settings persist;
   unsent drafts are intentionally session-only.
4. Archive and restore a conversation or partner. Deletion identifies its permanent
   scope before confirmation and removes dependent local records.

The app stores `skellyspeak.sqlite3` in its platform application-data directory under
identifier `com.freemocap.skellyspeak`, as configured in `native/tauri.conf.json`
and `native/tauri.release.conf.json`. On macOS this is
`~/Library/Application Support/com.freemocap.skellyspeak/`. No application data is
synchronized. Send transmits selected context through the selected hosted or custom-server route;
see [privacy and data flow](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/privacy.md).

## Verification

Run `npm run check:fast` before committing. It checks Rust formatting, localization
catalogs and source usage, unused message candidates, diagnostic policy, styles,
and the validation tooling's types and regression tests. It needs installed npm
dependencies and Rust with `rustfmt`, but performs no compilation or live AI calls.
It reports all failed checks and exits nonzero. Review unused-message candidates
before deleting translations; dynamic references can require investigation.

Root `npm test` runs this gate automatically before the UI tests. CI runs the same
gate first; compilation, full test suites and app builds wait for it to pass.
Android XML validation runs alongside it and also gates the Android build.
The fast gate does not replace the full verification below or the requirement to
wait for successful CI before tagging a release.

```sh
npm run check:fast
npm test
npm run build
npm run contracts:check
npm run styles:check
npm run ios:check
npm run ios:test
cargo fmt --manifest-path native/Cargo.toml -- --check
cargo clippy --manifest-path native/Cargo.toml --lib --tests -- -D warnings
cargo test --manifest-path native/Cargo.toml --lib
```

Rust integration-style tests use disposable SQLite files for persistence, revision
conflicts, settings copying, deletion, archives, duplicate actions and session rules.
Frontend tests cover snapshot ordering and draft scope. Vitest excludes dependency, dist, reference and Node launcher test directories.
Node launcher tests run separately with `npm run logs:test`. `npm run contracts` regenerates TypeScript declarations
from Rust; the check command detects drift without changing files.

Automated verification covers execution gates, duplicate Send/publication, cancellation,
source deletion, scoped snapshots, captured settings, interrupted attempts, output
validation and token retention. Loopback HTTP conformance tests exercise the actual
adapter, explicit model requests, redirect refusal and error redaction. They require
localhost networking permission and make no live AI calls.

Native request admission now shares four permits across partner chat, coach chat and
desktop transcription on all access routes. One audio request may wait for capacity;
excess waiting audio is rejected without submission. Tests cover mixed occupancy,
queue saturation, source/configuration invalidation and release on cancellation.
Queued chat/coach turns pause on matching HTTP 429 refusals, retaining the original
reason, diagnostics and reported retry time. These queue pauses survive restart;
Resume or Retry explicitly requests work again without a client reset-time gate.
Step does not release a refusal pause. Refusals do not create persistent access
lockouts for new chat, reading, generation or audio requests: the server decides
current access. Retrying one turn does not resume other paused turns, and failed
attempts are never automatically replayed. Transcription receipts now retain route, model, timing and outcome;
interrupted attempts become unknown on restart and are never replayed. Native execution snapshots expose these receipts, and usage projections include
them with unavailable token usage; presentation remains pending. Audio and transcript text are not stored in receipts;
this limit does not establish a bound on upstream work after local cancellation.
Capacity is provisional. Authored admission events persist to the native file sink
without time-based suppression. They identify capacity waiting, queue rejection
and wait duration without request content. See Development diagnostic coverage.

Automated checks cover PKCE/state validation, account decoding, hosted payload rules,
route capture, credential revocation, retained profile counts and local HTTP adapters.
Google browser authentication, keychain persistence and live hosted replies require
native verification; passing mock tests does not establish those results.
The native bundle builds successfully. Desktop and narrow-window layouts, direct
chat startup and title-free one-click creation were inspected. The keychain lookup
no longer holds the workspace lock. No database reset was needed. The user verified desktop recording/transcription and basic private coach replies;
other devices and providers still require capability-specific checks.
Hosted deployment passed its test, container and exact-revision traffic checks,
and the user confirmed hosted chat works. The client preserves documented
rate/allowance/spending-pause reasons and request IDs. See the
[security audit](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/SECURITY-AUDIT.md) for additional local hardening and remaining
repository/cloud checks; source changes require deployment or native restart.

The signed macOS development launcher passed local build, bundle/signature
verification, Vite readiness, native-process startup and termination cleanup checks.
Run `npm run logs:check` to type-check the logging launchers. Missing/ad-hoc signing identities
fail before launch. Account status no longer refreshes on window focus, preventing
Keychain dialogs from triggering another refresh when focus returns. Remembered
Keychain access across reloads still requires interactive verification.

Desktop Google sign-in uses a loopback callback. Mobile deep-link sign-in is pending.
The iOS prerequisite probe found no full Xcode installation; phone-device validation,
Windows, Linux and Android builds remain unverified.

## Architecture and roadmap

See [the repository map and documentation status](docs/README.md). Existing
architecture notes and plans in [the archive](https://github.com/freemocap/skellyspeak/tree/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/) are historical reference,
not authoritative descriptions of current behavior. The documentation content
audit and internal layer organization remain future work.

## Hosted service development

Active source, security boundaries, diagnostic contracts and deployment checks are
documented in [server/README.md](server/README.md). The app supports an on-demand
authenticated service status check; the matching server deployment is required.


## AI access foundation: current source checkpoint

Custom URL connects to a self-hosted SkellySpeak server. Hosted and Custom URL chat
use grouped `/operations`; audio uses the shared service contracts. Include `/v1` in the custom API base URL. HTTPS is required except
on loopback. No automatic endpoint or credential fallback is provided.

Custom Check connection calls authenticated `/protocol`, validates the protocol
version and configured chat/transcription capabilities, and performs no inference.
Our server requires a session token issued by that server. Selecting no authentication
cannot bypass server authentication. Hosted session credentials are never reused for
Custom URL; its token is stored separately and bound to the saved destination.
Simple translation and reaction tasks use the Fast model; other chat tasks use Standard. Read-aloud uses the shared AI access route and its selected
speech model; actual playback requires device verification. A protocol check does not establish live inference quality.

Standard and Fast model IDs apply to chat. Models settings selects only models,
including Transcription and Read aloud. AI access selects one route for every
capability: Hosted sign-in or Custom URL. Each capability uses only credentials
from the shared selected route; missing credentials fail explicitly without fallback.
Custom URL stores its address and authentication choice separately. Workspace format changes require migrations that preserve existing data.

Hosted and custom chat batch only operations sharing captured destination and
credential authority. Custom requests omit hosted install/platform/version headers.
Transcription remains a separate multipart request using its selected model and route.
The user verified local Custom URL chat with real configured inference keys;
all seven local Firestore emulator tests passed. Recheck the native development
session when resuming this branch; no new runtime check is implied by this summary.

Hosted and custom-server session tokens stay in the platform credential store.
The desktop credential service is `com.freemocap.skellyspeak.credentials`.
Old provider-named credentials are never read or copied into the new namespace.
Sign in again or replace the custom-server session token if needed. The app does not access the retired namespace, even for deletion; old entries
can remain in Keychain but are never used by this app.
The application supports only its current schema; it does not convert old settings
or retain retired route variants for historical receipts.

Restart the signed native development launcher once after updating its tooling.
It then watches Rust changes and restarts the app automatically; frontend reload
alone cannot load new native commands. AI access now has two tabs, Hosted sign-in
and Custom URL. Live Keychain prompt behavior must be checked in the signed app.

## Reply translation slice

When Translation is enabled at Send, the declared reply_translation operation waits
for the validated partner reply, then translates that message using the captured
explanation language and Standard target. No conversation history or coach content
is supplied to this task. Translation text and its operation state arrive through
the existing conversation snapshot and appear beneath the source reply.

The source message is immutable and uniquely owned by its turn; its ID is the whole-
passage identity for this slice. The result is stored in that turn's context JSON,
not as another conversation message. Source deletion cascades through its turn and
operations. Word glosses bind validated UTF-16 spans to this same immutable source.

A turn can be assisting after its reply is saved; this does not block the next Send.
Assistance uses the existing bounded permits, captured route and durable attempts.
There are no automatic retries or repair calls for translation. Explicit Retry
retries only the failed operation and preserves the saved reply. Attempt admission
reserves room for dependency work. Cancellation or deletion prevents late publication.

Changing Translation toggles display and applies to future sends; it does not
backfill existing replies. Panel hydration and reopening only read saved results.

Basic hosted translation has user QA and durable receipt verification: one reply
and one translation per exchange. The historical build plan is archived;
this does not establish every route or cancellation/restart scenario in live use.

## Word gloss slice

Each new partner turn declares one Standard word-gloss operation. After the reply is
saved, glossing and optional translation are independently eligible for the shared
four permits. There is no per-word inference or batch-fill delay. Existing messages
are not backfilled. Clicking a glossed word reveals its saved meaning inline; opening
or reopening the conversation creates no requests.

The model selects inclusive first/last grapheme IDs; native code derives exact
source spans and validates strict structured output before persistence. Valid partial results are usable. Malformed output fails explicitly,
retains reported usage and does not replace saved meanings. Retry word meanings
retries only that operation using the current AI access settings; it never regenerates
the reply or translation. Explicit retries have no lifetime attempt limit; automatic
repair remains bounded. Restarted unknown work requires explicit retry.

Tests cover sibling completion order, failures, cancellation, source deletion,
captured languages, partial results, restart and scoped retry. The latest local
voice run accepted four gloss attempts across three replies. Complete coverage and
linguistic quality remain the next focused slice; accepted output is not a quality
score. Reading/reopening must create no inference. Explicit retry targets glosses
only; expected new-turn work is reply, gloss, enabled translation and enabled speech.

## Voice integration checkpoint

The source implements transcription → automatic Send → partner text →
speech playback. Auto-send and automatic reading persist per conversation and
remain independently switchable. Speech is a source-bound scheduler operation,
using the shared AI access route and selected speech model; it shares admission capacity
with other AI work but does not block translation or gloss eligibility.

Playback reads bounded in-memory audio. Opening history does not generate speech
or autoplay it. Explicit replay can request audio; cancellation prevents late
playback, and restarting loses the audio cache without automatically regenerating
it. No operating-system speech fallback is used. Audio playback releases its Blob
URL when stopped or completed. Local cancellation cannot guarantee upstream billing
stops.

The user reports working desktop voice interaction. Latest local Custom URL
receipts show three successful transcriptions/replies/speech generations, with
speech and gloss running independently. General speech fidelity, stop/replay and
other devices still need their own checks. Current automated results and next
work were recorded in the archived build plan; detailed evidence is in
[the integration report](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/workflow/reports/integration-logging.md).

The local server and native app must both include the current audio protocol.
Normal local server restarts preserve the session token. Select Custom URL once in AI access and choose `scribe_v2`/`eleven_v3` for the configured ElevenLabs service;
see [the current setup guide](docs/notes/audio-provider-setup.md).

## Development diagnostic coverage

Start the native app with `npm run macos:dev` and the local API with
`npm run server:local`. Each invocation prints its private run directory under
`.local/logs/`. These directories are Git-ignored, readable by the current user
and agents on this machine, and retained across runs without automatic deletion.
Directories use mode 700 and files mode 600. Do not attach raw log directories to
issues or commits; review them for private data before sharing.

Each run captures process stdout/stderr in `stdout.jsonl` and `stderr.jsonl`,
including inherited child output. `launcher.jsonl` records lifecycle and exit
status. Each native launch creates a unique `native-<uuid>/` subdirectory with
`diagnostics.jsonl` (frontend events),
`native.jsonl` (native events/log facade/panic notices), and its manifest. The
local API adds `server-logging.jsonl`, `server-stdout.jsonl`,
`server-stderr.jsonl`, and its manifest. The outer process streams preserve
credential-redacted text; structured files preserve reviewed diagnostic fields.
`SKELLYSPEAK_LOG_RUN_DIR` groups these sinks under the launcher run directory.
Native reloads create fresh subdirectories there; they never reopen or overwrite
the previous process's streams.

For other local development tools, including the Firestore emulator, use
`node tools/dev-run.ts process <executable> <arguments...>` to capture their
inherited output. Running a tool directly bypasses that outer capture. Native
app runs started independently still create structured files under `.local/logs/`
in debug builds; release builds use the platform app log directory. Cloud-hosted
server logs remain in Cloud Logging and are not automatically mirrored locally.

Records are appended synchronously (Python/native streams flush each record).
Frontend delivery acknowledges the native file write before publishing a caught
fault; bridge delivery failures are explicitly reported and counted. In-memory
rings limit the UI read view only, not file retention. Files are readable while
processes run; this is not a guarantee against power-loss or hardware failure.

Interactive launcher output masks credentials but preserves ordinary content,
server addresses, local paths and build identifiers. URL user information,
queries and fragments are masked; a plain development URL remains clickable.
Saved process diagnostics use the stricter policy, including content and URL
redaction. Terminal output is therefore not a sanitized diagnostic export.
Multiline private keys and credentials split across writes are masked in both sinks.
The diagnostic policy is not a filter for conversation or model-response content
displayed in the application. Frontend/structured sinks
exclude arbitrary argument bodies, stacks, transcripts and provider payloads;
redacted bodies have explicit markers/counts. Unknown error causes are therefore
not complete error text. Incomplete process lines get immediate arrival markers;
the body is recorded on newline or orderly close. Lines over 65,536 characters
are explicitly redacted to bound memory. Abrupt termination can lose an unfinished
line's body or an unacknowledged frontend event. Bootstrap errors before frontend
capture and output from independently launched tools are not retroactively
recoverable. A disk write failure is an error, never a successful logging receipt.

When investigating a failure, inspect **every stream from every run since the
preceding checkpoint**, including successful events; do not start with an
error-only filter. Correlate timestamps, process/run IDs, frontend sequence/fault
IDs, and local durable operation/transcription/hold records. A missing inference
attempt does not mean no failure occurred: capture, settings and admission can
fail first. Report precisely which sources were read and any missing coverage.

Logging checks: `npm run logs:check`, `npm run logs:test`, `npm test`, native
`cargo test`, and `server/tests/development/test_logs.py`. Node launcher tests are separate
from the frontend Vitest suite. `.local/` is excluded from Vite's file watcher so
log writes do not reload the webview.

### Tracing native AI validation failures

`native.jsonl` records `inference_prepared` and `inference_validation` for scheduled
text/structured operations. Join events by `attemptId`, also present in workspace
attempt records. Inspect `validationAccepted`, `stage`, `finishReason`, token and
byte counts, `domainReason` and `structure` (safe schema path/reason). Preparation
records retain schema/instruction/content fingerprints and whether captured content
matches the running build. Provider success can still fail native validation;
validation acceptance alone does not prove the later database commit succeeded.
Speech outcomes emit `speech_validation` with attempt/operation IDs, decoder
acceptance and content-free transcript comparison profiles. Canonical Unicode
and whitespace equivalence are reported independently of playback acceptance;
transcript differences do not block otherwise complete, valid audio. No transcript
or source excerpts are logged. See the
[speech investigation](docs/notes/indic-live-failures-2026-09-17.md).

These events exclude raw prompts and responses. See the
[diagnostic investigation](docs/notes/local-errors-2026-09-16.md) for coverage and limits.

## License

SkellySpeak is licensed under the GNU Affero General Public License, version 3
or (at your option) any later version (**AGPL-3.0-or-later**). See [LICENSE](LICENSE)
for the full license text.

Third-party components and materials retain their respective licenses and notices.

### Real app smoke suite

`npm run e2e:android` drives the installed Android development app through Spanish,
Arabic and Chinese chats using its configured live AI route. It checks partner
starts, sent text, replies, feedback, glosses and Arabic joining. Add voice with
`npm run e2e:android:voice`; prerecorded speech goes through the real recording,
transcription and send flow, with only the microphone source substituted.
These explicit live runs incur provider usage. `npm run e2e:android:preflight`
checks access without inference. See [setup and verification limits](tools/e2e/README.md).
Missing devices and failed prerequisites fail the run; they are not passing tests.

### AI model evaluation

The [model-routing screen](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/workflow/benchmarks/model-routing/README.md) records
paid synthetic comparisons, output defects and the proposed Fast/Standard/Strong
task split. Production routing is unchanged. Run its offline checks with
`node --test tools/benchmarks/model-routing.test.ts`; paid execution is explicit
and requires the ignored `server/local.env` OpenRouter key.

## Adding languages

See the [language content guide](content/CONTENT_README.md) for typed YAML authoring,
local/shared definitions and inspection. The app bundles teaching content;
workspace data contains learner preferences and history. Learning-language IDs
use readable names and are independent of browser locales and UI translations.
Use **Browse languages** beside the compact selector or in **More** to inspect
varieties, romanization examples, guidance and the complete definitions.
Run `npm run languages:check` and `npm run contracts:check` after content changes.

### Conversation topics and prompt creator

Empty conversations show topic buttons, past/future practice choices and all five
difficulty levels. Select any combination, then let the partner start or send your
own message. Choices are available for every language and variety and do not
trigger generation on selection.

**Customize…** opens the Conversation Prompt Creator with Form, editable YAML and
read-only prompt/request preview views. It includes optional persona background,
all difficulty instructions, custom topics and saved-topic management. Apply changes
the draft; Cancel discards edits. Conversation settings reopen it after starting,
with changes applying to subsequent turns. Preview and actual requests share the
same native composer; editable prose lives under `content/prompts/conversation/`.

The active prompt source is
[`content/prompts/conversation/`](content/prompts/conversation/).
When persona background is enabled, the native composer keeps name, location,
occupation and age (when provided), then samples 1–3 secondary details from
background, current situation, manner, interests, opinions, interesting facts,
favorite books/movies and quirks. Each list entry is one candidate; blank details
are skipped. The random conversation ID seeds the selection, keeping it consistent
across previews, replies and retries for an unchanged profile. New chats can select
different details; repeats are possible. The full saved profile remains intact.
Romanized names and vibe emoji remain display metadata. Canned dialogue examples are currently disabled. The
selected difficulty is the final constraint after the opening/reply task. Openings combine a
small concrete contribution with an answerable question. For fresh conversations
without a selected topic, a stable hash of the conversation ID chooses an editable
`opening_angles` situation; preview and execution use the same choice. This varies
starting material without guaranteeing unique outputs. A learner-selected topic
takes priority, and follow-ups receive no new opening situation.
The `conversation-XX` value is a request version label, not another prompt file.
Experiment records in `docs/notes/` are not runtime inputs. Restart/rebuild the native app to load YAML
changes; the prompt preview shows the same assembly used by new turns.

Lesson generation, quizzes and lesson handoffs have been removed. Coaching,
evidence and conversation rewards remain. See the
[implementation report](docs/notes/conversation-prompt-implementation-2026-09-18.md)
for scope and verification.

Variety support uses separate target and explanation choices, plus an independent
interface locale. See the [content guide](content/CONTENT_README.md).
Workspace format **54** supports automatic upgrades from format **45** onward.
Every stored-format change includes a consecutive migration; app releases do not
necessarily change the database format. Startup retains a local recovery copy and
upgrades the complete chain in one transaction, preserving learner history.
Older unsupported, newer and damaged databases are refused without automatic reset.
See [workspace migration policy](native/README.md#workspace-migrations).
App builds carry their own teaching content.

The September 14 workspace redesign and its verification limits are recorded in
[the design-pass report](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/workspace-redesign-report.md).

“Save a copy of my data” in Settings and schema-refusal recovery copies the database,
SQLite sidecars to Downloads. App-owned teaching content is not workspace data.
Copy failures leave no published partial backup.

### Diagnostic log export and privacy

Use the single log-export control in More or an error surface. On Android,
**Share logs** opens the system chooser, whose **Save logs** destination opens a
file picker. Error panels use a compact export icon beside the dismiss control.
On iOS, **Share logs** presents the system share sheet with the ZIP attached;
choose a sharing app or **Save to Files**. Dismissing the sheet cancels export.
Desktop uses **Save logs** and saves to Downloads. Every export also refreshes
`logs/execution-diagnostics.jsonl` from the current workspace's retained AI history.
It includes conversation and workspace runs, all attempts (including retries),
shared execution IDs, recorded states and adapter-redacted response observations.
Generation failures are included even if playback never starts or no UI fault is
raised. Partial response metadata is retained alongside the final failure.
The snapshot explicitly identifies unavailable workspaces and missing evidence;
a snapshot read/write failure fails export rather than silently omitting history.
The ZIP excludes conversation content, graph inputs/outputs, provisional text/audio,
recordings and credentials. Existing retained failures are included after updating;
no regeneration is needed.

Privacy rules live in `redaction-policy/policy.json`; run
`npm run diagnostics:policy` after editing and `npm run diagnostics:check` to check
all generated runtime copies. Redaction uses `[secret redacted]` and
`[user content redacted]`, preserving surrounding explanations. Error receipts and
provider receipts remain separate so successful HTTP responses cannot hide later
validation failures. See the [audit and verification report](docs/notes/centralized-diagnostics-audit-2026-09-21.md)
for coverage and platform limits.
