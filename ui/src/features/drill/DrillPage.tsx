import { useWordArrival } from './useWordArrival'
import { MobileAttemptHistory } from './MobileAttemptHistory'
import { MicrophoneSelector } from '../../components/media/MicrophoneSelector'
import { MicrophoneCheck } from '../../components/media/MicrophoneCheck'
import { useAudibleScrub } from '../../components/media/useAudibleScrub'
import { useRecordingPlayback } from '../../components/media/useRecordingPlayback'
import { useClipPreview } from './useClipPreview'
import { TakeQueue } from './TakeQueue'
import { PracticeAiStatus } from './PracticeAiStatus'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { useI18n } from '../../components/localization/i18n'
import { reportFault } from '../../platform/diagnostics/faults'
import { errorMessage } from '../../platform/diagnostics/error-details'
import { onRecordingPublished } from '../../platform/audio/recording-events'
import { useMicRecorder } from '../../platform/audio/useMicRecorder'
import { replaySelectionAudio, speakSelection } from '../../platform/audio/reading-speech'
import { cachedReadingAudio } from '../../platform/ipc/reading'
import type { PlaybackHandle, PlaybackObserver } from '../../platform/audio/speech-player'
import { CONTINUOUS_RECORDING_POLICY } from '../../generated/contracts'
import { TargetMessage } from '../../components/reading/TargetMessage'
import { ReadingLanguageScope } from '../../components/reading/ReadingLanguageScope'
import { ReadingScopeContext } from '../../components/reading/ReadingContext'
import { ResizeHandle, useStoredSize } from '../../components/layout/ResizeHandle'
import { useRecorderLayout } from '../../components/media/useRecorderLayout'
import { useUiDirection } from '../../components/localization/useUiDirection'
import { languageFor } from '../../platform/ipc/tauri'
import { useSettingsStore } from '../../state/settings/settings'
import { aiTraySlot } from '../../state/navigation/ai-tray'
import { clearDrillAttempts, deleteDrillAttempt, deleteDrillItem, drillItems, lastDrillItem, inspectDrillAudio } from '../../platform/ipc/drill'
import type { AudioInspection, DrillAttemptView, DrillGenerationPreview, DrillItemView, ListeningSettings } from '../../generated/contracts'
import { useDrillVisit } from './useDrillVisit'
import { useDrillAttempts } from './useDrillAttempts'
import { ResponseDetails } from '../../components/feedback/ResponseDetails'
import { DrillLayout } from './DrillLayout'
import { AttemptInspection } from './AttemptInspection'
import { DrillComparison, type TimeDirection, type TimeScale } from './DrillComparison'
import { AttemptRows } from './AttemptRows'
import { ClearTakes } from './ClearTakes'
import { useAttemptAudio } from './useAttemptAudio'
import { DrillStorage } from './DrillStorage'
import { DrillAnalysis } from './DrillAnalysis'
import { PhraseRail } from './PhraseRail'
import { AddPhrases } from './AddPhrases'
import { QuickStart } from './QuickStart'
import { usePersistentToggle } from '../../components/persistence/usePersistentToggle'
import { RecordDock, attemptCounts, dockPhase, type RecordMode } from './RecordDock'

/** The reference reading of one phrase: its audio and that audio's analysis. */
interface Reference { itemId: string; audioBase64: string; inspection: AudioInspection; alignment: import('../../generated/contracts').SpeechAlignment | null }

/** Practise saying one line: hear it, say it, see how close you were.
 *
 * Everything but the item, the attempt and the comparison is shared with Chat —
 * the target card, the recorder, the transcription and the spectrogram. */
export function DrillPage({ active }: { active: boolean }) {
  const tr = useI18n()
  const settings = useSettingsStore(state => state.settings)
  // New phrases are created in the language being practised now; an existing
  // item carries its own scope, which is what its aids and recordings use.
  const creating = useMemo(() => settings && {
    language: settings.target_language,
    variety: settings.target_variety,
    explanation: settings.native_language,
    explanationVariety: settings.native_variety,
  }, [settings])
  const [items, setItems] = useState<DrillItemView[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState<unknown>(null)
  // A phrase list that could not be read is said out loud, not only logged: an
  // empty page would otherwise read as "you have no phrases".
  const [loadFailure, setLoadFailure] = useState<unknown>(null)
  const [referenceFailure, setReferenceFailure] = useState<unknown>(null)
  const [reference, setReference] = useState<Reference | null>(null)
  // Which attempt the panel is showing. Nothing chosen means the newest one, so
  // a fresh attempt takes the panel without the learner asking.
  const [chosenAttemptId, setChosenAttemptId] = useState<string | null>(null)
  const [showUnscored, setShowUnscored] = useState(false)
  const [asking, setAsking] = useState(false)
  const [initialPreview, setInitialPreview] = useState<DrillGenerationPreview | null>(null)
  // The card list has been read at least once for this visit, so an empty list
  // means there are no cards rather than that they have not arrived yet.
  const [loaded, setLoaded] = useState(false)
  // The starter opens on every visit with no cards until the learner turns it off.
  const starterPreference = usePersistentToggle('skellyspeak_practice_starter', true)
  // Whether this visit has decided on the starter: it is offered once, on the
  // visit's first card list, never later when the last card is deleted.
  const starterDecided = useRef(false)
  const savingPreference = useSettingsStore(state => state.savingPreference)
  const setPreference = useSettingsStore(state => state.setPreference)
  const [speaking, setSpeaking] = useState(false)
  // A card's audio has been requested and has not arrived.
  const [fetchingAudio, setFetchingAudio] = useState(false)
  const [mode, setMode] = useState<RecordMode>('tap')
  const [autoDetect, setAutoDetect] = useState(true)
  const [listening, setListening] = useState<ListeningSettings>({
    pauseMs: CONTINUOUS_RECORDING_POLICY.defaultPauseMs,
    thresholdDb: CONTINUOUS_RECORDING_POLICY.defaultThresholdDb,
    minTakeMs: CONTINUOUS_RECORDING_POLICY.defaultMinTakeMs,
    silenceTimeoutMs: CONTINUOUS_RECORDING_POLICY.defaultSilenceTimeoutMs,
  })
  // Follow script direction until the learner explicitly chooses a time direction.
  const [chosenDirection, setChosenDirection] = useState<TimeDirection | null>(null)
  const [timeScale, setTimeScale] = useState<TimeScale>('words')
  // Every split on the page can be dragged; each size is kept for next time.
  const [reportWidth, setReportWidth] = useStoredSize('drill-report')
  const [dockHeight, setDockHeight] = useStoredSize('drill-dock')
  const page = useRef<HTMLElement>(null)
  const panes = useRef<Record<'dock' | 'attempts' | 'report', HTMLElement | null>>({ dock: null, attempts: null, report: null })
  const measure = (name: keyof typeof panes.current) => () => {
    const element = panes.current[name]
    if (!element) throw new Error(`The ${name} pane is not on the page.`)
    return name === 'report' ? element.getBoundingClientRect().width : element.getBoundingClientRect().height
  }

  const px = (size: number | null) => size === null ? undefined : `${Math.round(size)}px`
  const [referenceTime, setReferenceTime] = useState(0)
  const referencePlayer = useRef<PlaybackHandle | null>(null)
  const playbackRate = useRef(settings?.tts_rate ?? 1)
  playbackRate.current = settings?.tts_rate ?? 1
  const selected = items.find(item => item.id === selectedId) ?? null
  useEffect(() => {
    if (!selected || selected.language !== creating?.language) return
    try { localStorage.setItem(`skellyspeak_drill_phrase_${selected.language}`, selected.id) }
    catch (error) { setFailure(error); reportFault('Saving selected drill phrase', error) }
  }, [selected?.id, selected?.language, creating?.language])
  // The item's own stored scope: an aid asks about the phrase as it was written,
  // not as today's settings happen to read.
  const scope = useMemo(() => selected && {
    language: selected.language,
    variety: selected.variety,
    explanation: selected.explanation,
    explanationVariety: selected.explanationVariety,
  }, [selected])

  const showingLanguage = useRef(creating?.language)
  showingLanguage.current = creating?.language
  const mounted = useRef(false)
  const loading = useRef(0)
  const selectionGeneration = useRef(0)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; loading.current++ } }, [])
  const reload = useCallback(async (keep?: string) => {
    if (!creating) return
    const request = ++loading.current
    const generation = selectionGeneration.current
    const language = creating.language
    const [loaded, lastVisited] = await Promise.all([drillItems(language), lastDrillItem(language)])
    if (!mounted.current || request !== loading.current || showingLanguage.current !== language) return
    const remembered = localStorage.getItem(`skellyspeak_drill_phrase_${language}`)
    setItems(loaded)
    setLoaded(true)
    setSelectedId(current => generation === selectionGeneration.current && keep
      ? keep : loaded.some(item => item.id === current) ? current
        : loaded.some(item => item.id === lastVisited) ? lastVisited
          : loaded.some(item => item.id === remembered) ? remembered : loaded[0]?.id ?? null)
  }, [creating?.language])
  const refresh = useCallback((keep?: string) => {
    void reload(keep)
      .then(() => { if (mounted.current) setLoadFailure(null) })
      .catch(error => { if (mounted.current) setLoadFailure(error); reportFault('Loading drill items', error) })
  }, [reload])
  useEffect(() => {
    if (!active) return
    setItems([]); setSelectedId(null); setLoadFailure(null); setLoaded(false); starterDecided.current = false
    refresh()
    const unsubscribe = onRecordingPublished(owner => { if (owner.kind === 'drillItem') refresh() })
    return () => { unsubscribe(); loading.current++ }
  }, [active, refresh])

  const history = useDrillAttempts(selected?.id ?? null, active)
  const newestTake = useRef<{ item: string | null; id: string | null }>({ item: null, id: null })
  // The take that just arrived, for the length of one movement: the rows it
  // displaced step down to make room for it. Nothing is set on a first read, so
  // opening a phrase moves nothing.
  const [arrivedId, setArrivedId] = useState<string | null>(null)
  const clearArrival = useCallback(() => setArrivedId(null), [])
  useEffect(() => {
    const id = history.attempts[0]?.id ?? null
    const item = selected?.id ?? null
    if (id && newestTake.current.id && newestTake.current.item === item && newestTake.current.id !== id) {
      setChosenAttemptId(null)
      setArrivedId(id)
    }
    newestTake.current = { item, id }
  }, [history.attempts[0]?.id, selected?.id])
  const visit = useDrillVisit(active, creating?.language ?? null, selected?.id ?? null)
  const owner = useMemo(() => active && visit.visitId && selected ? { kind: 'drillItem' as const, id: selected.id } : null, [active, selected?.id, visit.visitId])
  const mic = useMicRecorder({ owner, onTranscribe: () => {}, listening, captureMode: mode === 'live' ? (autoDetect ? 'auto' : 'monitor') : 'manual' })
  const clip = useClipPreview(owner?.id ?? null, mic.listeningStatus)
  useEffect(() => { if (clip.preview) setChosenAttemptId(null) }, [clip.preview?.recordingId])
  const phase = dockPhase({ ready: visit.visitId !== null, recording: mic.recording, transcribing: mic.transcribing })
  // The microphone and the speakers share one authority, so neither reference
  // playback nor replay runs while an attempt is being captured or stored.
  const holdingAudio = mic.recording || mic.transcribing

  const changeListening = (next: ListeningSettings) => {
    setListening(next)
    if (mic.recording && mic.listeningStatus?.listening) mic.tune(next)
  }

  const changeMode = (next: RecordMode) => { if (!mic.recording && !mic.starting) setMode(next) }
  const changeAutoDetect = (enabled: boolean) => {
    setAutoDetect(enabled)
    if (mic.recording && mode === 'live') mic.tune(listening, enabled ? 'auto' : 'monitor')
  }
  const holdSession = useRef<{ began: number; ready: Promise<void>; released: boolean } | null>(null)
  useEffect(() => () => { holdSession.current = null }, [owner?.id])
  const holdStart = () => {
    if (holdSession.current || mic.recording) return
    holdSession.current = { began: Date.now(), ready: mic.toggleMic(), released: false }
  }
  const holdEnd = () => {
    const session = holdSession.current
    if (!session || session.released) return
    session.released = true
    if (Date.now() - session.began < listening.minTakeMs) {
      holdSession.current = null
      mic.cancel()
    } else {
      const stopped = mic.stopMic()
      void Promise.all([session.ready, stopped]).then(() => {
        if (holdSession.current === session) holdSession.current = null
      })
    }
  }

  // Deleting takes changes the item's counts and its history, so both are read again.
  const [removedRecordings, setRemovedRecordings] = useState<Set<string>>(new Set())
  const [deletingTakes, setDeletingTakes] = useState(false)
  const removeTakes = async (run: () => Promise<unknown>) => {
    setDeletingTakes(true); setFailure(null)
    try { await run(); setChosenAttemptId(null); history.retry(); await reload(selected?.id) }
    catch (error) { setFailure(error) } finally { setDeletingTakes(false) }
  }
  const deleteTake = (take: DrillAttemptView) => void removeTakes(async () => {
    await deleteDrillAttempt(take.id)
    if (take.transcriptionAttemptId) setRemovedRecordings(ids => new Set([...ids, take.transcriptionAttemptId!]))
  })
  const clearTakes = (since: Date | null) => {
    if (!selected) throw new Error('Clearing takes needs a selected phrase.')
    const item = selected.id
    void removeTakes(async () => {
      await clearDrillAttempts(item, since?.toISOString() ?? null)
      // Capture is held while clearing; every live receipt belongs to this visit.
      const ids = history.attempts.filter(take => !since || new Date(take.createdAt) >= since)
        .flatMap(take => take.transcriptionAttemptId ? [take.transcriptionAttemptId] : [])
      setRemovedRecordings(previous => new Set([...previous, ...ids]))
    })
  }

  const remove = async (item: DrillItemView) => {
    setBusy(true); setFailure(null)
    try { await deleteDrillItem(item.id); await reload() }
    catch (error) { setFailure(error) } finally { setBusy(false) }
  }

  /// Play the item the way it should sound, and keep the audio so its
  /// spectrogram can sit beside the learner's. The audio is reused: replaying a
  /// native cache checks the current source and speech configuration on every play.
  const referenceRequest = useRef<AbortController | null>(null)
  useEffect(() => {
    setSpeaking(false); setFetchingAudio(false); setFailure(null); setReferenceFailure(null); setReference(null); setReferenceTime(0); referencePlayer.current = null
    if (active && selected && scope) {
      const itemId = selected.id
      const controller = new AbortController()
      referenceRequest.current = controller
      const current = () => referenceRequest.current === controller && !controller.signal.aborted
      void cachedReadingAudio({ ...scope, text: selected.text, aid: 'speech', referenceItem: itemId }).then(async audio => {
        if (!audio || !current()) return
        const { audioBase64 } = audio
        const inspection = await inspectDrillAudio(itemId, audioBase64, { speechAlignment: audio.alignment })
        if (current()) setReference({ itemId, audioBase64, inspection, alignment: audio.alignment })
      }).catch(error => { if (current()) setReferenceFailure(error) })
    }
    return () => { referenceRequest.current?.abort(); referenceRequest.current = null }
  }, [selected?.id, selected?.text, scope?.language, scope?.variety, scope?.explanation, scope?.explanationVariety, active])
  const playReference = async (item: DrillItemView, startSeconds?: number) => {
    if (!active || !scope || holdingAudio) return
    referenceRequest.current?.abort()
    const controller = new AbortController()
    referenceRequest.current = controller
    setSpeaking(true); setFetchingAudio(false); setReferenceFailure(null)
    const current = () => referenceRequest.current === controller && !controller.signal.aborted && itemShowing.current === item.id
    const observer: PlaybackObserver = {
      sourceText: item.text,
      startSeconds,
      onTime: seconds => { if (current()) setReferenceTime(seconds) },
      onReady: player => { if (current()) { referencePlayer.current = player; player?.setRate(playbackRate.current) } },
    }
    const rate = settings?.tts_rate ?? 1
    const volume = (settings?.master_volume ?? 100) * (settings?.voice_volume ?? 100) / 10000
    try {
      if (startSeconds !== undefined && reference?.itemId === item.id) {
        // Seeking retained audio must not issue a speech-generation request.
        await replaySelectionAudio(reference.audioBase64, controller.signal, () => {}, rate, volume, observer, reference.alignment)
      } else {
        // The AI pill names the request until its audio arrives.
        setFetchingAudio(true)
        await speakSelection({ ...scope, text: item.text, aid: 'speech', referenceItem: item.id }, controller.signal, () => {}, rate, volume, {
          ...observer,
          onAudio: async spoken => {
            if (referenceRequest.current === controller) setFetchingAudio(false)
            controller.signal.throwIfAborted()
            if (!spoken.audioBase64) throw new Error('The speech service returned no audio.')
            const inspection = await inspectDrillAudio(item.id, spoken.audioBase64, { speechAlignment: spoken.audioAlignment })
            controller.signal.throwIfAborted()
            if (current()) {
              setReference({ itemId: item.id, audioBase64: spoken.audioBase64, inspection, alignment: spoken.audioAlignment })
            }
          },
        })
      }
    } catch (error) {
      if (current() && !(error instanceof DOMException && error.name === 'AbortError')) setReferenceFailure(error)
    } finally {
      if (referenceRequest.current === controller) { setSpeaking(false); setFetchingAudio(false); referencePlayer.current = null }
    }
  }
  useEffect(() => { referencePlayer.current?.setRate(settings?.tts_rate ?? 1) }, [settings?.tts_rate])
  const seekReference = (seconds: number) => {
    if (holdingAudio || !selected || reference?.itemId !== selected.id) return
    setReferenceTime(seconds)
    if (speaking) referencePlayer.current?.seek(seconds)
  }
  // What is on screen right now, for late results to check themselves against.
  const itemShowing = useRef<string | null>(null)
  itemShowing.current = selected?.id ?? null

  // Pages arrive newest first, so the head of the list is the latest attempt.
  const visibleAttempts = history.attempts.filter(entry => showUnscored || (entry.comparison.reliability?.accepted !== false && entry.comparison.matchRatio !== null))
  const preview = chosenAttemptId === null && clip.preview && !removedRecordings.has(clip.preview.recordingId) ? clip.preview : null
  const previewAttempt = preview ? history.attempts.find(entry => entry.transcriptionAttemptId === preview.recordingId) : null
  const attempt = visibleAttempts.find(entry => entry.id === chosenAttemptId) ?? visibleAttempts[0] ?? null
  const audioAttempt = preview ? previewAttempt ?? null : attempt
  const attemptAudio = useAttemptAudio(selected?.id ?? null, audioAttempt)
  const shownAttemptAudio = preview && audioAttempt?.transcriptionAttemptId !== preview.recordingId ? null : attemptAudio.audio
  const inspectionCache = useRef(new Map<string, AudioInspection>())
  useEffect(() => { inspectionCache.current.clear() }, [selected?.id])
  useEffect(() => {
    if (attemptAudio.audio?.inspection) inspectionCache.current.set(attemptAudio.audio.attemptId, attemptAudio.audio.inspection)
  }, [attemptAudio.audio])
  const scrubVolume = (settings?.master_volume ?? 100) * (settings?.voice_volume ?? 100) / 10000
  const attemptPlayback = useRecordingPlayback({ audio: shownAttemptAudio?.base64 ?? null, duration: shownAttemptAudio?.inspection?.duration ?? 0,
    enabled: active && !holdingAudio, rate: settings?.tts_rate ?? 1, volume: scrubVolume, onError: setFailure })
  const referenceScrub = useAudibleScrub(reference?.itemId === selected?.id ? reference?.audioBase64 ?? null : null,
    active && !holdingAudio, speaking, scrubVolume, setReferenceFailure,
    () => { referenceRequest.current?.abort(); setSpeaking(false) },
    selected ? { text: selected.text, alignment: reference?.alignment } : undefined)

  const locale = scope ? languageFor(scope.language, scope.variety) : creating ? languageFor(creating.language, creating.variety) : null
  // Until the learner chooses, the recorder follows the interface direction, as in the conversation panel; the card's script never moves it.
  const recorder = useRecorderLayout('practice', useUiDirection())
  // Once open, the starter stays until it is closed or cards exist, even when
  // "Don't show this again" is ticked inside it.
  const [starterShown, setStarterShown] = useState(false)
  useEffect(() => {
    if (!active || !loaded || starterDecided.current) return
    starterDecided.current = true
    setStarterShown(items.length === 0 && loadFailure == null && starterPreference.open)
  }, [active, loaded, items.length, loadFailure, starterPreference.open])
  const starterOpen = starterShown && items.length === 0
  const closeStarter = () => setStarterShown(false)
  if (!creating) return <p role="status">{tr("Loading…")}</p>
  const shown = reference?.itemId === selected?.id ? reference : null
  const rtl = locale?.direction === 'rtl'
  const direction = chosenDirection ?? (rtl ? 'rtl' : 'ltr')
  const attemptUnavailable = !attempt ? null
    : attempt.audioPrunedAt !== null ? tr("This attempt's recording was removed by the storage limit.")
    : attempt.audioBytes === null ? tr("This attempt's recording was not kept.")
    : null

  // With no card the stage, the recorder and the attempt list are still the
  // real ones, each in its own empty state. Only a read, empty card list says
  // there are no cards; before the first read nothing claims it.
  const empty = loaded && items.length === 0
  // The AI pill leads the recorder, as it leads Chat's; attempts queued by a
  // listening session are still on their way through the speech service. On
  // phones the AI View's tray rises right above the pill. The recording
  // panel's grip sits on the panel itself, below the pill, as in Chat.
  const dock = (<>
        <div className="ai-tray-slot" ref={aiTraySlot} />
        <div className="drill-ai-status"><PracticeAiStatus fetchingAudio={fetchingAudio}
          transcribing={mic.transcribing || mic.listeningStatus?.processing === true || (mic.listeningStatus?.queued ?? 0) > 0} /></div>
        <ResizeHandle label={tr("Resize the recording panel")} axis="y" grow={-1} size={dockHeight} min={150} max={900} measure={measure('dock')} onResize={setDockHeight} />
        <div className="drill-pane drill-dock-pane" ref={element => { panes.current.dock = element }}>
          <RecordDock microphoneSelector={<MicrophoneSelector value={settings?.microphone_device_id ?? null}
            disabled={!settings || mic.starting || phase === 'recording' || phase === 'working' || savingPreference}
            onChange={microphone_device_id => { void useSettingsStore.getState().update(current => ({ ...current, microphone_device_id }), 'Changing microphone') }} />}
            microphoneCheck={<MicrophoneCheck device={settings?.microphone_device_id ?? null} disabled={!settings || mic.starting || phase === 'recording' || phase === 'working'} />}
            device={settings ? settings.microphone_device_id : undefined} silentTake={mic.silentTake !== null} onDismissSilentTake={mic.dismissSilentTake}
            layout={recorder} health={mic.health} deviceLabel={mic.deviceLabel} starting={mic.starting} empty={empty} phase={phase} mode={mode} onMode={changeMode} autoDetect={autoDetect} onAutoDetect={changeAutoDetect} settings={listening} onSettings={changeListening}
            listeningStatus={mic.listeningStatus} waveSource={mic.waveSource} spectrum={mic.spectrum}
            onToggle={() => void mic.toggleMic()}
            onHoldStart={holdStart} onHoldEnd={holdEnd} />
        </div>
  </>)
  // The listening session's counts sit beside the attempt list, not in the recorder.
  const counts = attemptCounts(mic.listeningStatus, tr)
  const countsLine = counts && <p className="drill-attempt-counts">{counts}</p>
  const liveTakes = [...mic.pendingRecordings, ...(mic.listeningStatus?.takes ?? [])].filter(take => !removedRecordings.has(take.recordingId))
  const queue = <TakeQueue compact={!liveTakes.some(take => take.failure != null)} takes={liveTakes} attempts={history.attempts}
    onRecordAgain={() => void mic.toggleMic()} recordingBusy={mic.recording || mic.transcribing} />
  const hasPendingTake = liveTakes.some(take => !history.attempts.some(item => item.transcriptionAttemptId === take.recordingId))
  const arrivingAttempt = audioAttempt && liveTakes.some(take => take.recordingId === audioAttempt.transcriptionAttemptId) ? audioAttempt : null
  const heldHistoryId = useWordArrival(arrivingAttempt?.id ?? null,
    shownAttemptAudio?.attemptId === arrivingAttempt?.id || attemptAudio.failure != null || arrivingAttempt?.audioBytes === null || arrivingAttempt?.audioPrunedAt != null,
    selected?.id ?? null)
  const presentedAttempts = visibleAttempts.filter(take => take.id !== heldHistoryId)
  const mobileRowReserved = hasPendingTake || heldHistoryId !== null
  const renderAttemptDetails = (take: DrillAttemptView) => <AttemptInspection attempt={take}
    audio={attemptAudio.audio?.attemptId === take.id ? attemptAudio.audio.inspection : inspectionCache.current.get(take.id) ?? null}
    reference={shown?.inspection ?? null} rtl={rtl} onDelete={() => deleteTake(take)} deleting={deletingTakes || holdingAudio} />
  const report = (
      <MobileAttemptHistory key={selected?.id} detail={id => {
        const take = history.attempts.find(entry => entry.id === id)
        return take ? renderAttemptDetails(take) : null
      }} preview={openAttempt => <>{countsLine}<TakeQueue compact takes={liveTakes} attempts={history.attempts.filter(take => take.id !== heldHistoryId)} /><AttemptRows reservationKey={mobileRowReserved ? "reserved" : "settled"} attempts={presentedAttempts} selectedId={attempt?.id} onSelect={id => { setChosenAttemptId(id); openAttempt(id) }} rtl={rtl} /></>}>
      <aside className="drill-log" aria-label={tr("Attempts")} ref={element => { panes.current.report = element }}>
        {history.attempts.length > 0 && <ClearTakes disabled={deletingTakes || holdingAudio} onClear={clearTakes} />}
        <div className="drill-pane drill-attempts-pane" ref={element => { panes.current.attempts = element }}>
          <div className="drill-report-status">{countsLine}{queue}</div>
          <AttemptRows attempts={presentedAttempts} selectedId={presentedAttempts.some(take => take.id === attempt?.id) ? attempt?.id : presentedAttempts[0]?.id} onSelect={setChosenAttemptId} rtl={rtl}
            arrivedId={arrivedId} onArrivalEnd={clearArrival}
            renderDetails={renderAttemptDetails}
            footerControls={<label className="drill-unscored"><input type="checkbox" checked={showUnscored} onChange={event => setShowUnscored(event.target.checked)} />{tr("Show unscored")}</label>} />
        </div>
        {history.failure != null && <ErrorNotice as="p" error={history.failure}>{errorMessage(history.failure)}
          <button type="button" className="btn" onClick={history.retry}>{tr("Try again")}</button></ErrorNotice>}
        {history.loading && <p role="status">{tr("Loading…")}</p>}
        {!history.loading && history.attempts.length === 0 && <p>{tr("No attempts yet. Record one to compare.")}</p>}
        {history.hasMore && <button type="button" className="btn drill-more" disabled={history.loading} onClick={history.loadMore}>{tr("Show older attempts")}</button>}
      </aside>
      </MobileAttemptHistory>
  )
  const toggleReference = () => {
    if (holdingAudio || !selected) return
    if (speaking) { referenceRequest.current?.abort(); referencePlayer.current?.stop(); setSpeaking(false) }
    else void playReference(selected, referenceTime > 0 && referenceTime < (shown?.inspection.duration ?? 0) ? referenceTime : undefined)
  }

  const practice = (
      <main className="drill-stage">
        {loadFailure != null && <ErrorNotice as="p" error={loadFailure}>{errorMessage(loadFailure)}
          <button type="button" className="btn" onClick={() => refresh()}>{tr("Try again")}</button></ErrorNotice>}
        {visit.failure != null && <ErrorNotice as="p" error={visit.failure}>{errorMessage(visit.failure)}
          <button type="button" className="btn" onClick={visit.retry}>{tr("Try again")}</button></ErrorNotice>}
        {failure != null && <ErrorNotice as="p" error={failure}>{errorMessage(failure)}</ErrorNotice>}

        <DrillComparison comparisonAccepted={audioAttempt?.comparison.reliability?.accepted !== false} target={selected && scope ? <DrillAnalysis item={selected} scope={scope} nativeLanguageName={settings?.native_language ?? ''}>
            {analysis => (
              <TargetMessage provenance={{ source: selected.source, addedAt: selected.createdAt, reported: null }} readAloud={false} addToDrill={false} layout="bubble" text={selected.text} segments={[]} segmentsKey={selected.id}
                translation={null} romanization={null} pronunciation={null} translateLabel={tr("Translate")}
                segmentsPending={false} lookupWords status={null} annotation={null} analysis={analysis}
                speech={{ speaking, onToggle: toggleReference, disabled: holdingAudio, error: null }} focused={false} rtl={locale?.direction === 'rtl'} />
            )}
          </DrillAnalysis> : null}
          playbackSpeed={<label className="drill-playback-speed"><span>{tr('Voice speed')}</span><select className="field" aria-label={tr('Voice speed')}
            value={settings?.tts_rate ?? 1} disabled={savingPreference || !settings}
            onChange={event => void setPreference('tts_rate', Number(event.target.value))}>
            {[...new Set([0.5, 0.65, 0.8, 1, 1.2, 1.5, settings?.tts_rate ?? 1])].sort((a, b) => a - b).map(rate => <option key={rate} value={rate}>{rate}×</option>)}
          </select></label>}
          referenceFailure={referenceFailure != null && selected ? <ErrorNotice as="div" error={referenceFailure}><strong>{tr('Reference')}</strong><p>{errorMessage(referenceFailure)}</p>
            <button type="button" className="btn" disabled={holdingAudio || speaking} onClick={() => void playReference(selected)}>{tr('Try again')}</button><ResponseDetails value={referenceFailure} /></ErrorNotice> : undefined}
          onPlayReference={toggleReference} playingReference={speaking}
          referenceNote={holdingAudio ? tr("Playback waits until the attempt is stored.") : tr("Replays reuse the saved reference; no new request is made.")}
          reference={shown?.inspection ?? null} referenceTime={referenceTime} onSeekReference={seekReference} referenceScrub={referenceScrub} attemptScrub={attemptPlayback.scrub}
          attempt={shownAttemptAudio?.inspection ?? null} attemptAudioReady={!!shownAttemptAudio} preview={preview} attemptTime={attemptPlayback.time}
          attemptLabel={preview ? tr("Attempt {value0}", { value0: preview.number }) : attempt ? tr("Attempt {value0}", { value0: String(attempt.sequence) }) : null}
          attemptFailure={clip.failure ?? attemptAudio.failure} onRetryAttempt={() => { if (clip.failure) clip.retry(); else attemptAudio.retry() }} attemptUnavailable={preview ? null : attemptUnavailable}
          onSeekAttempt={attemptPlayback.seek}
          direction={direction} onDirection={setChosenDirection} timeScale={timeScale} onTimeScale={setTimeScale} holding={holdingAudio}
          playingAttempt={attemptPlayback.playing} onPlayAttempt={attemptPlayback.toggle} />


      </main>
  )

  return (
    <ReadingScopeContext value={scope ?? creating}><ReadingLanguageScope language={scope?.language ?? creating.language} variety={scope?.variety ?? creating.variety}>
    <section className="drill-page" aria-label={tr("Practice")} ref={page} style={{
      '--drill-report-width': px(reportWidth), '--drill-dock-height': px(dockHeight),
    } as CSSProperties}>
      <div className="drill-error-overlay">
          {mic.failure != null && <ErrorNotice as="div" error={mic.failure}><strong>{tr('Microphone')}</strong><p>{errorMessage(mic.failure)}</p><ResponseDetails value={mic.failure} />
            <button type="button" className="btn" disabled={mic.recording || mic.transcribing} onClick={() => void mic.toggleMic()}>{tr('Record again')}</button></ErrorNotice>}
      </div>
      <DrillLayout items={items} empty={empty} selectedId={selectedId} locked={holdingAudio} onAddPhrases={() => setAsking(true)}
        onSelect={id => { selectionGeneration.current++; setChosenAttemptId(null); setSelectedId(id) }}
        attempt={attempt} rtl={rtl} dock={dock} report={report} queue={queue}
        reportResize={<ResizeHandle label={tr("Resize the report column")} axis="x" grow={-1} size={reportWidth} min={220} max={900} measure={measure('report')} onResize={setReportWidth} />}
        rail={<PhraseRail items={items} selectedId={selectedId} busy={busy} locked={holdingAudio}
        onSelect={id => { selectionGeneration.current++; setChosenAttemptId(null); setSelectedId(id) }} onDelete={remove} onAddPhrases={() => setAsking(true)}>
        <DrillStorage active={active} onChanged={reload} />
      </PhraseRail>}>

      {practice}

      </DrillLayout>

      {/* Raised over the practice columns rather than replacing them. */}
      {asking && <AddPhrases scope={creating} initialPreview={initialPreview} onAdded={async () => refresh()}
        onClose={() => { setAsking(false); setInitialPreview(null) }} />}
      {starterOpen && <QuickStart scope={creating} showAgain={starterPreference.open} onShowAgain={starterPreference.toggle}
        onPreview={async preview => { setInitialPreview(preview); closeStarter(); setAsking(true) }} onClose={closeStarter} />}
    </section>
    </ReadingLanguageScope></ReadingScopeContext>
  )
}

export default DrillPage
