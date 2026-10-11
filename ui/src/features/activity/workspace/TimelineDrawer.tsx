import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { useI18n } from '../../../components/localization/i18n'
import { messageKey } from '../../../domain/localization'
import type { ChangeCategory, LoadedHistory, Timeline } from '../../../domain/activity/graph-history'
import type { GraphView } from '../../../domain/activity/graph-projection'
import { humanizeId } from '../../../domain/activity/graph-types'
import { dispositionWord } from './wording'

/// What the workspace shows: the live snapshot, a recorded revision, or the
/// artifact's structure with no run overlaid.
export type RevisionSelection = { kind: 'live' } | { kind: 'structure' } | { kind: 'revision'; revision: string }

const CHANGE_WORD = {
  disposition: messageKey('states'), activation: messageKey('activation'), attempts: messageKey('attempts'), reasons: messageKey('reasons'),
  paused: messageKey('pause'), active: messageKey('activity'), stepping: messageKey('stepping'), step: messageKey('step availability'),
} as const satisfies Record<ChangeCategory, string>

/// Operations in dependency order, composition members after their group's
/// other members, for the swimlane rows.
function laneOrder(view: GraphView): string[] {
  const depth = new Map<string, number>()
  const parents = new Map<string, string[]>(view.ids.map(id => [id, []]))
  for (const edge of view.edges) {
    const to = edge.kind === 'data' ? edge.to.node : edge.to
    parents.get(to)!.push(edge.kind === 'control' ? edge.from : edge.from.node)
  }
  const of = (id: string): number => {
    const known = depth.get(id)
    if (known !== undefined) return known
    const value = Math.max(-1, ...parents.get(id)!.map(of)) + 1
    depth.set(id, value)
    return value
  }
  return [...view.ids].sort((a, b) => of(a) - of(b) || (a < b ? -1 : a > b ? 1 : 0))
}

function laneName(view: GraphView, id: string): string {
  const operation = view.operations[id]
  return operation.group ? `${humanizeId(operation.group.slice(operation.group.lastIndexOf('/') + 1))} / ${humanizeId(operation.name)}` : humanizeId(operation.name)
}

export function TimelineDrawer({ view, timeline, history, loading, error, selection, liveRevision, open, compact, onSelect, onToggle, onLoadEarlier, onRetry, onOpenNode }: {
  view: GraphView
  timeline: Timeline | null
  history: LoadedHistory | null
  loading: boolean
  error: string | null
  selection: RevisionSelection
  liveRevision: string | null
  open: boolean
  compact: boolean
  onSelect: (selection: RevisionSelection) => void
  onToggle: () => void
  onLoadEarlier: () => void
  onRetry: () => void
  onOpenNode: (node: string) => void
}) {
  const tr = useI18n()
  const selectedIndex = timeline && selection.kind === 'revision' ? timeline.revisions.indexOf(selection.revision) : -1
  const label = selection.kind === 'structure' ? tr('Map only')
    : selection.kind === 'live' ? (liveRevision === null ? tr('Live') : tr('Live · revision {value0}', { value0: liveRevision }))
    : tr('Revision {value0}', { value0: selection.revision })
  const changed = (index: number) => timeline!.changes[index].map(category => tr(CHANGE_WORD[category])).join(', ')
  return <section className="ai-timeline-drawer" data-open={open} data-compact={compact} aria-label={tr('Run history')}>
    <div className="ai-timeline-row">
      <div className="ai-timeline-start">
        <button type="button" className="ai-chip" aria-expanded={open} onClick={onToggle} title={tr('Every change this run went through, step by step')}>{open ? tr('Hide history') : tr('History')}</button>
        {open && history?.before != null && <button type="button" className="ai-chip" disabled={loading} onClick={onLoadEarlier}>{tr('Load earlier')}</button>}
      </div>
      <div className="ai-timeline-track">
        {open && timeline && !compact && <div className="ai-stops" style={{ gridTemplateColumns: `repeat(${timeline.revisions.length}, minmax(0, 1fr))` }}>
          {history?.before != null && <span className="ai-stops-unloaded" title={tr('Not loaded')} />}
          {timeline.revisions.map((revision, index) => <button key={revision} type="button" className="ai-stop" data-changed={timeline.changes[index].length > 0}
            aria-pressed={index === selectedIndex} aria-label={tr('Revision {value0}', { value0: revision })}
            title={timeline.changes[index].length ? tr('Revision {value0} · changed: {value1}', { value0: revision, value1: changed(index) }) : tr('Revision {value0}', { value0: revision })}
            onClick={() => onSelect({ kind: 'revision', revision })}><span /></button>)}
        </div>}
        {open && timeline && compact && <label className="ai-timeline-select">{tr('Revision')}
          <select className="field" value={selection.kind === 'revision' ? selection.revision : ''} onChange={event => onSelect({ kind: 'revision', revision: event.target.value })}>
            {selection.kind !== 'revision' && <option value="">{label}</option>}
            {timeline.revisions.map(revision => <option key={revision} value={revision}>{tr('Revision {value0}', { value0: revision })}</option>)}
          </select>
        </label>}
        {open && !timeline && loading && <span role="status" className="ai-muted">{tr('Loading…')}</span>}
      </div>
      <div className="ai-timeline-end">
        <span className="ai-timeline-label">{label}</span>
        <button type="button" className="ai-chip" aria-pressed={selection.kind === 'structure'} onClick={() => onSelect({ kind: 'structure' })} title={tr('The steps and their connections, with no run overlaid')}>{tr('Map only')}</button>
        <button type="button" className="ai-chip" aria-pressed={selection.kind === 'live'} disabled={liveRevision === null} onClick={() => onSelect({ kind: 'live' })}>{tr('Live')}</button>
      </div>
    </div>
    {error && <div className="ai-timeline-error"><ErrorNotice as="p" error={error}>{error}</ErrorNotice><button type="button" className="ai-chip" onClick={onRetry}>{tr('Retry')}</button></div>}
    {open && timeline && !compact && <div className="ai-lanes" role="list">
      {laneOrder(view).map(id => {
        const spans = timeline.attempts[id]
        return <div className="ai-lane" role="listitem" key={id}>
          <button type="button" className="ai-lane-name" onClick={() => onOpenNode(id)} title={id}>{laneName(view, id)}</button>
          <div className="ai-lane-cells" style={{ gridTemplateColumns: `repeat(${timeline.revisions.length}, minmax(0, 1fr))` }}>
            {selectedIndex >= 0 && <span className="ai-lane-cursor" style={{ gridColumn: `${selectedIndex + 1} / ${selectedIndex + 2}` }} />}
            {timeline.lanes[id].map(segment => {
              const attempts = spans.filter(span => span.fromIndex <= segment.toIndex && span.toIndex >= segment.fromIndex).map(span => `#${span.id}`)
              const first = timeline.revisions[segment.fromIndex], last = timeline.revisions[segment.toIndex]
              return <span key={segment.fromIndex} className="ai-lane-segment" data-tone={segment.tone} data-disposition={segment.disposition}
                data-open-start={segment.openStart || undefined} data-open-end={segment.openEnd || undefined}
                style={{ gridColumn: `${segment.fromIndex + 1} / ${segment.toIndex + 2}` }}
                title={`${dispositionWord(tr, segment.disposition)} · ${first === last ? first : `${first}–${last}`}${segment.openStart ? ` · ${tr('Started before the loaded history')}` : ''}`}>
                {(segment.tone === 'running' || segment.tone === 'problem') && attempts.join(' ')}
              </span>
            })}
          </div>
        </div>
      })}
    </div>}
  </section>
}
