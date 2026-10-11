# AI activity workspace completion and verification

Status: implemented source; verified results and acceptance limits below.
Plan: [AI activity workspace](ai-activity-workspace-plan-2026-10-11.md), §14.

## Completed

The shared checkout now contains the replacement DOM/SVG workspace, and the 21
obsolete files from the handoff have been removed. Native surface associations
connect conversation content to result-level hover hints. Native application
selection state now carries workspace runs and recorded revisions between windows.
The generic on-demand execution command remains deferred by the plan's §12 decision.

Review fixes cover observer feedback loops, axis-specific clipping, asynchronous
history paging and abandoned reads, restored revision selection, terminal attempt
spans, folded-group bulk expansion, historical animation, evidence-panel identity,
and avoiding fabricated optional-output availability.

Hover means downstream structural influence. It does not claim that a particular
attempt produced the displayed content. Associations come from native publication
owners and executable result bindings; no independent viewer topology was added.
Word-gloss targets attach to visible annotations and open word helpers, never the
unannotated source word. Product surfaces outside the tagged conversation owners
currently have no target. Hidden or absent targets are reported as not on screen.
Selection handoff is native application memory, not persistence across restarts.

## Automated verification in the actual PC checkout

- `npm test`: **296 files, 2,022 tests passed**, including the fast pretest gate.
- `npm run build`: passed application/test type checking and production bundling.
- `npm run check:fast`: passed after generated-contract refresh.
- `cargo test --manifest-path native/Cargo.toml --lib`: final state **940 passed,
  5 ignored**, including the shared audio publication classifier.
- `cargo clippy --manifest-path native/Cargo.toml --lib --tests -- -D warnings`:
  passed after final native changes.
- `cargo check --manifest-path native/Cargo.toml --bins`: passed.
- `npm run contracts:check`: passed after regeneration from Rust.
- `npm run docs:links`: passed (9 current documentation entry points).
- `git diff --check`: passed; Git reports existing CRLF normalization notices.

Full-suite warnings include jsdom's unimplemented canvas context; Vite reports
the existing large bundle warning. These did not fail the checks.

`npm run previews:check` **does not pass repo-wide**. It reports stale contracts
and deleted-module references in `ai-view-fixture`, `conversation-preview`,
`conversation-transitions-preview`, `localization-preview`, `practice-preview`,
`progress-preview`, `skill-levels-preview`, and `xp-payout-preview`. Those unrelated
preview migrations were not changed or hidden. No diagnostic names the new
`ai-workspace-preview.tsx`; that tool was also exercised directly in the browser.

## Browser fixture verification

The running Vite preview was checked in Chromium: across layout, 390px down
layout, expanded cards, folded-group collapse/expand, history drawer, light and
dark appearances. Measured SVG anchors and rendered pin centers differed by less
than 0.2px in the across layout. Enter opened the operation dialog; Escape closed
it and restored focus to the triggering card. Structure mode had zero animated
status dots, progress bars or edges.

Preview: `/tools/ai-workspace-preview.html`; query `orientation=down` selects the
phone arrangement and `theme=dark` selects dark appearance. Its data is synthetic.

These checks do **not** constitute a live native pop-out/provider smoke test.
Cross-window messaging and hover lifecycle are covered by automated tests;
native-window integration, coarse-pointer hardware, 150% reading scale and OS
reduced-motion acceptance were not manually exercised in a running native app.
The originally proposed React Flow comparison spike was not performed.

## Checkout handling

Worked in the shared checkout and preserved unrelated changes. No commit, push,
release, version bump, deployment or persisted workspace migration was made.


## Usability correction after refactor review (2026-10-11)

Implemented in the shared checkout; the earlier collapse/expand interaction
verification above describes the preceding UI, not the revised controls.

- Across columns share a vertical midpoint, and the canvas centers the graph in
  available space while retaining overflow scrolling.
- A single Compact toggle controls visual density. Compact nodes open inspection
  directly; expanded cards have a visible Details label and no shrink control.
- Deliberately opening an operation inspector immediately reads retained attempt
  evidence. Other callers retain their explicit Response disclosure. Inspection
  still validates engine/run/artifact/revision/node/attempt identity.
- History has Play/Stop for the loaded revisions, at 900ms per revision (not
  wall-clock execution timing). It stops at the last loaded revision. Earlier
  pages remain explicitly loadable; playback never dispatches native work.
- Running states animate while live or during replay. A stopped historical view
  remains still. Current conversation products and external stations are omitted
  from historical/structure views because they are not captured revision evidence.
- Control prerequisites remain visible at reduced opacity rather than appearing
  only on hover. Runtime topology and semantics are unchanged.

The column-centering test first failed (centers 109 versus 67) and passed after
layout correction. Activity/domain and affected conversation regressions passed:
17 files, 192 tests. The production build and fast gate passed. Repo-wide preview
types remain blocked by the previously listed stale previews, plus record-preview
using an obsolete gloss coverage value. Browser checks used synthetic data:
centered desktop flow, live edge animation, revision playback to completion, and
one-click compact-node inspection. No live provider/native-window smoke test was
performed. Raw request prompts and response bodies remain unavailable under the
existing retention policy; this UI change does not introduce payload storage.
