# UI — React and TypeScript

The interface shown inside the native application's webview. Application code
lives in `src/`; tests shared across features, development tools, and static files
have separate homes.

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
| [src/app/](src/app/) | Startup, windows, shell, and application composition |
| [src/features/](src/features/) | Product features and their local components and tests |
| [src/components/](src/components/) | Reusable interface elements and hooks |
| [src/state/](src/state/) | State shared across features |
| [src/domain/](src/domain/) | Product logic independent of React and Tauri |
| [src/platform/](src/platform/) | Native bridge, browser/device integration, and diagnostics |
| [src/styles/](src/styles/) | Shared styles and design tokens |
| [src/generated/](src/generated/) | Rust-generated contracts and construct catalog |
| [tests/](tests/) | Cross-cutting architecture checks, shared setup, and mocks |
| [public/](public/) | Files served directly, including fonts and icons |
| [assets/](assets/) | Authored artwork and source assets |
| [tools/](tools/) | UI style tools and development previews |

Feature-specific tests stay beside their implementation. `tests/setup.ts`
registers shared mocks explicitly. Application modules must not import test
infrastructure.

## Responsibility groups

| Area | Subfolders and ownership |
| --- | --- |
| `app/` | `shell/`, `windows/`, `navigation/`, `shortcuts/`; startup and composition remain at the root |
| `features/conversation/` | `session/`, `messages/`, `composer/`, `reading/`, `coaching/`, `partners/`, `progress/`, `speech/`; composed by `ConversationPage.tsx` |
| `features/drill/` | Manual phrase/attempt composition, session/visit lifecycle and recording storage controls; uses shared reading components and `platform/audio/` |
| `features/skills/` | `overview/`, `evidence/`, `learner/`; composed by `SkillsPage.tsx` |
| `features/settings/` | `access/`, `appearance/`, `language/`, `workspace/`; composed by the settings dialog/modal |
| `features/languages/` | In-app language browser and source/model inspection |
| `features/activity/`, `features/startup/` | Small cohesive groups, kept flat |
| `components/` | `controls/`, `dialogs/`, `feedback/`, `reading/`, `learning/`, `media/`, `localization/`, `layout/`, `persistence/` |
| `state/` | `navigation/`, `session/`, `settings/`, `learning/`; shared initialization at the root |
| `domain/` | `access/`, `audio/`, `input/`, `conversation/`, `reading/`, `language/`, `localization/`, `learning/`, `rewards/`, `update/` |
| `domain/learning/` | `catalog/`, `evidence/`, `statistics/`, `rewards/` |
| `platform/` | `ipc/`, `audio/`, `diagnostics/`, `appearance/`, `updates/` |
| `styles/` | `foundations/`, `shell/`, `components/`, `features/conversation/`, `features/settings/`, `features/skills/`; ordered by `index.css` |

Conversation's partners, coaching and rewards still share its orchestration
and context; they are submodules of that feature. Place a component with its feature
unless it is shared across surfaces. Shared components may understand a domain but
must not import feature code or shared application state. Features remain independent
of one another; the app composes them. The architecture tests enforce these boundaries.

Keep pure conversation state and reading transformations separate from language
identity and interface localization. Translation dictionaries are in
`domain/localization/locales/`. Learning evidence and credit calculations belong
in `domain/learning/`; reward animation geometry belongs in `domain/rewards/`.
Browser recording and playback lifecycle belong in `platform/audio/`, and native
command adapters belong in `platform/ipc/`.

Keep the exact CSS cascade order in `styles/index.css`. Both style tools recurse
through the subfolders; validation rejects omitted, missing, or duplicate manifest
imports. Do not add ad hoc stylesheet imports to feature components.

This pass grouped existing modules without decomposing their implementations.
`src/types.ts` still mixes types from several areas; splitting it and the large
components is separate work. `src/vite-env.d.ts` provides frontend type declarations.

## Reply help and reading ownership

`features/conversation/composer/TurnReplyHelp.tsx` binds the automatic brief and
explicit grammar/suggestion requests to their accepted partner message. The
partner bubble's Analysis button opens `reading/PartnerMessageAnalysis.tsx`,
which uses the shared reading inspector for that reply's text and captured
language scope. It does not display learner assessment or consume a turn-help
result; the native reading executor retains its explanations. The learner
bubble's Coach button continues to open message feedback.
`domain/conversation/reply-help.ts` projects
native result and operation state; command acceptance is not generation completion.
Retries target one help kind, and opening saved help does not request new work.

Replies retain whole-passage translation and sound fields through `ReadingPassage`.
Words use shared `TargetText`/`ReadingHelp`, saved annotations and the existing
speech path. `MessageReadingScope` restores the message's captured language,
variety and explanation context. Native reading execution shares equivalent text
requests and retains bounded results in the workspace. Each surface owns only its
current display and cancellation; `app/ReadingTools.tsx` resets those surfaces with
the workspace session identity. No generated-result cache or pending-request map
lives in the reading provider.

Accepted and retained generated word meanings are queried locally on demand through
`platform/ipc/saved-reading.ts`, independently of mounted conversation providers.
The native query returns source-bound annotations in their captured scope; the
existing pure `domain/reading/saved-gloss-index.ts` projects exact words and
alternative saved meanings. A word covered by a partial saved passage needs no
new inference; a whole-passage request requires complete coverage. Unknown words
use the native executor, which may return a retained partial result. Explicit
retry bypasses saved lookup and requests fresh execution, preserving accepted
gloss spans. Lookup failures remain visible and never silently trigger inference.

Mounted accepted annotations still provide synchronous display. Generated results
are read asynchronously on an explicit action, including on another surface or
after reopening; they do not populate unrelated mounted cards automatically.
Translation and explanation reuse is also native-owned. The analysis view passes
the accepted translation when a quote exactly matches its owning message.

## Partner creation

The New partner dialog automatically creates the partner and opens its conversation
when generation succeeds. Its top Create action saves manually entered details.
If creation fails, generated details remain editable and Retry save retries only
persistence, without generating again. Closing during generation cancels ownership
and prevents a late result from creating a partner; closing is blocked while the
creation request is saving. Existing partner edits retain their autosave behavior.

## Commands

From the repository root, `npm ci` installs the UI workspace and development tools.
Run `npm test` or `npm run build` there. `npm run dev` starts frontend assets only;
use `npm run macos:dev` on macOS or `npm run tauri -- dev` on other desktop platforms
for the native app. Tests can be selected with `npm test -- tests/architecture`.

`npm run styles:check`, `npm run styles:dead`, and `npm run styles:prune` invoke
this folder's style tools. The repository-wide import graph and move tool remain
under root `tools/` because they resolve references across layers.

Run `npm run previews:check` from the repository root to type-check all design
previews against current production components. The conversation preview is at
`/tools/conversation-preview.html`; its sample controls do not perform native
operations or AI requests.

Native commands and persistence belong to [native](../native/); the hosted API
belongs to [server](../server/). `src/generated/contracts.ts` and
`src/generated/skill-catalogs/catalog.json` are generated from Rust with
`npm run contracts`; check them with `npm run contracts:check`. Do not edit
these generated files by hand.

Working notes and verification reports belong in [docs/notes/](../docs/notes/).

## Interface localization

Interface dictionaries live in `src/domain/localization/locales/`; interface
language is independent of the language being practised. English messages are
keys. Use `useI18n()` for rendered labels and accessibility text, `tr.number()`
and `tr.date()` for display formatting, and `messageKey()` for message keys
stored in metadata. Pass numeric interpolation values as numbers, not `String(n)`.
Keep native identities, provider field names, source quotations and persisted
assessment text unchanged. Proper names use `translatedName()` where applicable.

Skill catalog labels, descriptions and criteria are translated at display time.
The Rust-generated catalog remains the domain source; never edit it to localize
the UI. Its authored text must have entries in every interface dictionary.

`npm run build` checks catalog parity, placeholders, plural forms, duplicate
keys, rendered literal text (including expression/template accessibility text),
and generated skill-catalog coverage. `npm run localization:test` type-checks
and tests the checker. These checks cannot prove linguistic quality or discover
all dynamically supplied prose.

`npm run localization:audit` distinguishes direct translator calls from other
source/data references and reports unreferenced removal candidates. TypeScript
and JSON references match whole string values, not identifier or prose substrings. Add `-- --json`
for file references or `-- --check` to fail on candidates. The audit includes
active native/content sources and UI previews, excluding dictionaries, tests and
archived code. Other textual references are conservative evidence, not proof of
runtime reachability. Review dynamic callers before removing a candidate from
all seven dictionaries; the tool never deletes automatically. CI runs this audit.

For visual review, open `/tools/localization-preview.html` through the development
server. It renders production skill lists, descriptions, rewards and coach-term
controls with disposable sample data. Switch all seven locales and inspect narrow
layouts, especially German text wrapping and Arabic direction. It does not save
preferences or call native/AI services.

## Design system

[docs/design-system/](../docs/design-system/) is the published design system: the
brand book (`README.md`), tokens with usage notes (`tokens.json`), one folder per
shared component (guidelines and a preview) and the icon set. It is generated
from this package and must follow it, never lead it:

- Token **values**, `components/bundle.css` (this cascade), component previews and
  `assets/Icons/` are generated by `npm run design-system`
  ([ui/tools/design-system/build.ts](tools/design-system/build.ts)); previews are
  real components rendered statically by
  [previews.tsx](tools/design-system/previews.tsx).
- Token names and usage notes, `meta.sourceVars` (token → CSS variable), the
  brand book, component READMEs and the cover are written by hand.
- `npm run design-system:check` fails when a generated file is stale. A new shared
  component gets an entry in `previews.tsx` and a `components/<Name>/README.md`.

## Shared style vocabulary

The reviewed explorer is in `docs/notes/style-guide/`. The real-component fixture
is `tools/style-preview.html` (open through `npm run dev`); it uses local sample
state, without native calls or persistence.

| Need | Canonical basis |
| --- | --- |
| Text | `--type-meta` 11px, `--type-ui` 13px, `--type-body` 15px, `--type-title` 18px, `--type-reading` 20px, `--type-display` 24px |
| General surfaces | `--bg`, `--chrome`, `--sheet`, `--field`; text `--ink`, `--ink-2`, `--ink-3`; borders `--line`, `--line-soft` |
| Layout spacing | Ordered `--space-*` steps scale together using `--layout-scale` |
| Controls | `--control-height` 40px, `--control-height-sm` 32px; coarse pointers enforce 44px |
| Depth | `--shadow-sm/surface/floating/menu/drawer/input/recessed`, `--scrim`, `--scrim-strong` |
| Reading | Independent reading/script scale, word spacing, serif/script fonts and annotation roles |
| Interaction and status | `--interaction-ink/fill/fill-hover/tint`, `--focus-ring`; `--success-*`, `--warning-*`, `--danger-*` (each `-ink`, `-line`, `-tint`) |
| Learning domains | `--d-*` fills, `--di-*` inks, `--dm-*` muted fills, `--ink-on-domain` |

These roles describe different needs; equal current colors do not justify merging
status, learning or selected-state meanings. There are no aliases: one name per
role. Retired names (`--accent-*`, `--analysis-*`, `--card`, `--quiet`, `--correction-*`,
`--failure-ink`, `--status-*-tint`, the `--c-*` palette layer, `--text-*` sizes and
shell/paper aliases) should not return. `npm run styles:check` also rejects literal
line heights, spacing and border widths.

Shared rules live in `styles/components/{buttons,fields,panels,dialogs,popovers,
reading}.css`. Feature CSS owns composition and explicit feature variants.
`styles/shell/notices.css` owns fault/update notices; `styles/features/settings/
{forms,appearance}.css` owns Settings-specific controls. Apply depth within the
component base, rather than adding a global override sheet after all features.

Appearance settings (theme and palette) are learner preferences, validated/defaulted in Rust and
applied by `platform/appearance/useAppearance.ts`. Reading preferences retain their
own provider. The complete app restyle remains an incremental effort; the current
status and detailed language/reward exploration follow-up are recorded in
[the checkpoint](../docs/notes/style-system-refactor.md).

### Conversation styling

`styles/features/conversation/` separates workspace/mobile layout, messages,
composer/recording, reply help, header/pickers, startup, reading evidence and
inline rewards into named sheets. Each remains in the ordered manifest.
Shared activity indicators and waveform framing are under `styles/components/`.

The `.msg` base is shared by stream bubbles and partner excerpts.
`.msg.chat-message` explicitly adds stream padding, width caps, focus borders and
corners. Excerpt layout remains under `.reaction-excerpt .msg` in persona styles.
The `.field.composer-input` variant uses body-size input typography, independent
of reading size, while retaining language script scale. Picker variants distinguish the compact `.learning-line`
from labeled `.conversation-languages` rows. Preserve these real differences;
avoid identical declarations in the base and variant.

### Coaching, progress and inspection styling

- Conversation owns `coaching-dock.css`, `explanations.css`, `coach.css` and
  `start.css` for conversation choices and the prompt creator; shared compact actions live in component styles.
- `progress-map.css`, `progress-report.css` and `reward-presentation.css`
  separate maps, numeric reports and reward presentation.
- `features/skills/{learner-model,evidence}.css` owns learner tables and evidence.
- `features/conversation/speech-inspection.css` owns audio timeline inspection.
- Shared error details, YAML viewing and `inspection-action` controls live in
  `components/{errors,yaml-export,inspection-controls}.css`.

DetailDialog's `size="wide"` option gives YAML and learner reports the same wide
layout. The audio timeline retains its larger content-specific layout.
Inspection table-heading buttons intentionally override the common action base
with transparent backgrounds and no padding. Dock and floating reward cards
share their badge recipe, with explicit layout variants; their motion and
learning meaning remain separate from general surface depth.

### Settings and Skills composition

Settings styles separate `settings.css` (shell/navigation/scrolling/footer),
`access.css`, `audio.css`, `reset.css`, `forms.css` and `appearance.css`.
Joined route tabs and destructive reset confirmation are intentional variants.
Mobile shell rules own footer/scroll geometry; form rules own control spacing.

Skills styles separate `skills.css` (page, toolbar and list), `graph.css`
(nodes and scoped React Flow overrides), `inspector.css` and `review.css`.
Evidence and learner-model styles keep their existing feature owners.
The review inspector's recessed well remains distinct from the graph sheet.
Page-level font inheritance has low specificity so explicit control roles win.

### Style review tools

With `npm run dev` running, open `/tools/style-preview.html` for shared app
components or `/tools/detail-style-preview.html` for language and reward details.
Both use production CSS and sample data; they do not save workspace settings.
Experimental choices are labeled and scoped to the preview. Keep review-only
layout and controls under `ui/tools/`, and record decisions in `docs/notes/`.

Type-check these Vite-served tools separately from the application build:

```sh
npx tsc -p ui/tools/tsconfig.previews.json
```

### Conversation workspace review

The global target-language picker and header partner picker have separate scopes.
The voice composer, two-tab coach pane and responsive layout are documented in
[the UX rebuild note](../docs/notes/conversation-ux-rebuild.md).
Open `/tools/conversation-preview.html` through the existing development server
for a production-component layout fixture with sample data; it does not call AI,
record audio or save preferences.

Model selection has its own Settings section, owned by
`features/settings/models/SettingsModels.tsx`. Standard, Fast and Transcription
choices are shared across all access routes. Audio choices are global defaults;
the shared native speech resolver applies language preferences and model capabilities
and captures the actual model on each operation. `features/settings/access/` owns
route selection, credentials and endpoint configuration, with no model editor.


### Language membership and script size

The language browser's checkboxes save membership immediately; unchecking removes
the shortcut while preserving conversations and progress. The current language
cannot be removed until the learner switches languages. Clicking a language's
name inspects it without switching conversations. Details remain expanded and the
dialog scrolls; section grids adapt to the available width.

Script size is an independent per-language learner preference in this browser.
Rust validates and persists optional scriptScales overrides; missing entries use
the selected variety's content scalars.font_scale. ReadingProvider applies the
effective scale alongside the learner's overall reading size. The browser's
original-script examples preview the selected size. Default removes the override.

### Language fonts

Reading and interface roles share bundled script fallbacks for Arabic,
Devanagari, Malayalam and Simplified Chinese. Latin roles retain Newsreader and
IBM Plex; Noto Sans covers extended letters and reading aids. Font selection is
independent of script and learner size settings. See the
[font coverage and extension policy](public/fonts/README.md) before adding a new
language or changing a stack. The comparison fixture is
`/tools/fonts-preview.html`; it includes all current languages and marked Arabic.

### Language behavior boundary

Language-specific rules belong to content configuration and `domain/language/`.
Reading and feature components ask generic character capabilities; they must not
match language IDs or Unicode scripts themselves. The shared script helper uses
Unicode joining properties and grapheme segmentation to preserve connected text
across languages. Mixed-script runs and grapheme-safe language badge samples also
belong to that owner. `tests/architecture/language-handling.test.ts` guards against
script matchers escaping into runtime components. Translation dictionaries,
multilingual fixtures and bundled font asset declarations are data, not runtime
language branches.

### Shared target-language reading

`components/reading` owns word help across surfaces. Use `SavedGlossText` for
exact saved anchors, `AnnotatedText` for token arrays, and `TargetText` for known
target-language text without saved annotations. Unannotated words open compact
anchored help on a deliberate 300 ms mouse hover or a click/keyboard action.
Missing meanings are requested then and cached by source and language; rendering
and brief pointer passes never start inference. Source words remain mounted with
inherited sentence typography while helpers live outside the text flow. Use
`ReadingLanguageScope` for a source in a different language/variety. Selecting text does not open a global popup. Inspection is available through
explicit word-help actions. Use
`InspectText` beside action labels instead of nesting token buttons inside them.

Saved conversation annotations are projected by `ConversationReadingProvider`
into the shared `SavedReadingProvider` index. Known exact surface forms resolve
synchronously across reading surfaces and use `SavedGlossText`; a known word
must not enter loading state or request inference. Scope includes both languages
and varieties. Exact source annotations take precedence over saved alternatives.
Reply suggestions, frames and starters use this same renderer and reading
preferences, without a separate translation/pronunciation disclosure layout.

MessageTools owns toolbar rendering, overflow and accessibility for both speakers.
useMessageToolDefinitions owns Translate, Words, Pronunciation and Analysis in
that order for both speakers. Pronunciation uses written phonetic notation and
toggles saved reading aids; Play and Inspect recording retain distinct controls.
Learner Analysis opens its coaching feedback with the existing disclosure rules;
partner Analysis opens its explanation. Conversation messages
and their coach copies share TurnView, including the learner's edit, recording
and feedback behavior. Tour and preview message examples also use TargetMessage.

TargetMessage provides shared Analysis when reading lookup is enabled; owners
may supply a saved-message analysis handler. Reply suggestions and sentence
starters use its full toolbar. Starters declare template syntax through
ReadingTemplateContext, so underscore slots work even without surrounding spaces.
Blank help requests two or three completed sentences through the reading service,
preserving the original template and every character outside its slots. Outside
declared templates, only standalone underscore runs count as slots; ordinary
identifiers retain their word behavior. No request starts merely by rendering.

Word helpers include token read-aloud. `app/ReadingTools.tsx` injects the native
reading service and shared audio player; shared controls must not import feature
state. The inspector and AI activity expose retained reading-request receipts.
`/tools/reading-preview.html` is an offline fixture with no live speech or AI.

Target text shares playback highlighting through the same reading renderers.
Chat replies, reading sentences and word help, and Drill reference playback use
the shared audio clock; reference scrubbing follows its pointer position.
Retained timing is preferred, with estimated pacing when timing cannot be mapped.
The highlight is a reading aid, not pronunciation or assessment evidence.
`/tools/speech-follow-preview.html` exercises this with silent local audio,
including speed, seeking, reading aids and dialogs.

## Motion preferences

Settings → Appearance → Animation offers Follow system (default), Full animation
and Reduced motion. The choice is learner-owned and saved by native preferences.
`platform/appearance/motion.ts` resolves that choice with the operating system's
reduced-motion preference; `useAppearance` applies it at each window's root.
CSS uses `:where(:root[data-motion="reduced"])`; JavaScript uses `reducedMotion()`
or the reactive `useReducedMotion()` hook. Keep OS detection in that shared owner
so explicit choices work consistently. XP effects and sound settings retain their
own controls; Full animation does not enable effects the learner switched off.
