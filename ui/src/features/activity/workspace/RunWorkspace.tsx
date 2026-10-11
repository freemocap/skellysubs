import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useI18n } from '../../../components/localization/i18n'
import type { AttemptPreview, DefinitionSnapshot, InspectionSnapshot, RunHistory } from '../../../generated/graph-contracts'
import { buildTimeline } from '../../../domain/activity/graph-history'
import { overlayGraph, previewNode, type GraphOverlay } from '../../../domain/activity/graph-overlay'
import { projectGraph, type GraphView } from '../../../domain/activity/graph-projection'
import { downstreamResults, groupDownstreamResults } from '../../../domain/activity/graph-reach'
import type { Orientation } from './layout'
import { NodePopover } from './NodePopover'
import type { HoverTarget, Preview } from './NodeItems'
import { TimelineDrawer, type RevisionSelection } from './TimelineDrawer'
import { useRunHistory } from './useRunHistory'
import { useWorkspaceState } from './useWorkspaceState'
import { Workspace } from './Workspace'
import { nodeName } from './wording'

const NO_HISTORY = () => Promise.reject(new Error('This graph has no recorded run history.'))

/// Projection memoized by artifact identity: a re-delivered snapshot of the same
/// artifact keeps the same structural view.
function useProjection(snapshot: DefinitionSnapshot | InspectionSnapshot): GraphView {
  const cache = useRef<GraphView | null>(null)
  if (!cache.current || cache.current.artifactId !== snapshot.artifact_id) cache.current = projectGraph(snapshot)
  return cache.current
}

export interface RunWorkspaceProps {
  initialRevision?: RevisionSelection
  onRevision?: (revision: RevisionSelection) => void
  /// The live snapshot of a run, or a definition with no run.
  graph: DefinitionSnapshot | InspectionSnapshot
  preview: AttemptPreview | null
  loadHistory: ((before: string | null) => Promise<RunHistory>) | null
  /// Execution controls, shown only while the live revision is selected.
  controls: ReactNode
  orientation: Orientation
  compact: boolean
  popover: string | null
  onPopover: (node: string | null) => void
  /// Graph results downstream of the hovered step, or null when nothing is hovered
  /// or the view is not live.
  onHoverResults: (results: string[] | null) => void
}

/// The line that says what matters right now, built from the run's facts: the
/// steps working (with the text being written), the steps that need a request,
/// the steps with a problem, and how many are done.
function NowLine({ view, overlay, preview, selection, onFocus }: {
  view: GraphView; overlay: GraphOverlay | null; preview: Preview | null; selection: RevisionSelection; onFocus: (node: string) => void
}) {
  const tr = useI18n()
  if (selection.kind === 'structure' || !overlay) return <p className="ai-now"><span className="ai-now-word">{tr('Map of the steps, with no run overlaid.')}</span></p>
  if (selection.kind === 'revision') return <p className="ai-now"><span className="ai-now-word">{tr('Replaying revision {value0}.', { value0: selection.revision })}</span></p>
  const byTone = (tone: string) => view.ids.filter(id => overlay.nodes[id].tone === tone)
  const running = byTone('running'), ask = byTone('ask'), problem = byTone('problem'), held = byTone('held')
  const done = byTone('done').length
  const chip = (id: string, className: string) => <button key={id} type="button" className={`ai-now-chip ${className}`} onClick={() => onFocus(id)}>{nodeName(view, id)}</button>
  const streaming = preview && running.includes(preview.node) ? preview : null
  return <div className="ai-now">
    {running.length > 0 && <span className="ai-now-group"><span className="ai-now-word">{tr('Working now')}</span>{running.map(id => chip(id, 'is-running'))}</span>}
    {streaming && <span className="ai-now-stream" dir="auto">{streaming.text}<span className="ai-caret" aria-hidden="true" /></span>}
    {problem.length > 0 && <span className="ai-now-group"><span className="ai-now-word">{tr('Needs attention')}</span>{problem.map(id => chip(id, 'is-problem'))}</span>}
    {held.length > 0 && <span className="ai-now-group"><span className="ai-now-word">{tr('Held')}</span>{held.map(id => chip(id, 'is-held'))}</span>}
    {ask.length > 0 && <span className="ai-now-group"><span className="ai-now-word">{tr('Runs when you ask')}</span>{ask.map(id => chip(id, 'is-ask'))}</span>}
    {running.length === 0 && problem.length === 0 && <span className="ai-now-group"><span className="ai-now-word">{overlay.active ? tr('Nothing working right now') : tr('This run has finished')}</span></span>}
    <span className="ai-now-count">{tr('{value0} of {value1} steps done', { value0: done, value1: view.ids.length })}</span>
  </div>
}

export function RunWorkspace({ graph, preview, loadHistory, controls, orientation, compact, popover, onPopover, onHoverResults, initialRevision, onRevision }: RunWorkspaceProps) {
  const tr = useI18n()
  const view = useProjection(graph)
  const live = 'nodes' in graph ? graph : null
  const [selection, setSelection] = useState<RevisionSelection>(initialRevision ?? (live ? { kind: 'live' } : { kind: 'structure' }))
  const selectionKey = JSON.stringify(initialRevision ?? null)
  useEffect(() => { setSelection(initialRevision ?? (live ? { kind: 'live' } : { kind: 'structure' })) }, [selectionKey])
  const [historyOpen, setHistoryOpen] = useState(false)
  const [focus, setFocus] = useState<{ id: string; serial: number } | null>(null)
  const workspace = useWorkspaceState()
  const history = useRunHistory(live?.engine ?? '', live?.run ?? '', graph.artifact_id, loadHistory ?? NO_HISTORY,
    !!live && !!loadHistory && (historyOpen || selection.kind === 'revision'), selection.kind === 'live', live?.revision ?? '0')

  const shown: InspectionSnapshot | null = selection.kind === 'live' ? live
    : selection.kind === 'revision' ? history.history?.frames.find(frame => frame.revision === selection.revision) ?? null
    : null
  useEffect(() => {
    if (selection.kind === 'revision' && !shown && history.history?.before && !history.loading && !history.error) history.loadEarlier()
  }, [selection, shown, history.history, history.loading, history.error, history.loadEarlier])
  const overlay = useMemo(() => shown ? overlayGraph(view, shown) : null, [view, shown])
  const timeline = useMemo(() => history.history ? buildTimeline(view, history.history) : null, [view, history.history])
  const previewShown: Preview | null = useMemo(() => {
    if (selection.kind !== 'live' || !live || !preview) return null
    const node = previewNode(live, preview)
    return node ? { node, text: preview.capture.text, live: preview.live } : null
  }, [selection.kind, live, preview])

  const hover = (target: HoverTarget | null) => {
    if (!target || selection.kind !== 'live' || !live) { onHoverResults(null); return }
    onHoverResults(target.kind === 'group' ? groupDownstreamResults(view, target.id) : downstreamResults(view, target.id))
  }
  const select = (next: RevisionSelection) => { onHoverResults(null); setSelection(next); onRevision?.(next) }
  const focusNode = (node: string) => {
    const operation = view.operations[node]
    if (!operation) throw new Error(`Cannot focus an unknown step: ${node}`)
    if (workspace.state.levels[node] !== 'card') workspace.setLevel(node, 'card')
    setFocus(current => ({ id: node, serial: (current?.serial ?? 0) + 1 }))
  }
  const popoverNode = popover !== null && view.operations[popover] ? popover : null

  return <div className="ai-run" data-orientation={orientation}>
    <div className="ai-run-toolbar">
      <NowLine view={view} overlay={overlay} preview={previewShown} selection={selection} onFocus={focusNode} />
      <div className="ai-run-actions">
        <button type="button" className="ai-chip" onClick={() => workspace.setAll([...view.ids, ...Object.keys(view.groups)], 'bubble')}>{tr('Collapse all')}</button>
        <button type="button" className="ai-chip" onClick={() => workspace.setAll([...view.ids, ...Object.keys(view.groups)], 'card')}>{tr('Expand all')}</button>
        {(Object.keys(workspace.state.offsets).length > 0 || Object.keys(workspace.state.sizes).length > 0) && <button type="button" className="ai-chip" onClick={workspace.tidy}>{tr('Tidy up')}</button>}
        {selection.kind === 'live' && controls}
      </div>
    </div>
    {selection.kind === 'revision' && !shown && <p role="status">{tr(history.history && !history.history.before && !history.loading ? 'No recorded AI operations.' : 'Loading…')}</p>}
    <Workspace view={view} overlay={overlay} live={selection.kind === 'live'} preview={previewShown} state={workspace.state} orientation={orientation} focus={focus}
      onLevel={workspace.setLevel} onFold={workspace.setFolded} onLocalSteps={workspace.setLocalSteps} onOffset={workspace.setOffset} onBox={workspace.setBox}
      onOpen={node => { onHoverResults(null); onPopover(node) }} onHover={hover} />
    {live && loadHistory && <TimelineDrawer view={view} timeline={timeline} history={history.history} loading={history.loading} error={history.error}
      selection={selection} liveRevision={live.revision} open={historyOpen} compact={compact} onSelect={select}
      onToggle={() => setHistoryOpen(open => !open)} onLoadEarlier={history.loadEarlier} onRetry={history.retry} onOpenNode={onPopover} />}
    {popoverNode && <NodePopover view={view} node={popoverNode} overlay={overlay} snapshot={shown} timeline={timeline} preview={previewShown} onClose={() => onPopover(null)} />}
  </div>
}
