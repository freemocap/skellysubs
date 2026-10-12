# Working agreement

## Current stage

The repository is organized into peer `ui/`, `native/`, `server/`, `docs/`,
and `tools/` folders, plus `content/` for editable behavior data and schemas.
The root README and layer READMEs describe this layout.
Internal module organization is a separate follow-up. Deployment requires explicit authorization.

Historical `DESIGN.md` and `BUILD-PLAN.md` files are archived under `old/notes/`;
they are not current specifications. User guides live in `docs/docs-site`; retained
planning references live in `docs/notes/docs-site-archive`. Keep proposals, decisions, implemented
behavior, verification results and unresolved questions distinct.
Discuss ownership and user behavior before choosing storage,
frameworks, IPC or provider contracts. Do not present plans as working features.

## AI graph architecture

Before changing AI graph composition, scheduling, execution identity, result
sharing or graph visualization, read
[AI graph foundations](docs/notes/ai-graph-foundations.md) and the
[audit and staged plan](docs/notes/executable-ai-graphs-audit-and-plan-2026-10-08.md).
The agreed direction is architecture first: specify the shared ontology and
execution semantics, verify the core with domain-independent cases, then rebuild
and reconnect product workflows. Individual bugs and existing workflow branches
must not define the architecture. Preserve the distinction between agreed
constraints, draft contracts, implemented behavior and verified guarantees.

Execution and visualization must derive from the same validated executable graph
artifact through a short deterministic path. Do not add a separately maintained
viewer topology, operation catalog or interpretation of runtime semantics.
Generic serialization, generated contracts, layout and presentation adapters are
allowed; they must preserve semantic identities and authoritative runtime facts.
Changes to the ontology require explicit specification review and invariant tests;
temporary workflow-specific exceptions must not silently become architecture.

## Language-independent behavior

This rule applies throughout the app: UI, native code, server, prompts, matching,
analysis, tests and tooling. Solve language behavior at the highest shared level
that expresses the product requirement. Prefer Unicode properties, shared
algorithms and declared capabilities over language names, script block ranges,
locale checks, per-language branches or hand-maintained character lists.

Do not fix a cross-language problem with a language-specific patch. Establish a
single general policy and test representative scripts, canonical encodings and
meaningful counterexamples. Preserve source text and diagnostics; lossy matching
views must disclose what they ignore and must not be presented as pronunciation
or spelling evidence. Do not apply matching normalization to displayed content,
quotes, identifiers or unrelated operations.

Language/variety overrides in `content/languages/` are an absolute last resort:
first demonstrate why the shared policy or a general capability cannot express
the requirement, then document the specific exception and its evidence in
`docs/notes/`. Keep any justified override declarative in language configuration;
do not scatter language conditionals through application code. No override is
needed for Drill mark-insensitive matching. Existing exceptions are not precedent
for new ones; review them when their owning behavior is changed.

## UI organization

Follow the folder map in [ui/README.md](ui/README.md). Keep application code under
`ui/src/` and respect these ownership boundaries:

- `app/`: startup, composition, shell, windows, navigation gestures and shortcuts.
- `features/`: product surfaces. `conversation/` owns session flow, messages,
  composer, reading assistance, coaching, partners, progress/rewards and
  speech interaction in named subfolders. `skills/` groups overview, evidence and
  learner views; `settings/` groups access, language and workspace controls.
  `activity/` and `startup/` remain small, cohesive features.
- `components/`: reusable controls, dialogs, feedback, reading, learning, media,
  localization, layout and persistence components. Feature-specific components
  stay with their feature; shared components must not import feature/app state.
- `state/`: shared navigation, session, settings and learning stores/hooks;
  initialization stays at its root. Feature-local state stays with the feature.
- `domain/`: React/Tauri-independent rules. Conversation state, reading logic,
  language identity and interface localization are distinct owners. `learning/`
  groups catalog, evidence, statistics and reward calculations. `rewards/` holds
  presentation geometry, separate from learning evidence and credit calculations.
- `platform/`: IPC adapters, audio/browser APIs, diagnostics, appearance access
  and updates. Native calls belong in `ipc/`; browser recording and playback
  lifecycle belong in `audio/`. Domain code must not import platform code.
- `styles/`: foundations, shell, components and feature styles. Keep `index.css`
  as the single ordered manifest; preserve cascade order when moving sheets.
  Style tools must scan nested folders and reject unlisted or missing sheets.
- `generated/`: Rust-generated outputs; update their source/exporter, not the
  generated files by hand.

Appearance preferences are learner-owned: native validation/defaults live in
`native/src/configuration/appearance.rs`, settings controls in
`features/settings/appearance/`, and global theme/surface application in
`platform/appearance/useAppearance.ts`. Reading scale stays with the reading
provider. Generate contracts from Rust; do not duplicate appearance defaults.
Language script scale uses the existing `scalars.font_scale` configuration field
under `content/languages/` (`defaults.scalars` and variety `overrides.scalars`). Shared script defaults are `1.0`;
language/variety overrides tune script size independently of learner reading size.
Do not add a competing global Appearance setting for this multiplier.
Shared CSS owners are `components/{buttons,fields,panels,dialogs,popovers,reading}.css`;
shell notices and settings forms stay with their owners. Use `--type-meta/ui/body/
reading/title/display` for text roles and `--bg/chrome/sheet/field`, `--ink/ink-2/
ink-3`, `--line/line-soft` for general surfaces, text and borders. Do not restore
retired size or shell/paper aliases. Domain and status colors remain distinct.
Depth/shadow roles belong in each component base; do not recreate a global
appearance override sheet. Spacing scales independently of reading size and
control minimums; coarse pointers retain 44px controls.
Conversation CSS follows `workspace/messages/composer/reply-help/header/start/
reading-evidence/inline-rewards.css`. Shared activity and waveform styles live
under `components/`. Keep variants explicit: `.msg.chat-message` is the stream
bubble variant, `.reaction-excerpt .msg` is the partner excerpt layout, and
`.field.composer-input` adds reading typography to a shared field. Retained
contextual variants must explain their purpose and own only their differences.
Coaching dock and explanation cards have separate owners from conversation-start controls.
Conversation progress uses `progress-map`, `progress-report` and
`reward-presentation` sheets; speech timelines use `speech-inspection.css`.
Skills owns `learner-model.css` and `evidence.css`. Shared error details,
YAML export and `inspection-action` controls belong under component styles.
Settings separates shell/navigation, access-route controls, audio layout, reset
confirmation, forms and Appearance. Skills separates page/list layout, graph/vendor
styling, inspector layout and review-frame variants. Keep React Flow overrides
scoped in `features/skills/graph.css`; don't restore a high-specificity page-wide
font rule that defeats shared control typography. Deliberate variants (joined
access tabs, destructive confirmation, recessed inspector) retain distinct roles.
Use DetailDialog's `size="wide"` for wide data reports; retain content-specific
variants for genuinely different layouts such as audio timelines. Reward dock
cards and floating overlays share a badge base and document only their differences.
See [ui/README.md](ui/README.md) and the
[refactor checkpoint](docs/notes/style-system-refactor.md). Feature-specific
composition still needs subsequent passes. Pruning reports candidates and refuses
duplicate relocation; review source ownership before using write mode.

Colocate implementation tests with their owner. Cross-cutting tests, shared setup
and mocks belong in `ui/tests/`; UI-only tooling belongs in `ui/tools/`. Keep
`public/` for directly served files and `assets/` for authored source artwork.
Do not recreate flat catch-all folders or bypass the dependency checks with
re-export aliases at old paths. Add subfolders for concrete responsibilities,
not arbitrary file-count targets. The mixed `src/types.ts` and large components
need separate, deliberate decomposition; folder moves do not authorize behavior
changes or deletion of unused code.

## Native organization

Follow [native/README.md](native/README.md). Native code stays under `native/src/`:

- `application/`: startup, shared runtime state, command registration and background
  scheduling. `lib.rs` declares modules and exposes `run`; `main.rs` is the executable
  entry point. `startup.rs` owns Tauri setup/menu construction and the ordered
  command registration list; `state.rs` owns Application, its store guard and
  credential synchronization; `scheduler.rs` owns the dispatch loop. Group native
  command handlers in `commands/{workspace,connections,hosted,partners}.rs`.
  State/credential and persona-generation tests live under application/tests/,
  registered by their owning modules. Preserve lock scope, sign-in cancellation
  and command names when changing these seams.
- `conversations/`: conversation turns, execution, prompts, opening choices,
  revisions, reading-result publication and conversation exports.
  `execution/` separates admission, holds, connections, turns, snapshots, dispatch,
  publication, reading retries, speech and recovery. Keep its stable public entry
  points in mod.rs and behavior suites in execution/tests/; do not recombine them
  into a single implementation or test file. Preserve transaction boundaries when
  extending these modules.
- `partners/`: persona definitions, generation, generation receipts and reactions.
  Group persona definitions/prompts in `persona/` and generation/receipts in
  `generation/`; reactions remain in `partner_reaction.rs`.
- `learning/`: coaching, validated observations, learner state, progression,
  rewards and reward settings. Use `coaching/`, `learner/` and `rewards/` for those
  groups. Lesson generation is removed. Preserve durable turn ownership, one-time
  credit and evidence checks when changing coaching and rewards.
  Keep evidence independent of presentation metaphors.
- `speech/`: capture, recording commands, transcription receipts, audio inspection,
  fluency timing and speech cache. Use `recording/` for capture/commands/transcription,
  `analysis/` for inspection/fluency, and `cache.rs` for playback cache.
  Network adapters belong in `ai/transport/`.
- `ai/`: shared access, credentials, hosted connections, routing, admission, holds,
  refusal handling and provider transports. Use `connections/`, `hosted/`,
  `transport/` and `policy/` for these groups. Conversation execution stays with conversations.
  `transport/provider/` separates key verification, prose/structured payloads,
  HTTP requests and response decoding/validation, with matching test suites and
  shared local HTTP fixtures. Keep its public interface in mod.rs. Preserve
  route selection, input/response limits, redirect refusal, error redaction and
  the existing no-automatic-retry behavior across these seams.
- `storage/`: workspace ownership, database initialization, schemas, reset and workspace
  copy export. Feature-specific persistence remains with its feature; SQL alone is
  not a reason to move code here. Schema SQL files live in `schemas/`.
  `store/` separates workspace locking, schema validation, startup, snapshots and
  record creation. Its `commands/` coordinator owns the single command transaction,
  replay checks and receipt commit; partner, learning, conversation and assistance
  handlers borrow that transaction and must not commit independently. Keep store
  tests grouped by workspace, schema, preferences, transactions and lifecycle.
- `language/`: language lookup, emoji/Unicode handling, linguistics and local fixtures.
- `configuration/`: loading and validating editable source defaults from root
  `content/`. Editable data stays in `content/`; executable prompt assembly stays
  with the responsible native domain.
- `statistics/`, `diagnostics/`, `updates/`: usage reports, diagnostic records and
  application update discovery respectively. `bin/` holds the existing CLI entry points.

Keep unit tests and fixtures with their owning module. Use explicit module paths;
allow cross-domain access only with the visibility required by current callers.
Do not restore the old flat root through compatibility re-exports. `run` is the
intentional library entry-point export. `model.rs` is a temporary mixed-type file;
its decomposition, large-file splitting and deeper subfolders require a separate
agreed pass. Preserve existing runtime ownership, synchronization, native command
names and serialized contracts during folder-only work. Update exporters and the
UI command-registration test when their source locations move.

## Server organization

Follow [server/README.md](server/README.md). `server/app/` owns the hosted FastAPI
runtime; `tests/` owns server tests and shared setup; `development/` owns the local
launcher, local logging and example configuration; `operations/` owns service
usage and reconciliation tools; `deployment/` owns Cloud Build configuration,
deployment verification, retention provisioning and upload checks. Keep package
metadata, Dockerfile and .dockerignore at the server root.

Within `app/`, group authentication under `identity/`, request/work limits under
`admission/`, AI contracts/routing/streaming/audio under `inference/`, budget/quota
under `accounting/`, and account/request diagnostics under `diagnostics/`.
Keep main.py, config.py and the shared transactions.py helper at the app root.
Tests follow those subject groups, with integration, development, deployment and
operations suites; shared conftest.py stays at the tests root. Existing cross-test
fixture imports remain until a separately agreed extraction. The development
entry point is `server.development.launcher`; its Python logging module is `logs.py`.
These are whole-file groups; quota.py and main.py retain mixed responsibilities
pending later decomposition, and logging consolidation is separate future work.

Use explicit `server.<area>.<module>` Python imports and module entry points.
Update Docker copies, upload allowlists, CI and root launchers when files move.
Package only runtime code into the image. Preserve private local.env and
.local-server locations when moving launcher source. Whole-file organization
does not authorize splitting main.py, changing runtime behavior or running live
administrative/deployment commands. Deeper server subfolders need a separate review.

## Source file size

Prefer cohesive source files below 500 lines; smaller files are fine. 500–999
lines is a danger zone for responsibility review, and 1,000+ is almost always a
strong signal to split. These are guidelines, not hard limits or CI failure
thresholds. A cohesive file may legitimately be larger: preserve correctness and
readability rather than adding artificial boundaries or substantial complexity
solely to meet a count. Record the reason when keeping a flagged file large.
Include colocated tests in the count;
do not evade the policy by compressing formatting or moving everything into one
large test/helper file.

The [repository size inventory](docs/notes/large-file-inventory.md) records the
current scan baseline and separates authored source from generated/data/docs files.

Large-file cleanup now proceeds one original file at a time, starting with the
largest flagged source. Complete and verify each split, then check in before the
next file. Preserve behavior and tests; automated size reporting is still a
separate follow-up. Generated files, lockfiles and vendored data
need separate treatment in that check rather than manual splitting.

## Working notes

Put all new working notes, plans, investigations, verification reports, and
handoffs in `docs/notes/`, following [its README](docs/notes/README.md). Do not
scatter these files across the repository root, source folders, or documentation
website. Use descriptive names, group related topics when useful, and distinguish
proposals, agreed decisions, implemented behavior, and verification results.
Update existing notes for continuing work and mark superseded material clearly.
Maintained user/developer guides and module READMEs stay with their documentation
or code owners; promote settled information there when appropriate. Keep fixture
provenance and third-party license notices beside their data. Design proposals,
research recommendations and historical contract snapshots belong in `docs/notes/`,
even when they describe a single source module. Notes do not
become authoritative merely by being recorded. Leave historical `old/notes/`
material in the archive unless individual content is reviewed and adopted.

## Reference boundary

Everything in `old/` is deprecated reference material. It is not the active
application, documentation or a specification. Read it for concrete design
questions and intentionally evaluate useful architectural ideas. Record reviewed,
adopted principles in active documentation under `docs/`; reference material does
not become authoritative by association. Do not copy code, configuration, tests or documentation wholesale.
Any implementation reuse requires explicit review against the approved design
after implementation is authorized. Do not run or maintain the reference
application as part of design work.

Workspace history is durable from database format 45 onward. Application versions
and workspace format versions are independent. Any incompatible SQL or persisted
JSON change requires an explicit consecutive migration and a format version bump.
Keep released steps and historical format contracts unchanged, and retain the
complete chain from the supported baseline. No imports or conversions from the
archived application or pre-45 formats are required.

Migrations run under workspace ownership before ordinary startup recovery, with a
consistent recovery copy and one transaction for the entire chain. Validate source
identity, integrity and format, each step's output, and current product state before
commit. A failure must report its stage and retain useful redacted diagnostics;
never silently reset an unknown, damaged, newer or failed-to-migrate database.
Missing migration steps are errors. Reject pre-baseline files without modification.

New fields require meaningful defaults, explicit backfills or optional values.
Renames and restructuring preserve identities and relationships. Tightened
constraints require an explicit treatment of existing records. Deletion requires
a documented ownership review showing that information is preserved elsewhere,
reconstructible, or intentionally retired. Missing evidence is unknown, not zero.
Preserve learner evidence and earned awards, including their policy provenance;
formula changes must not silently rewrite historical credit or award it twice.
Derived projections may change only under an explicit product policy.

Do not use development resets as a substitute for migrations. Factory Reset remains
an explicit learner action and also removes local migration recovery copies. Steps
may change only transactional database data; files, audio and credentials require a
separately reviewed recovery protocol. Test individual steps, all supported starting
versions, failure rollback, repeated startup, preservation of history and equivalence
with fresh databases. See native/README.md and docs/notes/workspace-migrations.md.

## Collaboration

Edit the shared checkout in place. Do not overwrite it with whole files from a
stale clone. Before editing, read the current file and diff; preserve changes made
by another agent. A successful copy or clean type-check does not prove feature
wiring survived. Verify the affected user flow and regression tests.

When you ask the user a question and they do not answer it, stop and ask again.
Do not treat a non-response as permission to proceed on your own judgment.

Keep communication concrete and concise. Product UI uses plain functional labels:
no marketing copy, slogans, saccharine encouragement or filler. This rule governs
text. Reward presentation (tiered sound, animation and haptics defined in the
coaching plan's game layer) is required product behaviour, not decoration to strip. Settings must be
compact, with related inputs grouped and secondary details collapsed. Continue authorized design work and flag
meaningful decisions. Ask for user checks only when there is a specific artifact
to review. Distinguish design review, source implementation, automated verification
and a running application. Explain exactly what is ready to inspect.

Keep product design finite and specific to SkellySpeak. Do not reopen settled
platform, AI access or on-device/no-sync decisions through generic questionnaires.
Defer nonblocking details to focused design passes. Learning feedback and rich local
statistics deserve focused review before technical contracts; avoid repeated approval
questions for routine details.

Statistics are dense, mechanical scientific reports: numeric tables, distributions
and time series, organized by global usage, selected language and conversation
partner. No conversational or motivational statistical summaries. Performance-based
guidance belongs only in the language assessment area or separate coaching.

Keep domain records and metrics independent of visualization metaphors. Gardens,
flowers and alternative views render the same underlying data; renderer geometry,
styling and random seeds belong to presentation configuration. Evaluate geometric
Vibe matching through reusable embeddings and emoji references without presuming
a generative LLM call per observation.

## Coaching plan and research

Coaching, learner-model, new-chat and game-layer work follows
`docs/notes/docs-site-archive/coaching-plan.md`, executed per
`docs/notes/docs-site-archive/coaching-work-plan.md`, with the seams between work areas in
`docs/notes/docs-site-archive/coaching-contracts.md`. Cite research through
`references.bib` keys at the repo root (`sources: [key]` in YAML, `// [@key]` in
code, `[@key]` in docs). When research informs a change, add its entry with `url`,
`review` and `claim` in the same change.

## Git

**Do not create a commit unless the user explicitly instructs you to commit.**
Each such instruction authorizes exactly one commit, for the specified work,
and is consumed when that commit is created. Every subsequent commit requires
a new, explicit instruction. There is no standing or recurring commit permission.
Requests to implement, fix, finish, check in later, push, or release work do not
implicitly authorize a commit. Prior commit instructions do not carry forward.
Leave completed changes uncommitted unless explicitly told to commit; this also
applies to changes to these instructions. Do not bypass this rule with automatic
commits, merge commits, cherry-picks, amendments, or scripts that create commits.

Agents may stage changes, create and switch branches, push commits to the
configured remote as part of authorized work. These permissions do not authorize
creating a commit. Version bumps and release tags belong to the user: do not
change release versions, create or push release tags, or invoke release scripts
unless the user explicitly delegates that specific action in the current request.
Fixing CI or preparing work for release does not authorize those actions. Run
relevant checks before any explicitly authorized commit, and summarize Git
operations performed.

Inspect the working tree first and preserve unrelated user changes. Do not
include unrelated changes in commits. Destructive operations (including force
pushes, discarding changes, deleting branches or tags, and rewriting published
history) require explicit user authorization. Do not change Git configuration
without explicit authorization. Publishing packages and deploying applications
require explicit authorization; permission to push commits or tags alone does
not authorize triggering a known publishing or deployment workflow.

## Response information and observability

Preserve response information by default for observability and transparency, on
both success and failure. Remove credentials and content (prompts, messages,
transcripts, audio and echoed request content); do not replace the entire response
or error with a generic category to achieve redaction. Preserve non-content
metadata, including provider/service request IDs, provider error codes and redacted
reasons, requested/actual models, finish reasons, usage and billing provenance,
timing, retry/rate-limit information and validation stage/path/expected shape.
Keep validated partial metadata even when content decoding or publication fails.

Carry this information through adapters, server responses, native models,
persistence and diagnostic views. A concise UI summary may accompany expandable
technical details, but must not become the only retained representation. Preserve
additional non-sensitive provider metadata in a bounded structured form rather
than silently dropping fields because the display model does not use them.
Unknown fields need explicit sensitivity handling; do not blindly persist raw
bodies, headers, URLs or arbitrary exception strings. Mark redaction, truncation,
unreadable data and omitted fields explicitly so absent information is explainable.
Never confuse estimated allowance with actual cost or infer billing from HTTP
status. Tests must verify useful information survives as well as verifying that
credentials and content are removed.

## Quality

Fail on errors; do not substitute warnings or silent fallbacks. Keep documentation
about the active design and actionable questions. For UI work, review applicable
current guidance; the archived
`old/notes/ui-guidelines.md` is historical reference, not an active specification. Use TypeScript for frontend tooling. Run the relevant checks in README.md;
do not run archived application workflows.

Run `npm run check:fast` from the repository root by default at meaningful
implementation checkpoints, especially after changing UI text, translation
catalogs, styles, diagnostic policy or Rust source. During longer work, run it
after a coherent batch of edits so cheap failures are caught early; do not run
it after every individual edit or repeatedly without relevant changes.

For code, test or content changes, require a passing fast check on the final
state before declaring work ready, handing it back, or performing an authorized
commit or push. Fix failures and rerun after corrections. Review unused
translation candidates for dynamic callers before removing keys; do not weaken
checks or add dummy references to obtain a pass. Report any blocked check and
its cause explicitly; do not describe unverified work as passing.

The fast gate checks formatting, localization sources and usage, diagnostic
policy, styles and validation tooling tests/types. It does not run the full UI
suite or type-check application/test source. Also run affected regression tests
and `npm run build` for UI code or test changes before handoff, plus other relevant
README checks. Root `npm test` already includes the fast gate; a passing run on
the same final state satisfies that requirement without a duplicate invocation.
Documentation-only changes need applicable documentation checks, not the full
application suite.

When fixing CI, inspect every failed job on the current commit, not just the fast
gate or first failure. Reproduce and verify each failed check and run downstream
checks that earlier failures skipped where the local platform supports them.
For native changes, run the README's Clippy and native test commands; formatting
alone does not cover native validation. Report local passes and hosted CI status
separately, including any remaining unverified jobs.
