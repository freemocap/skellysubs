import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from 'react'
import { useI18n } from '../localization/i18n'
import { MicrophoneLamp } from './MicrophoneLamp'
import { MicrophoneCheckStart } from './MicrophoneCheck'
import type { MicrophoneHealth } from '../../domain/audio/microphone-health'
import { useMicrophonePresence } from '../../platform/audio/useMicrophonePresence'
import { ActivityIndicator } from '../feedback/ActivityIndicator'
import { ToolbarIcon } from '../controls/ToolbarIcon'
import { SegmentedChoice } from '../controls/SegmentedChoice'
import { DetailDialog } from '../dialogs/DetailDialog'
import { useUiDirection } from '../localization/useUiDirection'
import { useWidthTier } from '../layout/useWidthTier'
import type { RecorderLayout } from './useRecorderLayout'

/** How a recording begins and ends: one press each, held down, or cut at each pause. */
export type VoiceMode = 'tap' | 'hold' | 'auto'
/** What the microphone is doing: getting ready, ready, recording, or finishing work. */
export type VoicePhase = 'preparing' | 'ready' | 'recording' | 'working'

/** The pad answers a press (Tap, Auto) or a hold (Hold). */
export type PadAction = { kind: 'press'; onPress: () => void } | { kind: 'hold'; onHoldStart: () => void; onHoldEnd: () => void }

const MODES: readonly VoiceMode[] = ['tap', 'hold', 'auto']

/** The one recorder, identical in Chat and Practice.
 *
 * A two-by-two grid: the face (the stream, a draft, or one arrow pointing at the
 * pad) beside the microphone pad, then one row of controls under the face and
 * the Tap / Hold / Auto toggle under the pad. The pad's side is the learner's
 * choice, independent of which way the stream runs. The pad is only ever
 * the microphone: calm blue when ready, red with a red outline and a glow while
 * recording, faded while it waits. There is no standing instruction; the phase is
 * announced to screen readers instead. The microphone's own state is one lamp
 * in the control row (MicrophoneLamp); its words stay in the lamp's name and
 * tooltip, so nothing under the recorder grows. Two states also show over the
 * face, where the eye is: sustained quiet while recording is a band across the
 * stream, and a recording that never rose above the floor leaves a card that
 * leads to the check. Recording logic stays with the caller. */
export function VoicePanel({ label, phase, starting = false, health, deviceLabel, device, silentTake = false, onDismissSilentTake, face, prompt, mode, onMode, laterModes = [], modesDisabled = false, pad, controls, settings, microphoneSelector, microphoneCheck, status, faceTitle, onDiscard, layout, className }: {
  label: string
  phase: VoicePhase
  starting?: boolean
  /** The signal observed while recording; the lamp shows it, and sustained quiet bands the stream. */
  health?: MicrophoneHealth | null
  /** The device the recording actually opened, when the platform names it. */
  deviceLabel?: string | null
  /** The saved device choice (null is the system default), so the lamp can say
   * when it is not connected. Undefined while the owner has no choice to track. */
  device?: string | null
  /** The last recording never rose above the floor: the face says so and offers the check. */
  silentTake?: boolean
  onDismissSilentTake?: () => void
  /** The stream or a draft; null shows the prompt while the pad can start a recording. */
  face: ReactNode | null
  /** What the prompt says beside its arrow, when it has something to add, such
   * as the greeting to say; by default it only restates the pad, so screen
   * readers skip it. */
  prompt?: ReactNode
  mode: VoiceMode
  onMode: (mode: VoiceMode) => void
  /** Shown in the toggle but not available here yet: pressing one says “Coming soon”. */
  laterModes?: readonly VoiceMode[]
  modesDisabled?: boolean
  pad: { label: string; title?: string; disabled: boolean; action: PadAction }
  /** One row under the face, after the settings button and the lamp: the meter, Type, Auto-send or Detect attempts. */
  controls?: ReactNode
  /** The recording settings dialog's content; the button that opens it leads the control row. */
  settings?: ReactNode
  /** Visible in the desktop footer; available in recording settings at every width. */
  microphoneSelector?: ReactNode
  /** The local check row (MicrophoneCheck), in recording settings under the picker. */
  microphoneCheck?: ReactNode
  /** Announced when the phase changes, without drawing a standing instruction. */
  status?: string
  faceTitle?: string
  /** Overlaid on the microphone-facing edge of the stream while recording. */
  onDiscard?: () => void
  /** Where the pad sits and which way the stream runs, offered in the recording
   * settings. Without it the pad sits at the end of the reading direction. */
  layout?: RecorderLayout
  className?: string
}) {
  const tr = useI18n()
  const desktop = useWidthTier() === 'full'
  const recording = phase === 'recording'
  const [soon, setSoon] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)
  // Recording settings opened from the silent-recording card runs the check at once.
  const [checkAtOnce, setCheckAtOnce] = useState(false)
  const openSettings = (runCheck = false) => { setCheckAtOnce(runCheck); setSettingsOpen(true) }
  const closeSettings = () => { setSettingsOpen(false); setCheckAtOnce(false) }
  const elapsed = useElapsed(recording)
  const reading = useUiDirection()
  const presence = useMicrophonePresence(device, recording)
  const padSide = layout?.padSide ?? (reading === 'rtl' ? 'left' : 'right')
  // Grid areas follow the reading direction, so the physical side becomes start or end.
  const padAt = (padSide === 'right') === (reading === 'ltr') ? 'end' : 'start'
  useEffect(() => {
    if (!soon) return
    const timer = setTimeout(() => setSoon(false), 1800)
    return () => clearTimeout(timer)
  }, [soon])

  const action = pad.action
  const padEvents = action.kind === 'press' ? { onClick: action.onPress } : {
    onKeyDown: (event: KeyboardEvent) => {
      if ((event.key === ' ' || event.key === 'Enter') && !event.repeat) { event.preventDefault(); action.onHoldStart() }
    },
    onKeyUp: (event: KeyboardEvent) => {
      if (event.key === ' ' || event.key === 'Enter') { event.preventDefault(); action.onHoldEnd() }
    },
    onPointerDown: (event: PointerEvent<HTMLButtonElement>) => {
      event.currentTarget.setPointerCapture?.(event.pointerId); action.onHoldStart()
    },
    onPointerUp: action.onHoldEnd,
    onPointerCancel: action.onHoldEnd,
    onLostPointerCapture: action.onHoldEnd,
    onBlur: action.onHoldEnd,
  }
  const modeName = { tap: tr('Tap to record'), hold: tr('Hold to talk'), auto: tr('Auto') }
  const modeShort = { tap: tr('Tap'), hold: tr('Hold'), auto: tr('Auto') }
  const seconds = Math.floor(elapsed)

  return <section className={['voice-panel', className].filter(Boolean).join(' ')} data-phase={phase} data-mode={mode} data-pad={padAt} data-pad-side={padSide} aria-label={label}>
    <div className="voice-grid">
      <div className="voice-face" title={faceTitle}>
        {face ?? (phase === 'ready' && !pad.disabled && <div className="voice-prompt" aria-hidden={prompt ? undefined : true}>
          <span className="voice-prompt-text">{prompt ?? tr('Press the microphone to start')}</span>
          <ToolbarIcon name="chevron" size={18} />
        </div>)}
        {/* The lamp announces the full sentence, so the band is for sight only. */}
        {recording && health?.signal === 'quiet' && <div className="voice-face-notice" aria-hidden="true">{tr('No sound. Check mute or move closer.')}</div>}
        {recording && <span className="voice-chip"><span className="voice-dot" aria-hidden="true" />{Math.floor(seconds / 60)}:{String(seconds % 60).padStart(2, '0')}</span>}
        {recording && onDiscard && <button type="button" className="voice-discard-recording" onClick={onDiscard}
          aria-label={tr('Discard recording')} title={tr('Discard recording without transcribing')}><ToolbarIcon name="trash" size={15} /></button>}
      </div>
      {/* Over the face as its own grid item, so the shallow phone face does not clip it. */}
      {!recording && silentTake && <div className="voice-face-card">
        <p role="status">{tr('It sounds like nothing reached your microphone.')}</p>
        <button type="button" className="btn" onClick={() => { onDismissSilentTake?.(); openSettings(true) }}>{tr('Check microphone')}</button>
        <button type="button" className="voice-face-card-close" aria-label={tr('Dismiss')} title={tr('Dismiss')} onClick={onDismissSilentTake}><ToolbarIcon name="close" size={14} /></button>
      </div>}
      <button type="button" className="voice-pad" data-live={recording} aria-pressed={recording} aria-busy={phase === 'preparing' || phase === 'working'} aria-label={starting ? tr('Starting…') : pad.label} title={pad.title ?? pad.label}
        disabled={pad.disabled} {...padEvents}>
        {phase === 'preparing' || phase === 'working' ? <ActivityIndicator label={phase === 'preparing' ? tr('Starting…') : tr('Transcribing…')} compact announce={false} /> : <ToolbarIcon name="mic" size={30} />}
      </button>
      <div className="voice-controls">
        <button type="button" className="voice-mini voice-mini-icon" aria-label={tr('Recording settings')} title={tr('Recording settings')}
          aria-haspopup="dialog" aria-expanded={settingsOpen} onClick={() => openSettings()}><ToolbarIcon name="settings" size={15} /></button>
        <MicrophoneLamp phase={phase} health={health ?? null} presence={presence} deviceLabel={deviceLabel} onOpen={() => openSettings()} />
        {desktop && !settingsOpen && microphoneSelector && <div className="voice-microphone">{microphoneSelector}</div>}
        {controls}
      </div>
      <div className="voice-modes" role="radiogroup" aria-label={tr('Recording mode')}>
        {MODES.map(option => {
          const later = laterModes.includes(option)
          return <button key={option} type="button" role="radio" aria-checked={mode === option} aria-label={modeName[option]} title={modeName[option]}
            aria-disabled={later || undefined} data-later={later || undefined} disabled={modesDisabled}
            onClick={() => { if (later) setSoon(true); else onMode(option) }}>{modeShort[option]}</button>
        })}
        {soon && <span className="voice-soon" role="status">{tr('Coming soon')}</span>}
      </div>
    </div>
    <p className="voice-status" role="status" aria-live="polite">{(starting || phase === 'preparing' || phase === 'working') && <span className="activity-spinner" aria-hidden="true" />}{starting ? tr('Starting…') : status}</p>
    {settingsOpen && <DetailDialog title={tr('Recording settings')} capture="preserve" onClose={closeSettings}>
      <div className="voice-settings">
        <h2>{tr('Recording settings')}</h2>
        {microphoneSelector}
        <MicrophoneCheckStart.Provider value={checkAtOnce}>{microphoneCheck}</MicrophoneCheckStart.Provider>
        {settings}
        {layout && <>
          <Choice label={tr('Microphone button')} value={layout.padSide} onChange={layout.onPadSide}
            options={[['left', tr('Left')], ['right', tr('Right')]]} />
          <Choice label={tr('Time direction')} value={layout.time} onChange={layout.onTime}
            options={[['ltr', tr('Time →'), tr('Time runs left to right')], ['rtl', tr('← Time'), tr('Time runs right to left')]]} />
        </>}
      </div>
    </DetailDialog>}
  </section>
}

/** One setting with two or more named values, as a segmented control. */
function Choice<T extends string>({ label, value, options, onChange }: {
  label: string; value: T; options: [T, string, string?][]; onChange: (value: T) => void
}) {
  return <div className="voice-choice">
    <span className="voice-choice-label">{label}</span>
    <SegmentedChoice label={label} value={value} options={options} onChange={onChange} />
  </div>
}

/** Seconds since recording began, for the chip in the stream. */
function useElapsed(running: boolean): number {
  const [elapsed, setElapsed] = useState(0)
  const began = useRef(0)
  useEffect(() => {
    if (!running) return
    began.current = performance.now()
    setElapsed(0)
    const timer = setInterval(() => setElapsed((performance.now() - began.current) / 1000), 250)
    return () => clearInterval(timer)
  }, [running])
  return elapsed
}
