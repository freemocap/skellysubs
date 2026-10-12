import { useSettingsStore } from '../../state/settings/settings'
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { AiExecutionSettings } from '../../components/controls/AiExecutionSettings'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { OriginalContent } from '../../components/feedback/OriginalContent'
import { ResponseDetails } from '../../components/feedback/ResponseDetails'
import { useI18n } from '../../components/localization/i18n'
import type { GraphSurfaceKey } from '../../domain/activity/graph-surface'
import { getAiViewSelection, readGraphHistory, setAiViewSelection } from '../../platform/ipc/window'
import { nativeError } from '../../platform/ipc/workspace'
import { useNavigationStore } from '../../state/navigation/navigation'
import { useExecutionPreferences } from '../../state/settings/useExecutionPreferences'
import type { AiRevisionSelection } from '../../generated/contracts'
import { NativeRunControls } from './NativeRunControls'
import { RunPicker, type ElsewhereSelection } from './RunPicker'
import { useConversationActivity } from './useConversationActivity'
import { useGraphHoverPublisher } from './useGraphHoverPublisher'
import { WorkspaceRunView } from './WorkspaceRunView'
import { RunWorkspace } from './workspace/RunWorkspace'
import type { StationView } from './workspace/NodeItems'
import { turnStations } from '../../domain/conversation/turn-stations'

/// `screen` fills the phone; `tray` sits above the recording panel. Both stack
/// the graph down the screen.
export type AiViewMode = 'docked' | 'expanded' | 'window' | 'screen' | 'tray'

interface Selection {
  conversationId: string | null
  /// Null follows the newest turn.
  turnId: string | null
  /// The operation whose details are open.
  node: string | null
  revision?: AiRevisionSelection
}

/// The AI workspace: one stable view of the selected run's executable graph.
export function AiView({ mode, actions }: { mode: AiViewMode; actions: ReactNode }) {
  const audioSettings = useSettingsStore(state => state.settings)
  const playback = { rate: audioSettings?.tts_rate ?? 1, volume: audioSettings ? audioSettings.master_volume * audioSettings.voice_volume / 10000 : 0 }
  const tr = useI18n()
  const inspection = useNavigationStore(state => state.aiInspection)
  const executionPreferences = useExecutionPreferences()
  const activity = useConversationActivity()
  const { snapshot, turns } = activity
  const [selection, setSelection] = useState<Selection>({ conversationId: null, turnId: null, node: null })
  const [elsewhere, setElsewhere] = useState<ElsewhereSelection | null>(null)
  const [selectionError, setSelectionError] = useState<string | null>(null)
  const [selectionLoaded, setSelectionLoaded] = useState(false)
  const conversationId = snapshot?.conversationId ?? null
  const phone = mode === 'screen' || mode === 'tray'
  const hover = useGraphHoverPublisher(mode === 'window')

  // Restore the selected run when moving between windows.
  useEffect(() => {
    let cancelled = false
    getAiViewSelection().then(saved => {
      if (cancelled) return
      saved = useNavigationStore.getState().aiInspection ?? saved
      if (saved) {
        setSelection({ conversationId: saved.conversationId, turnId: saved.turnId, node: saved.operationKind, revision: saved.revision })
        setElsewhere(saved.elsewhere ?? null)
      }
      setSelectionLoaded(true)
    }).catch(error => { if (!cancelled) setSelectionError(nativeError(error)) })
    return () => { cancelled = true }
  }, [])
  useEffect(() => {
    if (!inspection || !selectionLoaded) return
    setSelection({ conversationId: inspection.conversationId, turnId: inspection.turnId, node: inspection.operationKind })
    setElsewhere(null)
    useNavigationStore.setState({ aiInspection: null })
  }, [inspection, selectionLoaded])
  useEffect(() => {
    if (selectionLoaded && conversationId && selection.conversationId !== conversationId) setSelection({ conversationId, turnId: null, node: null })
  }, [selectionLoaded, conversationId, selection.conversationId])
  useEffect(() => {
    if (!selectionLoaded || (conversationId && selection.conversationId !== conversationId)) return
    let current = true
    void setAiViewSelection({ conversationId: selection.conversationId, turnId: selection.turnId, operationKind: selection.node, ...(elsewhere ? { elsewhere } : {}), ...(selection.revision ? { revision: selection.revision } : {}) })
      .catch(error => { if (current) setSelectionError(nativeError(error)) })
    return () => { current = false }
  }, [selectionLoaded, conversationId, selection, elsewhere])

  const selectionReady = selectionLoaded && selection.conversationId === conversationId && conversationId !== null
  const pinned = selection.turnId ? turns.find(turn => turn.id === selection.turnId) : undefined
  // A saved selection can live beyond the snapshot's newest turns. Page until it
  // is found; never silently display a different exchange meanwhile.
  const restoring = selectionReady && selection.turnId !== null && !pinned
  useEffect(() => {
    if (restoring && activity.hasOlder && !activity.loadingOlder && !activity.error) activity.loadOlder()
  }, [restoring, activity.hasOlder, activity.loadingOlder, activity.error, activity.loadOlder])
  const turn = !selectionReady ? undefined : selection.turnId ? pinned : turns[0]
  const following = selectionReady && selection.turnId === null

  const turnKey: GraphSurfaceKey | null = turn && conversationId ? { scope: 'conversation', conversationId, turnId: turn.id } : null
  const workspaceKey: GraphSurfaceKey | null = elsewhere?.kind === 'workspace' ? { scope: 'workspace', engine: elsewhere.entry.engine, run: elsewhere.entry.run } : null
  const surface = elsewhere ? workspaceKey : turnKey
  const publish = hover.publish
  const onHoverResults = useCallback((results: string[] | null) => {
    publish(results && results.length && surface ? { key: surface, results } : null)
  }, [publish, surface])
  useEffect(() => () => publish(null), [publish, surface?.scope, turn?.id, elsewhere])
  const loadHistory = useCallback((before: string | null) => {
    if (!conversationId || !turn?.nativeGraph) throw new Error('No conversation run is selected.')
    return readGraphHistory(conversationId, turn.nativeGraph.run, before)
  }, [conversationId, turn?.nativeGraph?.run])
  const onPopover = (node: string | null) => setSelection(current => ({ ...current, node }))
  const orientation = phone ? 'down' : 'across'
  const transcription = elsewhere?.kind === 'transcription' ? snapshot?.transcriptionAttempts.find(attempt => attempt.id === elsewhere.id) ?? null : null
  const stations = useMemo<StationView[]>(() => {
    if (!turn?.nativeGraph || !snapshot) return []
    return turnStations(turn.nativeGraph, snapshot.messages.find(message => message.turnId === turn.id && message.role === 'user'), turn.speech)
      .map(station => ({ id: station.id, side: station.side, name: tr(station.nameKey), tone: station.tone, ports: station.ports,
        rows: station.rows.map(row => ({ label: tr(row.labelKey), value: row.value.kind === 'time' ? tr.dateTime(new Date(row.value.at)) : row.value.text })) }))
  }, [tr, turn, snapshot])

  return <section className="ai-view" data-mode={mode} aria-label={tr('AI activity')}>
    <header className="ai-view-head">
      <h2 className="ai-view-title">{tr('AI activity')}</h2>
      {mode !== 'tray' && <RunPicker turns={turns} selectedTurn={turn?.id ?? null} following={following} elsewhere={elsewhere}
        hasOlder={activity.hasOlder} loadingOlder={activity.loadingOlder} transcriptions={snapshot?.transcriptionAttempts ?? []}
        onFollow={() => { setElsewhere(null); setSelection(current => ({ ...current, turnId: null, node: null, revision: undefined })) }}
        onTurn={turnId => { setElsewhere(null); setSelection(current => ({ ...current, turnId, node: null, revision: undefined })) }}
        onElsewhere={next => { setElsewhere(next); setSelection(current => ({ ...current, node: null, revision: undefined })) }} onOlder={activity.loadOlder} />}
      {!phone && <AiExecutionSettings {...executionPreferences} />}
      {hover.visible && hover.visible.visible === 0 && <span className="ai-hover-note">{tr('Not on screen')}</span>}
      <div className="ai-view-spacer" />
      <div className="ai-view-actions">{actions}</div>
    </header>
    {(activity.error || selectionError) && <ErrorNotice as="p" className="ai-error" error={selectionError || activity.error}>{selectionError || activity.error}</ErrorNotice>}
    {elsewhere?.kind === 'workspace' ? <WorkspaceRunView key={JSON.stringify([elsewhere.entry.engine, elsewhere.entry.run])} entry={elsewhere.entry} visible orientation={orientation} compact={phone}
        initialRevision={selection.revision} onRevision={revision => setSelection(current => ({ ...current, revision, turnId: elsewhere ? current.turnId : turn?.id ?? current.turnId }))} popover={selection.node} onPopover={onPopover} onHoverResults={onHoverResults} />
      : elsewhere?.kind === 'transcription' ? <div className="ai-transcription">
          {transcription ? <><p>{transcription.model} · {transcription.state}</p>{transcription.error && <ErrorNotice as="p" error={transcription.error}>{transcription.error}</ErrorNotice>}<OriginalContent playback={playback} source={{ recording: transcription.id }} /><ResponseDetails value={transcription.diagnostics} /></>
            : <p className="ai-muted">{tr('No recorded AI operations.')}</p>}
        </div>
      : turn?.nativeGraph && conversationId
        ? <RunWorkspace key={turn.id} graph={turn.nativeGraph} preview={turn.nativePreview ?? null} stations={stations} turn={turn} messages={snapshot?.messages ?? []} loadHistory={loadHistory}
            controls={<NativeRunControls turn={turn} globallyPaused={snapshot?.connection.paused ?? false} />}
            orientation={orientation} compact={phone} initialRevision={selection.revision} onRevision={revision => setSelection(current => ({ ...current, revision, turnId: elsewhere ? current.turnId : turn?.id ?? current.turnId }))} popover={selection.node} onPopover={onPopover} onHoverResults={onHoverResults} />
        : <p className="ai-muted">{conversationId && !activity.error && !selectionError && (!selectionReady || (restoring && activity.hasOlder)) ? tr('Loading…') : tr('No recorded AI operations.')}</p>}
  </section>
}
