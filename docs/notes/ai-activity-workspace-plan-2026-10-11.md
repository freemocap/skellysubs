# AI activity workspace: UX/UI refactor and downstream-surface hover

Date: 2026-10-11. Status: **proposal, revision 2**. External review was
incorporated on 2026-10-11 (eight points plus a Phase 0 finding, §14). Phases 1–2
(pure modules) are being implemented as new files only; everything else is a proposal.

Owner surface: `ui/src/features/activity/`. Governing contracts:
[AI graph foundations](ai-graph-foundations.md) (G1–G11),
[inspection export](ai-graph-inspection.md), [history reader](ai-graph-history.md),
[production integration](ai-graph-production-integration.md).
Approved visual prototype: <https://claude.ai/code/artifact/fce26ddd-be05-481a-a2a6-12276e524368>
(real partner-reply topology, specimen run data). §3 describes it, so this note stands alone.

Labels used throughout: **agreed** = user-approved, **proposed** = recommendation,
**verified** = checked against source on 2026-10-11, **unverified** = Phase 0 must
confirm, **open** = needs a decision.

---

## 1. Goal and non-goals

**Goal.** Replace the AI View with one stable workspace that:
1. tells a learner at a glance what the AI is doing;
2. lets a developer reach every authoritative fact about a node without changing views;
3. shows execution over revisions and attempts in the same workspace;
4. hints which app surfaces display results downstream of a hovered node, across
   the dock boundary and the pop-out window;
5. keeps `Render(Layout(P(G,R), U))`: no viewer-side topology, operation catalog,
   readiness inference or invented state (G1, G2, G11).

**Non-goals.** No ontology/execution/scheduling changes. No graph editing. No generic
node-demand command in v1 (deferred, §7 N3). No change to the Skills graph.
No actual-provenance claim (§8.1).

---

## 2. Current state (audit, verified 2026-10-10)

| File | Today | Problem |
|---|---|---|
| `AiView.tsx` | Header with title, summary, `AiExecutionSettings`, one chip per exchange, window actions; "Other AI activity" `<details>` holding transcription attempts + `WorkspaceGraphActivity` | Overloaded header; workspace runs hidden two levels deep |
| `NativeRunView.tsx` (`NativeGraphTimeline`) | One `<select>`: "Graph structure", "Follow live", "Revision N" | Structure and time mixed; history unreadable |
| `NativeGraph.tsx`, `GraphCanvas.tsx` (React Flow) | 216×32 pills; boundary inputs drawn as nodes | Resource/activation/attempts/types invisible; compositions not drawn as groups; 30-input fixture swamps 3 operations |
| `native-graph-layout.ts`, `graph-layout.ts` | Depth columns, fixed pitch | No variable sizes |
| `NativeInspector.tsx` | Side panel | `{node, operation}`, `{attempts, reasons}` as raw JSON |
| `NativeAttemptDetails.tsx` | Evidence on open + `recordBotInspection` | Correct behaviour; keep it. Presentation is an id `<select>` |
| `NativeRunControls.tsx` | Pause/Resume, Step, Cancel, Retry with eligibility checks | Keep logic verbatim (§3.7) |
| `WorkspaceGraphActivity.tsx` | `<select>` of workspace runs, 1 s polling | Hidden, inconsistent |
| `AiViewPanel.tsx` | Dock / expanded / pop-out / phone tray / screen | Keep mechanics |
| `app/tour/demo/AiDemo.tsx` | Imports `NativeGraph` (verified) | Must migrate in cleanup |
| `app/tour/tourTarget.ts`, `TourSpotlight.tsx` | Existing element-spotlight mechanism (verified to exist; not yet read) | Phase 0: evaluate reuse for §8.6 |

---

## 3. Target UX (agreed unless marked)

### 3.1 Principles (agreed)
- One stable workspace. No depth modes and no Graph/Timeline mode switch.
- Each operation has three sizes: bubble → card (in place) → popover (modal).
  **Expansion is always user-controlled; the default is all bubbles** (review decision).
- The timeline is the bottom scrubber, which expands in place into swimlanes.
- Learner-first wording; developer facts one step deeper, never removed.
- Captured inputs (`Input`, `Constant`, `Absent`) sit inside cards, never as wires.
- Type-colored ports and wires (FreeMoCap graph-lab direction).

### 3.2 Workspace anatomy
```
┌ header ──────────────────────────────────────────────────────────────────────┐
│ AI activity [Run picker ▾] status sentence…  [Run controls*] [⚙ Helpers] ⤓ ⧉ ✕ │
├ canvas (scrolls both axes; rendered at scale 1.0) ────────────────────────────┤
│ counts: 16 operations · 3 folded in Speech · 18 bindings · 38 captured inputs │
├ timeline drawer ─────────────────────────────────────────────────────────────┤
│ [Structure] ●──●──◉──… revision 31 · live [Live] [Show lanes]                 │
└──────────────────────────────────────────────────────────────────────────────┘
* only for a live conversation turn, see §3.7
```
- **Run picker** (proposed): one listbox. Groups: *This conversation* ("Follow
  newest" pinned on top, then turns newest-first) and *Elsewhere*.
  - Conversation rows can show kind, channel, a state bar from `nativeGraph.nodes`
    and retry count from `attempts`, because `TurnView` already carries those.
  - Workspace rows (`NativeRunEntry` = `{engine, run, artifact, kind}`, verified)
    show **kind + short run id only** in v1. No relative time, state bar or retry
    count unless N6 (§7) adds summary fields. **Never fetch every run's graph to
    decorate the picker.**
  - Transcription attempts keep their existing `model · state` row.
- **Status sentence** is built from projected facts only (§5).
- **Counts line** discloses folding (G11): totals, folded groups, binding count,
  captured-input count.
- **First-run hint** (dismissible, remembered per viewer, storage in try/catch).

### 3.3 Bubble (size 1)
- Pill. Rendered height ≥ 34px; ≥ 44px hit target on coarse pointers (padding, not scaling).
- State dot + humanized node id (`_`→space, first letter capitalized, nothing else).
  The tooltip and accessible description include the exact disposition and the node id.
- Tones (§5.2): done / running / waiting / runs-when-asked / held / problem / ended.
- "N tries" suffix when `attempts[node].length > 1`.
- Click/Enter → card.

### 3.4 Card (size 2)
Rows use geometry tokens (§6.3), top to bottom:
1. Header button: resource icon (Provider/Local), name, state word; mono line
   `provider · automatic`. Click/Enter → popover.
2. Wired input rows (`Source::Output`, incl. guard as a diamond): pin, consumer port
   name, type name in type color, "← producer.port".
3. Captured line: `captured · a, b +n · 1 constant · 1 absent`.
4. Output rows: `out port Type` with pin.
5. Content slot, reserved structurally for every `resource === 'Provider'` node:
   live preview (`AttemptPreview.live`, labelled "not used yet") or "No text yet".
   The reading serif is used only if an existing reading-scope signal identifies
   target-language content (**unverified**). Otherwise the UI font.
6. Footer: progress track + first reason/attempt sentence (§5.3) + shrink button.

### 3.5 Popover (size 3)
`DetailDialog`. Esc, the close button and a scrim click close it; focus is restored
to the card header.
1. Title, status chip.
2. Summary assembled from facts (resource, activation, attempts, disposition, reasons).
3. **What went in / What came out** (consumer port, type, source in words).
4. **Tries**: one chip per attempt (id, state word, observed revision span, open
   ends marked, §3.6), plus this node's lane.
5. **Why**: every reason for this node, **including terminal states** (Failed,
   Cancelled, Skipped, Unknown), with structured fault details (§5.3).
6. **For developers** (`<details>`): node id, exact disposition, operation contract,
   implementation id, resource/reuse/activation (artifact + effective `activation`);
   buttons:
   - **Open response evidence**: `NativeAttemptDetails` behaviour (exact-cut read,
     identity check, `recordBotInspection` after render). Opening the popover alone
     never reads evidence or credits.
   - **Copy implementation ID** (review decision).
   - **Copy snapshot JSON**.

### 3.6 Timeline drawer
- Collapsed: Structure toggle · revision stops · revision label · Live · Show lanes.
- Expanded: one lane per operation (members of an unfolded composition indented
  under the group row; a folded group shows one member-dots row).
- Selecting a revision swaps overlays only. Topology never changes (G2/G11).
- Axis is logical revision; copy says "Spaced by revision, not by seconds".
- **Completeness rules (review point 6):**
  - Loaded window = merged pages. Left of the oldest loaded frame there's an explicit
    "Not loaded" band with **Load earlier** (uses `before`).
  - Attempt spans whose start or end falls outside loaded frames render open-ended
    (no cap + ellipsis). The popover says "started before the loaded history".
  - Change markers diff the **whole frame**: `nodes`, `activation`, `attempts`
    (ids/states/acquisition), `reasons`, `paused`, `active`, `stepping`,
    `step_available`. A marker's tooltip lists the categories that changed.
- **Refresh rules:** history is read when the drawer opens or a past revision is
  selected. While the drawer is open and following Live, a new committed snapshot
  revision triggers one coalesced refresh of the newest page (max one in flight; a
  newer trigger replaces a queued one). Pages merge by revision (BigInt on decimal
  strings).
  - Same revision with different content → `throw`.
  - Engine/run/artifact mismatch → `throw` (as today).
  - Closing the drawer stops refresh.
- **Responsive (review point 8):** at widths < 860px the scrubber becomes a
  labelled range slider plus a revision select; lanes aren't shown in the drawer,
  and each node's lane appears in its popover instead.

### 3.7 Run controls and helper settings (review point 5)
- `NativeRunControls` logic moves unchanged into the header's run cluster. That
  covers `nativeExecutionAvailable`, `replacedBy`, cancelled/invalidated states,
  `active`, `retryable`, `globallyPaused`, `hold`, `step_available` and the busy/error
  handling. It's shown only when the selected run is a conversation turn **and**
  the revision is Live. Its existing tests are ported.
- `AiExecutionSettings` moves into a header "Helpers" popover, unchanged.

### 3.8 Modes (reuse `AiViewPanel`)
| Mode | Differences |
|---|---|
| Docked | Bubbles; drawer collapsed |
| Expanded / pop-out | Remembered expansion levels |
| Phone tray/screen | `down` orientation (one column, topological rows, depth indent). Card = full width. Popover = full-screen dialog. Timeline = slider form (§3.6) |

### 3.9 Visual system (proposed)
- Tokens only (`styles:check`). Follows the learner's appearance setting (review decision).
- Type hues: tokens `--graph-type-0…7` and `--graph-type-ink-0…7`, aliased to the
  data palette (`d-*`, `di-*`, `partner-mark`, `progress-mark`). The design-system
  README records the role "data identity, not status".
- `typeHue(contract)` is a deterministic string hash → 0..7; collisions are allowed
  and the type name is always printed. `typeDisplayName`: last `.` segment, kebab →
  PascalCase; the full contract is in the tooltip.
- Motion 120–200ms; all looping animation off under reduced motion.

---

## 4. Architecture

### 4.1 Data path
```
InspectionSnapshot | DefinitionSnapshot | AttemptPreview | RunHistory pages | NativeAttemptInspection
   │ (existing IPC; no new reads)
   ▼ domain/activity/graph-projection.ts   artifact → GraphView        (structure only)
   ▼ domain/activity/graph-overlay.ts      snapshot → Overlay          (facts only)
   ▼ domain/activity/graph-history.ts      pages → Timeline            (facts only)
   ▼ features/activity/workspace-layout.ts GraphView + U + geometry → Layout
   ▼ renderer
```
**Invariant (G2):** `GraphView` depends only on the artifact. `Layout` depends only
on `GraphView`, `U.levels`, `U.folded`, orientation and geometry tokens. Neither
reads dispositions, attempts, previews or reasons. With user-controlled expansion
and reserved columns (§6.3), no runtime fact can move a node.

### 4.2 Modules
| Path | Responsibility | Phase |
|---|---|---|
| `domain/activity/graph-types.ts` | `typeHue`, `typeDisplayName`, `humanizeId` | 1 |
| `domain/activity/graph-projection.ts` | Operations, ports, wired/captured inputs, outputs, data/guard/control edges, results, composition groups (§5.4), counts | 1 |
| `domain/activity/graph-overlay.ts` | Disposition tone/word, reason → structured sentence parts (binding-resolved), attempt summaries, footer fact, status-sentence facts | 1 |
| `domain/activity/graph-history.ts` | Revision compare, page merge, frame diff categories, lanes with open-ended spans | 1 |
| `domain/activity/graph-reach.ts` | Downstream results of a node (§8.3) | 1 |
| `features/activity/workspace-layout.ts` | Reserved-column layout, folding, port anchors, edge paths, `down` orientation | 2 |
| `features/activity/workspace/*` | Renderer, popover, drawer, picker | 3–6 |
| `state/navigation/graph-hover.ts` | Hover store + bridge | 8 |
| `components/graph-target/*` | `graphTarget` attributes + overlay | 8 |

The `domain/` modules return **data, not English**: e.g. a reason becomes
`{ kind: 'input', port, producer, producerPort, availability }`. Translation into
words happens in the renderer through `tr`, so the domain layer stays
localization-free (AGENTS.md: domain is React/Tauri-independent).

### 4.3 Renderer choice (open, resolved by an experiment)
Phase 2b spike (~0.5 d): build expansion, scrolling, keyboard traversal and phone
sizing with (a) plain DOM + SVG edge layer and (b) React Flow custom nodes, against
the 16-node partner graph and a 40-node fixture. Decide on: in-place resize without
jumps, focus order, rendered hit sizes, scroll behaviour, code size. The layout
module is renderer-independent either way.

### 4.4 Presentation state `U` and selection contract (review point 5)
| Field | Persistence |
|---|---|
| `levels: Record<nodeId, 'bubble' \| 'card'>` | localStorage per graph contract; default bubble |
| `folded: Record<compositionPrefix, boolean>` | Same; default folded |
| `popover: nodeId \| null` | Selection contract |
| `revision: 'live' \| 'structure' \| decimal` | Selection contract |
| `run` | Selection contract |
| `timelineOpen` | localStorage per mode |

**N5 (native contract change, required):** replace `AiViewSelection
{conversationId, turnId, operationKind}` with:
```ts
type AiRunSelection =
  | { kind: 'conversation'; conversationId: string; turnId: string | null /* null = follow newest */ }
  | { kind: 'workspace'; engine: string; run: string }
type AiViewSelection = { run: AiRunSelection | null; node: string | null /* raw node id */; revision: string | null /* null = live, 'structure', or decimal */ }
```
Today's `operationKind` holds prefixed values such as `node:…`/`input:…`
(verified in `NativeGraph`/`NativeInspector`). The new field holds raw node ids, and
boundary ports are not selectable. No compatibility path: callers (`inspectAi` in
`navigation.ts`, `TurnReplyHelp.tsx`) are updated. **Unverified:** whether native
persists the selection beyond process lifetime. If it does, add the store migration.

### 4.5 Performance
- `GraphView`/`Layout` memoized by `artifact_id` + `U`. Preview updates re-render
  only the owning card's content slot (selector keyed by node).
- Workspace run updates: keep the existing poll but only while that run is selected
  and visible (verified: 1 s poll today). Replace it if a notification exists
  (**unverified**).
- Acceptance budget (proposed): on the 40-node fixture a preview update re-renders
  one card, and expanding a card commits in < 16 ms on the dev machine.

---

## 5. Semantics mapping

### 5.1 Structure → marks
| Mark | Field |
|---|---|
| Operation | key of `definition.nodes` (flattened, incl. `prefix/child`) |
| Group frame / folded group bubble | key of `definition.compositions` (§5.4) |
| Resource icon | `operations[node.operation].resource` |
| On-request outline | `node.activation` (structure) / `snapshot.activation[node]` (effective, when a run is shown) |
| Wired input + wire | `node.inputs[p] = Output{node, port}` |
| Guard diamond + dotted wire | `node.guard = Output{…}` |
| Control dashed wire | `node.after[]` |
| Captured line | `Input` / `Constant` (always omitted) / `Absent` |
| Outputs | `operations[op].outputs` |
| Type name/hue | port `contract` |

### 5.2 Disposition → tone/word (exhaustive, `satisfies Record<Disposition, …>`)
done: Adopted "Done", Available "Ready to use" · running: Prepared "Starting",
Running "Working" · waiting: Waiting "Waiting", Ready "Queued" · ask: Unrequested
"Runs when asked" · held: Blocked "Blocked", Paused "Paused", Held "Held" ·
problem: Failed "Failed", Unknown "Outcome unknown" · ended: Skipped "Skipped",
Disabled "Off", Cancelled "Cancelled". No run → "Structure".
"Failed" never claims a cause. The fault, when disclosed, is shown separately.

### 5.3 Reasons (review point 2)
Rendered from the reason's own fields plus the artifact binding, nothing else:

| Variant | Sentence | Notes |
|---|---|---|
| `DemandRequired` | "Runs only when it is requested." | |
| `PolicyDisabled` | "Its activation policy is Disabled for this run." | No claim about which setting |
| `Paused` | "The run is paused." | |
| `Revoked` | "Its authority to finish was revoked." | No claim about why |
| `GuardFalse` | "Skipped because its guard was false." | |
| `GuardUnavailable` | "Waiting for its guard value." | |
| `Admission{r}` | "Waiting for a {provider/local} admission slot." | |
| `Input{port, producer, availability}` | "Input {port} ← {producer}.{producerPort}: {waiting/blocked/absent}." | `producerPort` is resolved from `node.inputs[port]` (or `guard`). If the binding isn't `Output` from that producer → `throw`. If producer is null: "Input {port}: {availability}." |
| `Control{parent, state}` | "Runs after {parent} ({state word})." | |
| `Attempt{id, state}` | "Attempt #{id}: {state word}." + fault block when `Failed` | |

Fault block: `code` (Known → mono code; Omitted → "code withheld ({Content/Unclassified})")
and `path` (same treatment). Omission markers are always visible.
Reasons are shown for **all** dispositions, including terminal ones.

### 5.4 Compositions (verified; review point 4)
Verified in `native/src/ai/graph/composition.rs`: `Registry::expand` inserts each
child node into the parent's `definition.nodes` as `"{prefix}/{child}"`, qualifies
its sources and `after`, and keeps `definition.compositions[prefix] = Boundary
{artifact, source, bindings}` as metadata. Boundaries aren't operations.

- **Membership** is derived from the exported boundary, not by parsing ids:
  `members(prefix, boundary) = { prefix + '/' + k for k in boundary.source.nodes }
  ∪ members(prefix + '/' + p2, boundary.source.compositions[p2])` recursively. Each
  member must exist in `definition.nodes`, otherwise `throw`. A node claimed by two
  groups → `throw`. Members of a nested group belong to the innermost group, which
  sits inside its parent group.
- **Unfolded group:** members are rendered as ordinary operations inside a group
  frame (header: layers icon, prefix, member count). Every operation renders exactly once.
- **Folded group (default):** one group bubble/card represents the members. It has
  **no state of its own**: it shows one dot per member, and the card lists members
  with their dispositions. Edges crossing the boundary are rerouted to the group
  and deduplicated by `(external endpoint node, external port, contract)`; the
  counts line discloses "n operations folded in {group}".
- **No N2 needed.** Membership is fully determined by exported metadata.

### 5.5 Status sentence facts
Computed in `graph-overlay.ts` as data: running node ids, tone counts, whether any
running node has a prior failed attempt, revision mode. The renderer phrases it.

---

## 6. Interaction, layout, accessibility

### 6.1 Input map
| Input | Effect |
|---|---|
| Click/Enter bubble | → card; focus to card header |
| Click/Enter card header | → popover |
| Shrink button or `-` | → bubble |
| Click/Enter group header | Fold/unfold group |
| Esc | Close popover; otherwise collapse focused card |
| Tab | Column-major visual order |
| Arrow keys | Move focus along wires to upstream/downstream neighbours (proposed) |
| Hover / focus on node | Canvas emphasis + downstream-surface hint (§8) |
| Stops / slider | Select revision |

### 6.2 Accessibility (review point 8)
- **No transform scaling.** The canvas renders at 1.0 and scrolls on both axes, so
  rendered hit sizes and learner typography are preserved. There's no zoom-out below 1.0.
- Coarse pointers: every control's rendered hit area ≥ 44px.
- Row geometry comes from tokens that follow the learner's spacing/type scale (§6.3).
- Real buttons; `DetailDialog` focus handling; status sentence in a polite live
  region updated on fact changes only (not tokens).
- Type inks meet 4.5:1 in both appearances (checked in acceptance). Color is never
  the only signal.

### 6.3 Layout rules (review point 3)
1. Depth = longest path over data, guard and control edges (cycle → `throw`).
   Folded groups are contracted before depth is computed.
2. **Reserved columns:** every column's width = card width (token). Bubbles are
   centered in it, so expanding or collapsing never moves any column horizontally.
3. **Top-aligned columns:** a column stacks from the top. Expanding a node moves
   only the nodes below it in the same column. Other columns don't move.
4. Order within a column: one left→right barycenter sweep over **structure only**,
   tie-break by node id. Deterministic, computed once per artifact + folding.
5. Node size = f(level, structure, geometry). The content slot is reserved for
   Provider nodes, so runtime previews never change size.
6. Geometry tokens (`--graph-row`, `--graph-header`, `--graph-captured`,
   `--graph-footer`, `--graph-slot`, `--graph-card-width`, gaps) are defined in CSS
   and resolved to px once per appearance/scale change with a measuring hook. The
   layout takes them as input, so CSS and anchors can't drift.
7. Port anchors = header + row index × row height (+ border). Folded-group ports =
   deduplicated crossing bindings.
8. **When fitting happens:** never automatically on expansion. On run/artifact
   change and on viewport resize, the canvas scrolls the first running node (or the
   origin) into view. Expanding a card scrolls it into view only if it overflows.
9. `down` orientation: one column in topological order, depth indent; same anchor rules.

---

## 7. Native / contract changes

| # | Change | Status |
|---|---|---|
| N1 | Generated result-name unions per graph contract (`ui/src/generated/graph-results.ts` from `export-contracts`) | Recommended for §8. Catches spelling drift only (review point 1) |
| N2 | Explicit composition membership export | **Not needed** (§5.4 verified) |
| N3 | Generic node-demand command | **Deferred** (review decision) |
| N4 | Implementation id → source path | Dropped; "Copy implementation ID" instead |
| N5 | `AiViewSelection` as a run union + raw node + revision (§4.4) | **Required** before Phase 7 |
| N6 | `NativeRunEntry` summary fields (last revision, tone counts, retry count) | Optional; picker works without it |

N5/N1 need native tests, Clippy (warnings denied) and `npm run contracts:check`.

---

## 8. Downstream-surface hover (renamed from "provenance"; review points 1 and 7)

### 8.1 What it claims
Hovering (or focusing) a node highlights **app surfaces associated with graph
results that are structurally downstream of that node** for the selected run. It
does **not** claim that this node's output produced the value currently displayed.
Guards, retries, retained/shared executions and revisions can break that link.
UI copy: "Shows where results that depend on this step appear." Actual
provenance would need an authoritative association between the displayed adoption
and its producing execution, a future native capability not in scope here.

**Revision behaviour:**
- **Live**: app highlights on.
- **Past revision**: no app highlights (the app shows current state, not that
  revision); canvas emphasis only, with the note "App highlights are available in Live".
- **Structure**: no app highlights.

### 8.2 Target identity (full key everywhere)
```ts
type GraphSurfaceKey =
  | { scope: 'conversation'; conversationId: string; turnId: string }
  | { scope: 'workspace'; engine: string; run: string }
type GraphHoverTarget = { key: GraphSurfaceKey; results: string[] }
```
DOM attributes from the helper (all are matched):
`data-graph-scope`, plus `data-graph-conversation` + `data-graph-turn` or
`data-graph-engine` + `data-graph-run`, plus `data-graph-results` (space-separated tokens).

### 8.3 Downstream results (pure)
`downstreamResults(view, node)`: forward closure over data + guard edges (control
edges carry no value and are excluded). The output is the set of `definition.results`
keys whose source is an `Output` from a node in the closure. Hovering a folded group
uses the union over its members.

### 8.4 Tagging
- `graphTarget(key, ...results)` returns the attribute object. With N1, `results`
  is typed by the generated union for that graph contract.
- Placement rule: on the element that renders the adopted value, beside the code
  that reads it.
- **Verification beyond spelling** (review point 1): each tagged component gets a
  component test asserting that the tagged element contains the rendered value of
  the field mapped to that result. Phase 0 confirms each field ↔ result pairing
  against native publication (`conversations/execution/graph_publication/`).
- Candidate sites (**unverified pairing**): partner text (`TargetMessage`), translation
  (`TranslationStatus`), brief + assistance (`ReplyHelp`), explanations
  (`ExplanationCards`), feedback (`MessageFeedback`), assessment/attribution
  (`MessageAssessmentRequests`, `MessageSkillAnalysis`), speech (`features/conversation/speech/*`),
  coach answer. Workspace scopes (reading, proposals) are tagged by their surfaces with
  `{engine, run}` keys.

### 8.5 Transport and lifecycle
- **Same window:** zustand store `{ target, seq }`.
- **Pop-out → main:** `emitGraphHover({ session, seq, target | null })` via
  `emitTo('main', …)`, following `sendReadingQuestion` (verified pattern).
  - `session` = UUID per AI-view instance; `seq` = monotonic integer.
  - Main keeps the highest `seq` per session and drops lower ones.
  - Throttle: trailing rAF. **A clear cancels any scheduled send** and is sent
    immediately with the next `seq`.
  - Payload validation throws on bad shape/size, like `onReadingQuestion`.
- **Main → pop-out reply:** `{ session, seq, matched, visible }`. Replies whose
  `seq` isn't the latest are ignored. The window label comes from the existing label
  source (**unverified** constant).
- **Clear triggers:** pointer leaves the node, blur, `visibilitychange: hidden`,
  `pagehide`, popover open, run change, revision leaves Live. Main also clears on
  `ai-window-changed` / `ai-view-docked` and when the pop-out window is destroyed.
  No timers.

### 8.6 Highlight rendering and visibility
- A portal overlay (fixed, `pointer-events: none`, below dialogs) draws one rect per
  visible matched element. Phase 0 evaluates reusing `TourSpotlight`/`tourTarget`.
- **Visible area** = element rect ∩ viewport ∩ every ancestor with `overflow` ≠
  visible (clip rects) ∩ (app area minus the docked panel rect). Zero area → not
  visible. Partial → draw the clipped rect.
- While a target is active: recompute on capture-phase `scroll`, `resize`, a
  `ResizeObserver` on matched elements, and a `MutationObserver` on the app root for
  tagged elements being added or removed. Observers detach when the target clears.
- Ids only in payloads; no content.

### 8.7 Required tests (review point 7)
A→B→leave sequence ends cleared; a delayed throttled send after clear is dropped;
run change clears; docking clears; window destruction clears; stale replies are
ignored; content hidden inside a scroller isn't highlighted; mounting a tagged
element during hover picks it up.

---

## 9. Phases and estimate (re-estimated)

| Phase | Work | Est. |
|---|---|---|
| 0 | Verify: field↔result pairings via publication code; reading-scope signal for previews; pop-out label; selection persistence; workspace-run notifications; `TourSpotlight` reuse; graph sizes of all workflows | 0.75 d |
| 1 | `graph-types`, `graph-projection` (incl. composition groups), `graph-overlay`, `graph-history`, `graph-reach` + tests | 1.5 d |
| 2 | `workspace-layout` + tests | 1 d |
| 2b | Renderer spike (DOM vs React Flow) and decision | 0.5 d |
| 3 | Renderer: bubbles, cards, groups, edges, tokens, CSS, preview-tool fixtures | 2 d |
| 4 | Popover incl. evidence/credit, reasons with fault blocks, tries | 1 d |
| 5 | Timeline drawer, lanes, paging, refresh, responsive slider | 1.5 d |
| 6 | Header: run picker, status sentence, counts, run controls, helpers popover | 1 d |
| 7 | N5 selection contract (native + UI), modes, restore across windows | 1 d |
| 8 | Downstream-surface hover (+ N1 if accepted), component tests per tag | 1.5 d |
| 9 | Port existing behavioural tests, then delete superseded files (incl. `AiDemo.tsx` migration); docs | 0.75 d |
| 10 | Gates + user acceptance | 0.5 d |

Total ≈ **13 days**, excluding N6.

## 10. Verification and gates (review point 8)
- **Invariant tests:** G2 (projection/layout independent of snapshots); G11 counts
  (expanded: each operation exactly once; folded: each member counted in exactly one
  disclosed group; each binding exactly one wire, or one deduplicated crossing with
  counted sources); exhaustive disposition and reason mappings over generated
  enums; BigInt revision order and merge conflicts throw; U-purity (no IPC from
  presentation changes beyond requested reads); credit only on the explicit
  evidence action.
- **Layout tests:** expansion moves no other column and only lower siblings;
  deterministic order; anchors equal token-derived geometry; folded rerouting dedupes.
- **Renderer tests** (Playwright against the preview tool): pin centers within 1px
  of anchors; coarse-pointer hit areas ≥ 44px; learner reading scale 150% renders
  without clipping; phone width layout.
- **Port before delete:** `NativeGraph.test`, `NativeRunView.test`,
  `NativeRunView.controls.test`, `NativeRunControls.test`, `NativeAttemptDetails.test`,
  `AiView.test`, `AiSplit.test`, `WorkspaceGraphActivity.test`, `graph-layout.test`,
  `native-graph-layout.test`, `InspectionContent.test`, `Tour.test` (for `AiDemo`).
- **Final gates:** `npm run check:fast`, affected UI regressions, `npm run build`.
  For N1/N5: native library tests, Clippy with warnings denied, `npm run contracts:check`.
- **Manual acceptance:** live streaming; failure + retry; on-demand node requested
  mid-run; guard skip; Structure ↔ Live; pop-out hover; phone; both appearances;
  reduced motion; keyboard only.

## 11. Risks
| Risk | Mitigation |
|---|---|
| Hover read as provenance | Wording §8.1; Live-only app highlights |
| Tag points at the wrong surface | Per-tag component tests; Phase 0 pairing check |
| Reserved card-width columns make bubble mode wide | Horizontal scroll; phone uses `down`; measure in spike |
| History refresh cost while live | Coalesced, one in flight, drawer-open only |
| Font scaling breaks anchors | Token-derived geometry measured at runtime |

## 12. Decisions (resolved 2026-10-11)
Defer generic demand (N3) · explicit expansion, default bubbles · result-level hover
with the §8.1 qualification · "Copy implementation ID" · follow appearance ·
renderer chosen by the Phase 2b spike.

## 13. Implementer constraints
TypeScript only; fail loudly (`throw` on unknown variants, missing ports, identity
mismatch); no viewer catalogs or name-prefix parsing; no compatibility shims;
tokens only; strings through `tr` (domain modules return data); changes left
uncommitted unless the user says otherwise.

## 14. Implementation status (2026-10-11)

**Phases 1–7 implemented. Phase 8 infrastructure is done; tagging is deferred.**

Built:
- Domain: `ui/src/domain/activity/graph-{types,projection,overlay,history,reach,surface}.ts`.
- Workspace: `ui/src/features/activity/workspace/` (`layout`, `geometry`, `wording`,
  `NodeItems`, `Workspace`, `RunWorkspace`, `TimelineDrawer`, `NodePopover`,
  `useWorkspaceState`, `useRunHistory`).
- Shell: `RunPicker`, `WorkspaceRunView`, `useGraphHoverPublisher`, a rewritten
  `AiView`, `state/navigation/graph-hover.ts`, `app/shell/GraphHoverOverlay.tsx`,
  and hover IPC in `platform/ipc/window.ts`.
- Styles: `workspace.css`, `timeline.css`, `shell/graph-hover.css`; `ai-view.css`
  is shell-only. Type hues and card geometry are tokens in `tokens.css`.
- Localization: all 8 locales.
- Review harness: `ui/tools/ai-workspace-preview.html`.

Renderer decision: an own DOM/SVG renderer instead of React Flow. The 2b spike
was decided by reasoning, not run. Reasons: in-place bubble/card sizing,
structure-only geometry (G2), and the down orientation are all simpler without a
canvas library.

Removed: `NativeGraph`, `GraphCanvas`, `graph-layout`, `native-graph-layout`,
`NativeInspector`, `AiSplit`, `NativeRunView`, `WorkspaceGraphActivity`, their
tests, and the `activity-preview` and `native-graph-preview` tools.

Verified in the isolated mirror:
- `tsc` is clean.
- The style check passes (64 sheets).
- `check-languages` passes (1524 messages per locale).
- The usage check reports no new removal candidates.
- Vitest: 290 files pass. The one failure is `admin-bundle.test`, which needs a
  built server bundle the mirror lacks.
- Visual check in Chromium: across, down (390px), cards, popover.

Deferred:
- **N5 (native selection contract).** The workspace-run and revision selections
  last only for the window session.
- **N3 (generic demand command).**
- **Phase 8 tagging.** Publication keys outputs by `(node_key, output_port)` in
  `conversation_graph_effects`, not by result name. `TurnView` doesn't expose
  which node/port produced a message, so no conversation element can be tagged
  without guessing. Needed: native must expose the effect's `{node, port}` per
  published message/assessment/audio. The UI then maps it to result names via
  `artifact.definition.results`, with no extra mapping layer, and calls
  `graphTargetAttributes`. Until then the overlay never matches, and the AI view
  shows "Not on screen".

Still to run on the device: `npm test`, `npm run check:fast`, `npm run build`.

## 15. Review log
2026-10-11 external review, all incorporated:
(1) influence vs provenance → §8.1;
(2) reason wording, consumer port, terminal reasons, faults → §5.3;
(3) layout consistency → §6.3, §3.1;
(4) compositions → §5.4 (verified, N2 dropped);
(5) controls and selection → §3.7, §4.4 N5;
(6) history completeness/refresh → §3.6;
(7) hover identity/lifecycle/visibility → §8.2, §8.5–8.7;
(8) accessibility and gates → §6.2, §3.6, §10;
Phase 0 finding on `NativeRunEntry` → §3.2, N6.
