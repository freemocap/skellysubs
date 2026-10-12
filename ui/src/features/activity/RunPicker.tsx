import { useState } from 'react'
import { ToolbarIcon } from '../../components/controls/ToolbarIcon'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { useI18n } from '../../components/localization/i18n'
import type { NativeRunEntry, NativeRunPage, TranscriptionAttempt, TurnView } from '../../generated/contracts'
import { DISPOSITION_TONE, TONES, type Tone } from '../../domain/activity/graph-overlay'
import { listNativeWorkspaceRuns } from '../../platform/ipc/window'
import { nativeError } from '../../platform/ipc/workspace'

export type ElsewhereSelection = { kind: 'workspace'; entry: NativeRunEntry } | { kind: 'transcription'; id: string }

export function turnLabel(tr: ReturnType<typeof useI18n>, turn: TurnView): string {
  return `${turn.channel ?? tr('Graph')} · ${turn.nativeGraph?.run ?? turn.id}`
}

function toneCounts(turn: TurnView): { tone: Tone; count: number }[] {
  const nodes = turn.nativeGraph ? Object.values(turn.nativeGraph.nodes) : []
  return TONES.map(tone => ({ tone, count: nodes.filter(disposition => DISPOSITION_TONE[disposition] === tone).length })).filter(entry => entry.count > 0)
}

/// One list of every run the AI view can show: this conversation's exchanges and
/// runs elsewhere in the app. Workspace runs are listed on request only.
export function RunPicker({ turns, selectedTurn, following, elsewhere, hasOlder, loadingOlder, transcriptions, onFollow, onTurn, onElsewhere, onOlder }: {
  turns: TurnView[]
  selectedTurn: string | null
  following: boolean
  elsewhere: ElsewhereSelection | null
  hasOlder: boolean
  loadingOlder: boolean
  transcriptions: TranscriptionAttempt[]
  onFollow: () => void
  onTurn: (turnId: string) => void
  onElsewhere: (selection: ElsewhereSelection) => void
  onOlder: () => void
}) {
  const tr = useI18n()
  const [open, setOpen] = useState(false)
  const [page, setPage] = useState<NativeRunPage | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const loadRuns = (before: string | null) => {
    setBusy(true); setError(null)
    listNativeWorkspaceRuns(before)
      .then(next => setPage(current => before && current ? { runs: [...current.runs, ...next.runs], before: next.before } : next))
      .catch(cause => setError(nativeError(cause)))
      .finally(() => setBusy(false))
  }
  const current = elsewhere?.kind === 'workspace' ? `${elsewhere.entry.kind} · ${elsewhere.entry.run}`
    : elsewhere?.kind === 'transcription' ? (transcriptions.find(attempt => attempt.id === elsewhere.id)?.model ?? tr('Transcription'))
    : following ? tr('Follow live')
    : (() => { const turn = turns.find(item => item.id === selectedTurn); return turn ? turnLabel(tr, turn) : tr('Loading…') })()
  const choose = (action: () => void) => { action(); setOpen(false) }
  return <details className="ai-run-picker" open={open} onToggle={event => { if (event.target === event.currentTarget) setOpen(event.currentTarget.open) }}>
    <summary className="ai-chip" aria-label={tr('Choose a run: {value0}', { value0: current })}><span className="ai-run-current">{current}</span><ToolbarIcon name="chevron" size={13} /></summary>
    <div className="ai-run-menu">
      <p className="ai-run-group">{tr('This conversation')}</p>
      <button type="button" className="ai-run-option" aria-pressed={following && !elsewhere} onClick={() => choose(onFollow)} title={tr('Follow the newest exchange')}>{tr('Follow live')}</button>
      {turns.map(turn => <button key={turn.id} type="button" className="ai-run-option" aria-pressed={!following && !elsewhere && turn.id === selectedTurn} onClick={() => choose(() => onTurn(turn.id))}>
        <span className="ai-run-option-label">{turnLabel(tr, turn)}</span>
        <span className="ai-run-bar" aria-hidden="true">{toneCounts(turn).map(entry => <span key={entry.tone} data-tone={entry.tone} style={{ flexGrow: entry.count }} />)}</span>
      </button>)}
      {hasOlder && <button type="button" className="ai-run-option" disabled={loadingOlder} onClick={onOlder}>{loadingOlder ? tr('Loading…') : tr('Older')}</button>}
      <p className="ai-run-group">{tr('Elsewhere in the app')}</p>
      {!page && <button type="button" className="ai-run-option" disabled={busy} onClick={() => loadRuns(null)}>{busy ? tr('Loading…') : tr('Show runs from the rest of the app')}</button>}
      {page?.runs.map(entry => <button key={JSON.stringify([entry.engine, entry.run])} type="button" className="ai-run-option"
        aria-pressed={elsewhere?.kind === 'workspace' && elsewhere.entry.engine === entry.engine && elsewhere.entry.run === entry.run}
        onClick={() => choose(() => onElsewhere({ kind: 'workspace', entry }))}>{entry.kind} · {entry.run}</button>)}
      {page?.before && <button type="button" className="ai-run-option" disabled={busy} onClick={() => loadRuns(page.before)}>{tr('Older')}</button>}
      {page && <button type="button" className="ai-run-option" disabled={busy} onClick={() => loadRuns(null)}>{tr('Refresh')}</button>}
      {transcriptions.map(attempt => <button key={attempt.id} type="button" className="ai-run-option"
        aria-pressed={elsewhere?.kind === 'transcription' && elsewhere.id === attempt.id}
        onClick={() => choose(() => onElsewhere({ kind: 'transcription', id: attempt.id }))}>{attempt.model} · {attempt.state}</button>)}
      {error && <ErrorNotice as="p" error={error}>{error}</ErrorNotice>}
    </div>
  </details>
}
