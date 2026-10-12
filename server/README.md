# Hosted service

The active API source and deployment configuration live here. Authentication,
request/body limits, model pricing ceilings, transactional budget reservation,
settlement and session revocation apply to hosted AI calls.

## Shared language behavior

Follow the repository's [language-independent behavior rule](../AGENTS.md#language-independent-behavior).
Implement one general policy using Unicode properties and shared capabilities;
do not add language-specific code paths or character lists to solve a general
problem. Declarative language-config overrides are a documented last resort,
only after the shared approach has been shown insufficient. Preserve original
text; normalization belongs only to the operation that requires it.

## Folder map

| Folder | Responsibility |
| --- | --- |
| [app/](app/) | Hosted FastAPI application, authentication, AI proxying, admission, spending controls and diagnostics |
| [tests/](tests/) | Server tests and shared test environment |
| [development/](development/) | Local launcher, private logging and example environment configuration |
| [operations/](operations/) | Administrative usage reports and reservation reconciliation |
| [deployment/](deployment/) | Cloud Build configuration, deployment/revision verification, retention provisioning and upload checks |

Python imports use the `server` package. The container starts
`server.app.main:app`; root `npm run server:local` uses the local development
launcher. From the repository root, administrative tools run with
`uv run --project server python -m server.operations.stats` or
`uv run --project server python -m server.operations.reconcile` and their arguments.

### Application and test subfolders

`app/identity/` owns authentication; `admission/` groups request limits and work
claims; `inference/` groups AI contracts, routing, grouped execution, streaming and
audio input; `accounting/` holds budget/quota; `diagnostics/` holds account reports
and request logging. `main.py`, `config.py` and `transactions.py` remain at the app
root. These groups preserve existing implementations and cross-domain calls.

Tests use matching subject folders, plus `integration/`, `development/`,
`deployment/` and `operations/`; `tests/conftest.py` sets up the fake environment.
`development/launcher.py` starts the ordinary API with disposable process-local storage, while
`development/logs.py` captures Python logs. Their logging ownership and the root
process logger will be reviewed separately; this folder pass does not redesign them.

Dockerfile, .dockerignore, pyproject.toml and uv.lock stay at this build-context
root. The private `.env`, `.local-server/` state and `.venv/` stay here too;
only the public example moved into `development/`. Runtime files remain explicitly
allowlisted for the image and source upload. Large-file splitting and deeper
domain subfolders are deferred. Working notes belong in [docs/notes/](../docs/notes/).

## Diagnostics

`GET /v1/diagnostics` and `GET /v1/me` require the same signed, unrevoked
session as chat. Together they use a separate Firestore admission lane, defaulting
to 480 calls per account per UTC day and 600 calls globally. The owner panel can
change those defaults through effective service overrides. They do not debit the
inference/account daily request counter.

The deployment configuration defaults to a maximum of 24 registered accounts.
Saved owner-panel overrides take precedence over deployment defaults.

Signed-session short-window admission has two lanes per process:

| Lane | Per subject/minute | Per process/minute |
| --- | ---: | ---: |
| Chat and transcription | 600 | 2,400 |
| Account status and diagnostics | 30 | 60 |

A combined 2,460/minute ceiling remains, with at most 128 active subject windows.
An inference flood cannot consume the control lane's reserved capacity. Identity
storage exhaustion or infrastructure saturation can still prevent diagnostics;
these process-local gates are not distributed in-flight admission. Anonymous
traffic has a separate 240/minute process gate; GET /health has a separate
60/minute gate. These application limits do not provide edge DDoS protection. Failed/revoked
signed-session attempts consume admission; invalid signatures perform no database
work. The endpoint does not accept a target account ID.

It returns the caller's daily request usage/limit and allowance usage/limit,
shared exhausted flags, spending-pause state, UTC reset time, and revision.
Shared balances, other identities, secrets and transcripts are excluded. Usage
includes reserved amounts: a request can exceed remaining allowance even before
an exhausted flag becomes true. This is a transactionally read admission snapshot,
not a provider probe or a guarantee that the next request will be admitted.

The app exposes this under Settings → AI access & models → Hosted → Service
diagnostics → Check service status. It never polls automatically. A missing route
before deployment reports an HTTP failure, not a healthy result.

Errors carry `code`, `detail`, a generated `request_id` and applicable `resets_at`.
Short-window rejections include Retry-After. Codes distinguish personal/shared
account/diagnostics daily admission, ingress, auth throttling, allowance exhaustion,
spending pause and transcription capacity. Spending pause has no automatic reset.
Unexpected failures return a generic INTERNAL_ERROR with a request ID.

JSON application logs contain generated request ID, matched route template,
status, rejection code, exception class when applicable, revision and elapsed time
to response headers. They exclude raw paths, query strings, IPs, identities,
headers, tokens, bodies and exception messages. They do not claim that a streamed
response finished successfully. Cloud Run's separately managed request logs have
Google's own fields and retention; these application changes do not configure them.

`/health` remains lightweight liveness, not account/Firestore/provider readiness.
The learner diagnostics endpoints do not expose resets or administrative data.
The separate owner-only administration surface is documented below.

## Verification and deployment

Startup has no media decoder dependency. Uploaded PCM WAV is validated in-process
before provider dispatch; audio interpretation belongs to native code.

From the repository root:

```sh
uv run --project server --frozen --group dev pytest server -q
```

Requires Python >=3.12. No media decoder is required. Firestore emulator tests skip unless
`SKELLYSPEAK_FIRESTORE_TEST=1`; the GitHub workflow runs these with the emulator.
It also builds the container, checks liveness and unauthenticated rejection before
deploying. The runtime image includes an explicit list of application modules;
local secrets, tests and administrative scripts are not included. Tests check
that every runtime module is copied and boot that copied package in isolation,
so checkout imports cannot hide a missing Docker input.

`.github/workflows/deploy-server.yml` runs on main for
server changes or manual dispatch, using configured Workload Identity Federation.
`gcloud beta builds submit` surfaces build output from Cloud Logging. Build helpers
are digest-pinned. `server/deployment/deploy_candidate.py` resolves the pushed image to an immutable
digest and deploys a build-specific revision with no traffic. Only that exact ready
revision with matching image/digest can be promoted explicitly to 100%; resulting
traffic is verified. A failed deployment never promotes, even if its revision
subsequently appears ready. Reports contain only image/revision/traffic metadata
and failure codes; raw gcloud errors, runtime logs and service specs are withheld.
The workflow also checks that unauthenticated diagnostics return 401.

No production deployment or counter reset is performed by local tests. A green
workflow must be followed by an authenticated diagnostic check and one hosted chat.

Before any root-level Cloud Build submission, run `python server/deployment/check_upload_manifest.py`
with gcloud installed. It checks the actual gcloud upload manifest using harmless
private-file sentinels and a temporary CLI config; it does not upload or authenticate.
Both `.gcloudignore` (source archive) and `server/.dockerignore` (container context)
must include runtime/build inputs and exclude credentials, tokens, tests and tooling.

## Local review result

Security regression coverage includes unauthenticated/invalid-token denial before
Firestore access, revoked-session denial, independent bounded diagnostic admission,
account-scoped reports, read-only snapshots, secret-free error/log output, distinct
budget codes and deployment metadata redaction. No separate GitHub probe workflow,
GCS report bucket or diagnostic service account is added.

Historical verification for the earlier deployment/admission hardening pass: 162
server tests passed.
The Firestore emulator tests and Docker container startup remained CI gates and
were not run locally in that pass. Live IAM, logging policy, candidate startup and hosted inference
still need verification. See [the historical, unaudited review](https://github.com/freemocap/skellyspeak/blob/d16874e0a59ee00772ef0952751ac35d9e6e6d78/old/notes/SERVER-REVIEW.md) for findings,
remaining operational checks and supporting Google/OWASP guidance.

## Grouped-work admission module

`POST /v1/operations` uses account-scoped transactional claims after authenticated
per-item infrastructure admission and before spending reservation/provider dispatch.
The native hosted scheduler groups ready operations for this endpoint, saving
individual results as they arrive. Custom URL supports the grouped protocol after its connection check. Chat-completion and audio
routes do not use these claims; complete client/protocol integration before treating
duplicate protection as a property of every app request.

Each attempt identity is `<10-digit Unix submission seconds>-<32 lowercase hex>`.
It has a ten-minute submission window with 30 seconds of future clock tolerance.
The identity and server-computed canonical request fingerprint must remain unchanged
on duplicate delivery. An intentional retry has a new identity; clients must never
silently rewrite a rejected timestamp. Identical concurrent claims get one owner;
duplicates return state without an owner token or permission to dispatch. Reusing
an identity with a different fingerprint is a conflict. No result text is retained
in these admission receipts; duplicate success does not imply result recovery.

One account may hold 64 leases across server instances. Leases expire after five
minutes. Grouped execution enforces a 180-second work deadline and checks that a full
work deadline fits inside the lease after spending reservation, before dispatch.
No transaction remains open during inference. A known terminal outcome releases its
own slot; uncertain outcomes retain the slot until expiry. Lease expiry bounds our
coordination state; it cannot prove a remote provider stopped processing.

Receipts contain fingerprint, internal ownership token, state, expiry and a TTL
24 hours after the timestamp embedded in the identity. Deployment provisions and verifies Firestore TTL for
`work_attempts.ttl` before promoting the service. TTL is cleanup, not deduplication
correctness: the immutable submission window rejects the same expired identity even
if its receipt has been deleted. Account slot records contain at most 64 active
identities when claimed. No prompts, results, API keys or endpoint URLs are stored
or logged by this module. Existing daily admission must bound receipt creation per
item; batching must not debit it once per envelope.

Fake-ledger tests cover ownership, capacity, account scope, fingerprint conflicts,
uncertain outcomes, expiry and late/duplicate completion. The cross-process test in
`test_firestore.py` requires the explicitly enabled local emulator; it is not a
verified Cloud Run result. Grouped execution shields spending settlement and claim completion during cancellation.
Native client grouped integration is implemented; full live-provider and disconnect QA remains separate from these fake-ledger tests.

### Grouped HTTP contract

The authenticated POST body has exactly `version: 3` and `items` (1–8), capped at
1 MiB for the whole envelope. Each item has `operation_id` (32 lowercase hex),
`attempt_id` (the timestamped identity above) and `request` (the validated text-chat
payload). Operation and attempt identities must be unique within the envelope.
An item may request streaming with `deltas: true` for prose. Structured requests use the ordinary result event. Every text model ID goes unchanged
to OpenRouter; model names do not select a different provider. Audio is not encoded
in these JSON groups.

Responses are newline-delimited JSON (`application/x-ndjson`). Each item independently
produces an event containing its operation/attempt identities and one of:

- `type: frames`, with `offset` counting prior provider JSON frames and `frames`
  containing the original bounded payloads, including role/metadata/usage frames.
- `type: result`, with `response` containing the bounded provider JSON for ordinary
  requests. Streamed requests end with `stream_complete: true` and observed provider
  metadata/usage; native reconstructs the completion from its received frames.
- `type: duplicate`, with recorded `state`; no provider execution or result replay.
- `type: error`, with a fixed `code`, HTTP `status`, and bounded `retry_after` when
  supplied by admission. Raw exception and provider-error text are not exposed.

The last event is `type: complete` with `count`. Absence of an item result or this
marker cannot establish failure/no charge; the client must retain uncertain outcomes
without automatic replay. Results arrive in completion order through a bounded
one-event delivery channel. Provider requests execute concurrently, outside database
transactions. Each envelope consumes an authenticated infrastructure admission, plus
one per item (including duplicate checks); spending reservations occur only for
newly owned items. Unknown usage retains its reserved charge and lease until expiry.

Tests cover independent completion, duplicate delivery without extra inference or
spending, malformed groups, mixed success/failure and cancellation during inference.
No live deployment, native-client or full HTTP disconnect interoperability is claimed.

## Verified local emulator setup

The local Firestore suite passes all seven tests, including admission from separate
processes, using Google Cloud CLI 584.0.0 / Firestore emulator 1.22.0 and Homebrew
OpenJDK 21.0.12.1. The CLI is isolated under
`/private/tmp/skellyspeak-emulator-tools/google-cloud-sdk`; its config is under that
temporary directory. No cloud login or production credentials are needed.

For this machine, start the emulator with:

```sh
env PATH="/opt/homebrew/opt/openjdk@21/bin:$PATH" \
  CLOUDSDK_CONFIG=/private/tmp/skellyspeak-emulator-tools/config \
  CLOUDSDK_CORE_DISABLE_USAGE_REPORTING=true \
  CLOUDSDK_PYTHON="$PWD/server/.venv/bin/python" \
  /private/tmp/skellyspeak-emulator-tools/google-cloud-sdk/bin/gcloud \
  emulators firestore start --host-port=127.0.0.1:8787 --project=skellyspeak-local-test
```

Then, from the repository root:

```sh
FIRESTORE_EMULATOR_HOST=127.0.0.1:8787 SKELLYSPEAK_FIRESTORE_TEST=1 \
  server/.venv/bin/python -m pytest server/tests/integration/test_firestore.py -q
```

The tests reject any other emulator address and use disposable project IDs. The
emulator warns that Java 25 will be required by newer tooling; align Java/tooling
versions before upgrading CI. Temporary tool files may be removed by the OS.
This verifies database transactions, not the full native Custom URL/server/provider
path. That local HTTP integration and app QA remain pending.

## Local server with real providers

Local credentials and logs use owner-only permissions: directory/file modes on
POSIX, and protected current-user access-control lists on Windows. The Windows
dependency is installed by `uv sync` only on Windows. Private-file helpers reject
symlinks, junctions and file aliases; failed credential replacement preserves the
previous file and removes its temporary file. These development helpers are not
included in the hosted image.

After `uv sync`, copy the sample and put `OPENROUTER_API_KEY` and `GROQ_API_KEY`
in `server/.env`. The file is Git-ignored and is loaded automatically; no other
environment variables, database, emulator, or cloud credentials are needed.

```sh
cp development/.env.sample .env
uv run python app/main.py
```

Those commands are intended to run from `server/`. From the repository root the
equivalent launch command is:

```sh
uv run --project server python server/app/main.py
```

`python app/main.py` also works in an activated `server/.venv`. The launcher uses
disposable process-local storage, real OpenRouter/Groq HTTPS endpoints, and binds
the API to `127.0.0.1:8765`. Local data is cleared when it stops. Add `--check` to
validate `.env` without starting the API or contacting either provider.

Daily spending, inference-request and diagnostics-request limits are disabled by
default in this local launcher. Usage, reservations and settlement are still
recorded; provider refusals, concurrency/rate controls and explicit spending
pauses still apply. Add `--enforce-usage-limits` to test daily quota enforcement.
The admin overview labels disabled limits. Hosted enforcement is unchanged.

In a desktop development build from this checkout, start the local server, then
open Settings → AI access → Custom URL and click **Connect to local server**.
The native app reads this checkout's `session-token.txt`, saves it in the existing
credential store with the fixed `http://127.0.0.1:8765/v1` address, and checks the
connection. The token is not returned to the UI. Models retain their current
selections. If the server is stopped, the saved setup remains and the connection
check reports the failure; start the server and check again.

This shortcut is unavailable in release and mobile builds. It does not read
`session.json`, search the disk, accept an arbitrary file or destination, change
hosted sign-in, or disable server authentication. Moving the checkout requires
rebuilding the development app so its source location matches.

For other builds, set AI access to Custom URL manually with:

- API base URL: `http://127.0.0.1:8765/v1`
- Authentication: Bearer session token
- Token: the contents of `server/.local-server/session-token.txt`

In the separate Models section, example selections are:

- Standard model: `google/gemini-2.5-flash`
- Fast model: `google/gemini-2.5-flash-lite`
- Transcription model, when enabled: `whisper-large-v3`

A signed session token is generated for the local emulator account and stored
owner-only with its signing key in `server/.local-server/session.json`. The token
is mirrored to `session-token.txt` and reused across restarts. Local tokens last
ten years; hosted sessions retain their existing 30-day expiry. No Google
sign-in or unauthenticated bypass is used for this test. The launcher and private
files are not packaged into the production image. Real chat/audio calls incur
provider charges; configuration checks do not invoke inference.

## Deployment retention gate

The deployment helper enables and verifies the timestamp field `ttl` on
`auth_states`, `login_codes`, `admission`, `usage`, `global_usage`,
`devices`, `reservations` and `work_attempts` in database `(default)`.
It reads field configuration only, never document contents. Missing policies
are requested asynchronously, then checked for ACTIVE state for up to ten minutes.
A failure or pending activation stops deployment before traffic changes.
If activation takes longer, rerun Deploy server after policies become ACTIVE.
Expired records are deleted asynchronously; application expiry and duplicate
checks do not depend on deletion timing. Unresolved reservations intentionally
have no TTL until reconciliation; no TTL is added to accounts or spending controls.

The Cloud Build identity `skellyspeak-build` needs
`datastore.indexes.list`, `datastore.indexes.get`,
`datastore.indexes.update` in the project. Updates use --async and readiness
is checked through field configuration, so operation-list/get permissions are
not needed by this helper. Use a narrowly scoped custom role;
do not grant Owner/Editor or give these administration permissions to the
runtime identity. IAM is not modified by the deployment helper. A permissions
failure is reported as GCLOUD_COMMAND_FAILED, with no raw cloud output logged.

Guidance: [Google TTL configuration and permissions](https://firebase.google.com/docs/firestore/ttl).

### Installation registration ceiling

An account can register 100 installations. These are random persisted app IDs,
not hardware identities; resetting app data or using separate app configurations
can produce multiple records on one machine. Repeated check-ins update the same
record. Records have a 90-day TTL refreshed at check-in; asynchronous Firestore
TTL deletion means expired records can remain counted until cleanup completes.
New registrations at 50 or more emit a count-only warning, without account IDs,
installation IDs, platform, credentials or application content. The ceiling is
a storage bound, not proof of compromise or a substitute for account-level
request and spending controls. Existing registrations can still check in at the
ceiling. Registration is performed by `/v1/me` after authentication; a 409 there
can therefore follow successful authentication.

Grouped item failures emit `operation_failure` log records with the server-generated
request ID, zero-based item index, allowlisted error code and exception class, HTTP
status, optional bounded upstream status and a fixed `http`/`internal` category.
These records omit bodies, URLs, identities, exception messages and tracebacks.
The outer streaming request may return 200 while an item fails; inspect item
records by request ID when diagnosing grouped requests. A correlated `group_finished`
record reports item count, delivered outcomes, errors and whether iteration completed;
it does not prove receipt by the client. Local private logs preserve the same safe
correlation fields. Unknown-usage settlement retains its
conservative reservation without replacing an existing upstream error. Storage
settlement failures remain failures.

The local launcher writes all inherited process output and structured Python
logging into private `.local/logs/server-.../` files; see the repository README's
Development diagnostic coverage section for the full capture/redaction contract.
Use the logged process launcher for the emulator too. Normal server restarts preserve
`server/.local-server/session-token.txt`; the app keeps using its saved token. Tokens must never be printed in logs.

## Model validation and provider failures

Text model IDs are forwarded unchanged to OpenRouter by both grouped operations
and `/v1/chat/completions`, without a server model-name allowlist, special model
routing, schema rewriting or price filter. Providers determine availability and
parameter support. The speech contract is selected by requested audio output,
not the model name. Transcription model IDs are forwarded unchanged to Groq.
`ALLOWED_MODELS` is not read or required.

Spending admission remains separate from model availability. Reservations use
estimates: known text rates where available and a default estimate of $0.30 per
million input tokens and $2.50 per million output tokens for other text models.
These estimates are not sent as provider price constraints. Successful requests
settle the reported actual cost, including costs above the estimate, without
turning a successful provider response into an error or pausing the service.
Subsequent admissions use the corrected balance. In-flight requests can exceed
the remaining allowance; these reservations do not guarantee a hard cost ceiling.
Unconfirmed usage retains its reservation pending reconciliation.

`/v1/protocol` lists recommended bindings and reports
`accepts_other_text_models: true`; the list is not an allowlist. Updated clients
treat advertised text and transcription bindings as recommendations in Custom URL
connection checks, regardless of the advertised list.

Grouped provider refusals affect their own operation, allowing siblings to finish.
The existing error `code` carries `OPENROUTER_HTTP_<status>` or
`GROQ_HTTP_<status>` with service status 502. Updated native clients explain the
provider/status, including provider account failures versus SkellySpeak limits.
Raw provider error bodies are not echoed into the UI or database. Diagnostic logs
include bounded, redacted provider error bodies as described below.
Provider errors survive conservative settlement instead of becoming a generic
internal error. No automatic retry is added.

Source tests use controlled provider responses; deployment and a real hosted chat
are separate verification steps. Both server deployment and an app rebuild are
needed for the complete behavior and improved error messages.

Pre-audit model-routing verification (September 14, 2026): 259 server tests passed; seven Firestore
emulator tests skipped. Native suite: 348 passed, one ignored. Rust formatting,
Clippy with warnings denied, and diff whitespace checks passed. Container source
inclusion is tested; Docker image startup and real provider execution were not run.

## Reservation preparation and historical reconciliation

Grouped JSON serialization checks run before money is reserved. Invalid preparation
fails its item without a charge or retained lease. Structured schemas are forwarded
unchanged for provider validation. Once provider submission starts, unknown usage
retains the reservation as before.

Unresolved reservations do not expire; daily usage ledgers retain their 90-day TTL.
`uv run --project server python -m server.operations.reconcile settle` verifies the OpenRouter receipt before settlement. If daily
ledgers have already expired, reconciliation can explicitly finalize a reservation
only after its UTC day plus 91 days. It corrects any surviving historical aggregate,
never recreates a deleted one, marks the reservation `historical_finalization: true`,
and gives that settled record the normal retention TTL. Ordinary settlement still
refuses missing ledgers, as does reconciliation before the retention boundary.
Historical finalization records actual cost even when it exceeds the reservation estimate.
Groq outcomes or refusals without an OpenRouter generation ID remain manual
investigation work; this command does not infer a zero provider charge.

Audit repair verification (September 14, 2026): 272 server tests passed; seven
Firestore emulator tests skipped. The actual gcloud manifest sentinel check passed.
These checks do not deploy, test live provider billing, or exercise Cloud Run.

Transcription accounting retains the existing duration-based service allowance
estimate ($0.111/hour, minimum ten seconds). It does not discover model-specific
Groq pricing or guarantee a provider spending ceiling for arbitrary models.

## Verbose local runtime logs

The local launcher prints sanitized Python events to the terminal and flushes the
same events immediately to `.local/logs/server-.../server-logging.jsonl`. Startup
prints the selected directory; `SKELLYSPEAK_LOG_RUN_DIR` selects an explicit run
directory. Existing run files are never overwritten or automatically deleted.

Coverage includes Uvicorn startup/shutdown, request arrival,
response headers and full response completion, disconnects/cancellation, streamed
byte/chunk progress (at most once per five seconds while chunks arrive), provider
submission/status/duration, budget reservation/settlement, and grouped operation
start/claim/duplicate/completion/failure. There is no periodic idle heartbeat.
A response header status of 200 does not establish stream success;
inspect the provider, settlement, operation and full response events too.

Events correlate using a server-generated request ID, never client IDs. Safe
metadata includes fixed routes/providers, status codes, timings, counts, token
counts and monetary micro-units. Bodies, prompts, audio, model names, raw URLs,
query strings, headers, account/device IDs, credentials, arbitrary exception text
and tracebacks are excluded from local terminal and file output. Unknown Python
messages and stdout/stderr writes produce explicit redaction records. This is
operational instrumentation, not a dump of application objects or library payloads.

Restart a running local server to load changes (`npm run server:local` from the
repository root). Restarting clears disposable local data but preserves the
session token. To follow the file, use `tail -f` on the printed directory's
`server-logging.jsonl`. The normal terminal already shows those events live.

### Provider error response bodies

Provider HTTP refusals now emit `provider_error_response` with the provider,
status, server-generated request ID and `response_body`, in both terminal and
local JSONL logs. This includes failed transcription and streaming-chat HTTP
responses, and JSON responses containing a provider error despite HTTP 200.
Successful response bodies (transcripts and generated content) remain excluded.

Error bodies are limited to 16 KiB and five seconds of reading. JSON keeps error,
message, type, code, param, detail and errors fields; other fields are removed.
Text error bodies are retained with redaction. Known request string echoes,
credential patterns, email addresses, URLs, IPv4 addresses, UUIDs, long identifiers
and quoted values are redacted. This is best-effort free-text redaction, not a
guarantee that every novel provider message is free of identifying information.
Incomplete/oversized bodies use an explicit marker and unreadable/truncated flags
rather than unfiltered partial JSON. Body-read failures preserve the original
provider HTTP error. These logs cannot recover bodies discarded by earlier runs.

### Resetting local authentication

Normal restarts reuse the saved signing key and session token. To deliberately
invalidate the old token and issue a replacement, run
`npm run server:local -- --reset-session-token` from the repository root, then
copy `server/.local-server/session-token.txt` into the app once. The flag cannot
be combined with `--check`; configuration checks never create or rotate sessions.

The first launch after upgrading from the former per-launch token behavior needs
one final token replacement. Subsequent restarts retain it. Missing token mirror
files are repaired from `session.json`; corrupt or expired credentials fail with
an explicit reset instruction rather than silently changing the saved token.
The development session helper is excluded from hosted deployment images.

### Internal provider credential checks

Authenticated `GET /v1/protocol?verify_providers=true` probes the server's
OpenRouter `/key` and Groq `/models` endpoints independently. Default protocol
requests remain metadata-only. The same diagnostic authentication/admission limits
apply; probes have bounded responses, ten-second deadlines, and no redirects.
Results contain provider, accepted/rejected/unreachable/invalid-response state,
HTTP status, and duration. They reveal neither credentials nor account metadata.
These checks make no inference requests and do not establish selected-model or
billing availability. The UI displays each provider separately and will not mark
the aggregate custom connection healthy when a provider fails. Provider-error logs
retain the sanitized error body for investigation.


## Audio relay and client interpretation

The native service client uses `POST /v1/audio/speech` with exactly `model`,
prepared `text`, and nullable provider `language_code`. Native validates language
capability and builds accent instructions while retaining the unchanged source text
for display, alignment and cache identity. The server supplies its configured voice,
checks request bounds and estimates allowance from the actual submitted text.
Provider credentials remain on the service.

Audio protocol version 3 returns `{version, response, usage}`: `response` is the
bounded provider JSON with credentials removed. The server does not decode PCM,
assemble WAV, merge timestamps, derive words, or drop malformed alignment. Shared
native code performs these operations and retains explicit validation diagnostics.

With `Accept: application/x-ndjson`, speech starts with a version 3 `start` record
(format `pcm_s16le`, 24 kHz, mono). Each ordered `audio` record carries `response`
(original provider frame) and a safe receipt. The final `complete` carries `usage`.
The server keeps only bounded frame buffers. Native owns PCM accumulation, WAV
creation, recording-clock character timing and projection to the original source.
The existing player owns adaptive buffering and synchronized highlighting.
Disconnects preserve uncertain spending and cancel upstream delivery.

Transcription forwards the app's original mono 16-bit PCM WAV without decoding or
resampling. The server verifies its header, available sample bytes, rate and duration
for abuse prevention/accounting (8–192 kHz, up to 120 seconds). Native prepares
provider language codes, context and recognition options. The model binds only the
credential/destination and rate: `scribe_v2` uses its speech provider; other model
identifiers retain the existing transcription destination. No model substitution or
automatic audio retry occurs.

Transcription also returns `{version: 3, response, usage}`. Native derives optional
word timing and recognition confidence from full provider data before diagnostic
redaction/truncation. Invalid timing leaves transcript text intact with a recorded
validation reason. Language policy remains in native configuration and
`content/speech/speech-routing.yaml`. The generated server `speech_catalog.py`
contains only model provider/task availability, not language support tables.

Logs remain content-free. Successful client responses preserve content and unknown
JSON fields within explicit size/depth limits; they are never used as raw log records.
Credentials and echoed secrets are removed on the server. Protocol 3 requires
matching native and server builds; no compatibility conversion is provided.

Authenticated `/v1/protocol` includes `audio.routing` version 1 with
`available_models` and `accepts_custom_transcription_models`. This lists configured
credentials/voice profiles, not live provider health or model quality. Microphone
startup reads it without sending content or making an inference request, then
captures the compatible model before audio capture. Older services lacking this
contract are refused explicitly. Matching native/server builds are required for
the version 3 relay contract. No deployment is implicit in source edits.

Set `ELEVENLABS_API_KEY` and `ELEVENLABS_VOICE_ID` in the private local environment.
The public sample uses George (`JBFqnCBsd6RMkjVDRZzb`). Synthesis currently binds
`eleven_v3`; transcription uses the requested model. Missing provider credentials
fail before provider submission. The service-wide voice is explicit; it does not map persona
OpenAI voice names onto invented ElevenLabs equivalents. Native clients use their selected service connection for both audio tasks.
The voice catalog remains later work.

Audio allowance rates are estimates: 111,000 microdollars/hour for Whisper Large v3,
40,000 for Whisper Large v3 Turbo and 220,000 for Scribe transcription
(with a ten-second minimum) and 100 microdollars per Unicode code point of synthesis
source. Successful audio ledger rows mark `cost_basis=estimate`; response
`cost_micros` stays null when no actual charge is reported. This is not an invoice
or a hard provider cost ceiling. Interrupted submissions retain their reservation
as unknown without turning a successful, settled synthesis into an error.
No silent retries are added. Unknown ElevenLabs charges require investigation;
the OpenRouter receipt reconciliation tool cannot reconcile them.

`/v1/protocol` advertises audio version/provider/model/readiness. Explicit provider
checks probe ElevenLabs `/models` with `xi-api-key`; models/voices must be enabled
for the checks you use. They do not invoke inference. Normal local logs retain
provider, status and request correlation, not keys, source text or audio.

See [the setup and deployment guide](../docs/notes/audio-provider-setup.md) for
exact native Models settings, key placement, Secret Manager IAM, pinned version
selection and rotation. The Cloud Build source now binds `elevenlabs-api-key:1`
by default via an overridable substitution. This source has **not been deployed**.

## Owner administration

The source includes a browser panel at `/admin`, restricted to the Google-verified
email `info@freemocap.org`. This uses the existing Google client and callback with
separate browser-bound admin state and a one-hour HttpOnly session. App bearer
sessions do not grant admin access. Admin checks are independent of learner daily
quotas, so the panel can investigate and restore an exhausted account allowance.

The panel shows paginated users and custom spending exceptions, up to 90 UTC days
of ledger history, device registrations, recent reservations, service controls,
Cloud Logging events and audited administrative changes. Logs load on demand with
bounded pagination. Time-of-day heat maps summarize loaded request-arrival events;
geographic origin is not collected. Usage includes reservations and estimated
charges and must not be read as a provider invoice.

Account rows show masked email components, such as `l•••r@e•••d`; clicking the
address fetches its full identity through the existing owner-only account route.
Click again to hide it. Full emails and names are excluded from overview and live
snapshot payloads. Masking reduces incidental exposure; it is not anonymization.
User IDs are visible by default, masked as `google:123...678901` (first three
and last six subject digits, with an ellipsis); clicking reveals the full account
ID and clicking again conceals it. Short subjects are fully masked. Hosted IDs
are the existing `google:<subject>` identity, not email-derived identifiers.

Last sign-in is the user profile timestamp written by Google authentication.
Last inference request is the newest retained reservation creation timestamp,
including pending/failed attempts. Neither measures human interaction or proves
successful completion; missing request history is unknown. Settlement timestamps
are not activity timestamps. Reports use one UTC date captured at generation for
all daily and 90-day reads, while retaining the sequential-read disclosure.
The account table distinguishes the current UTC allowance day (reset at 00:00 UTC)
from rolling past-24-hour and past-seven-day usage. Rolling totals use reservation
creation timestamps through report generation, with settled amounts or outstanding
holds; settlement does not move usage to a later window. Reads cover at most 10,000
recent reservations per account; a window with additional matching records or
missing amounts displays Unknown instead of a partial total. The existing 90-day
column still covers the current and preceding 89 UTC dates. Two-line column
headings keep periods and USD units visible.

Accounts, request events, allowance intervals, daily account history, reservations
and change history offer sortable headers and column controls. Account and event
tables also search their loaded rows. Sorting/searching is limited to loaded
pages, not all retained records. Choices survive refreshes/live snapshots for the
open page and reset on reload. Numeric values sort numerically, with unknown
observations last in either direction. Request-event durations measure the named
phase; reservation Created → updated measures bookkeeping, not provider latency.

Changes require a review dialog, same-origin request, current revision and operation
UUID. Daily allowance resets preserve usage/history and shared counters; they
restore personal allowance through credit offsets. Session revocation invalidates
existing sessions but permits signing in again. Effective service overrides survive
redeployment in `service_controls/limits`; missing overrides use environment/code
defaults. The extended allowance is a preset copied into individual account limits.

For hosted access, open `<PUBLIC_BASE_URL>/admin` in a browser and choose **Sign in
with Google**, using `info@freemocap.org`. The panel is served by the same Cloud Run
service as the API; it does not require a separate admin deployment. This requires
a deployed revision containing the admin routes and the existing Google OAuth
callback configuration. Local admin launch tokens do not grant hosted access.

**Maximum registered accounts** controls new learner registrations, not concurrent
sessions or the number of sign-in attempts. The Cloud Build default `_MAX_USERS`
is 12 and supplies the runtime `MAX_USERS` environment variable. To change a live
service with the panel deployed, edit that field and review/apply the change.
The stored admin override takes precedence over `MAX_USERS`, including after a
redeployment. Existing accounts can still sign in when the ceiling is reached.

UI source is `ui/src/features/admin/entry.ts`, with the shared-token stylesheet at
`ui/src/styles/features/admin.css` and HTML source in `ui/tools/admin.html`.
Generated browser assets are explicitly included in the runtime image:

```sh
node ui/tools/admin-build.ts
node_modules/.bin/tsc --noEmit --strict --skipLibCheck --target es2022 --module esnext --moduleResolution bundler ui/src/features/admin/entry.ts ui/tools/admin-build.ts ui/tools/admin-preview.ts
node ui/tools/admin-build.ts --check
```

For a read-only browser review with synthetic identities and records, run
`node ui/tools/admin-preview.ts` after rebuilding the assets, then open
`http://127.0.0.1:4318/admin`. This loopback fixture is separate from the local
API, does not load credentials, and is not packaged in the hosted image.

Cloud log access requires the runtime identity to have `logging.logEntries.list`
(e.g. Logs Viewer). The panel reports missing permissions as an error. Admin audit
records have a 365-day TTL, included in deployment retention provisioning. This
source work does not authorize deployment, IAM changes or live account resets.
See [implementation, verification and remaining decisions](../docs/notes/server-admin-panel-2026-09-20.md).

### Installation history

Account details show the retained installation count and the newest check-ins first.
The service retains up to 100 installation metadata records per account. A new
installation at capacity atomically replaces the least recently seen record;
this reporting limit does not reject sign-in. Undated legacy entries are retired
first. Each check-in renews a 90-day inactivity expiry; Firestore TTL cleanup can lag.

**Remove selected** and **Clear installation history** use the usual review dialog,
account revision check and transactional audit receipt. Clearing removes all
installation records present when the change executes. These actions retire only
platform/version and first/last-seen metadata, preserving the account, sessions,
usage and allowances. An installation is recorded again on its next account check.
Use **Revoke sessions** separately when access should be invalidated.

### Local administration

`npm run server:local` builds the admin assets and serves the panel with the normal
local API. In a desktop development build, choose **Settings → AI access → Custom
URL → Open local admin**. Restart the desktop app after rebuilding native commands.
The button opens your browser without changing the app's selected connection or
passing credentials through the webview.

Google sign-in remains disabled for the local launcher. Local administration uses
this checkout's private `server/.local-server/admin-token.txt` to obtain a one-use,
60-second browser link. The resulting HttpOnly session lasts one hour. This access
is loopback-only and implemented under `development/`, excluded from the hosted
image. Hosted administration continues to require the verified owner Google account.

The panel reads the same in-memory accounts, usage, policies and audit records as
the local API. It reports the newest 10,000 sanitized log events from the current
run, with 500-event pages and request/error filters. Server restart clears these
records and invalidates local admin sessions. Existing JSONL log files remain on
disk. Local provider requests still use real keys and can incur charges.

### Live admin stream

Enable **Live** to open one cookie-authenticated `/admin/live` WebSocket. The server
pushes overview/chart/account snapshots after committed data changes or runtime
events, coalescing bursts; the browser does not poll HTTP endpoints. Chart/filter
changes are subscriptions sent over that connection. Draft limit fields retain
their original revision and are never replaced by stream updates.

Local commits wake the stream directly. Hosted connections also use Firestore
snapshot listeners for shared account, usage, admission and policy changes.
Live request events cover the connected server instance's newest 500 sanitized
events. **Load logs** stops Live and loads historical/cross-instance logs through
the existing reporting API. Expanded log details stay readable during updates.

The server checks the same admin identity and exact browser Origin before accepting
connections, rechecks session expiry/revocation during updates and at idle
heartbeats, limits connections/subscription changes, and releases listeners on
close. No administrative writes are accepted over this socket. Disconnects are
visible; enable Live again to reconnect. The refresh-frequency selector and
one-second HTTP polling are removed.


## Provider rate-limit retries

The grouped operation executor uses `app/inference/retry.py` for explicit upstream
429s, before any partial output. It keeps the original operation claim and retries
only the rejected item, with a fresh budget reservation and settlement for each
provider submission. Limits match native AI policy: at most three retries,
1/2/4-second delays plus up to 250ms jitter, honoring `Retry-After` within a
30-second cumulative wait budget. Cancellation, quota exhaustion, partial output,
transport uncertainty and non-429 failures stop retries. Successful siblings are
never replayed. Retry diagnostics survive final success and failure; rejected
rounds retain unknown billing instead of being silently refunded.

Audio retries belong to the native operation owner so there is no second server
loop multiplying requests. Authentication and key probes are not inference retries.
The raw chat compatibility endpoint remains one provider submission per request;
SkellySpeak text operations use the grouped endpoint. Custom servers own retries
inside their grouped attempts; the client cannot safely replay their durable IDs.
See [the audit and verification report](../docs/notes/ai-retry-audit-2026-09-21.md).
