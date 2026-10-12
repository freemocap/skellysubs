import { PlaybackProgress } from '../../components/media/PlaybackProgress'
import { DetailDialog } from '../../components/dialogs/DetailDialog'
import { RecordingTrack } from '../../components/media/RecordingTrack'
import { useAnimatedAlignment } from './useAnimatedAlignment'
import { useClipArrival } from './useClipArrival'
import type { ClipPreview } from './useClipPreview'
import { SegmentedChoice } from '../../components/controls/SegmentedChoice'
import { matchTimedWords, wordAlignment } from '../../domain/audio/word-alignment'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { useMemo, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from 'react'
import { useIsMobile } from '../../components/layout/useIsMobile'
import { ResizeHandle, useStoredSize } from '../../components/layout/ResizeHandle'
import { useI18n } from '../../components/localization/i18n'
import { errorMessage } from '../../platform/diagnostics/error-details'
import { ToolbarIcon } from '../../components/controls/ToolbarIcon'
import { sharedScale } from '../../components/media/Spectrogram'
import { DetectionDetails, TimeAxis } from '../../components/media/InspectionTracks'
import type { AudioInspection } from '../../generated/contracts'
import { ActivityIndicator } from '../../components/feedback/ActivityIndicator'

/** Which way time runs across the spectrograms. Right-to-left puts the first
 * sound on the right, where a right-to-left script puts its first word. */
export type TimeDirection = 'ltr' | 'rtl'

/** `fit` stretches each recording across the width; `shared` puts both on one
 * time axis, so their lengths can be compared directly. `words` maps the take's
 * word boundaries to the reference for visual comparison only. */
export type TimeScale = 'fit' | 'shared' | 'words'

/** The phrase and how it should sound, above the selected take: one surface,
 * one time scale choice and one colour scale, so length, rhythm and loudness
 * can be read against each other. Each recording has its own play control
 * beside its own timeline, the way a media player does. */
export function DrillComparison({ target, reference, referenceTime, onSeekReference, onPlayReference, playingReference, referenceNote, referenceFailure,
  attempt, attemptAudioReady = attempt !== null, preview, comparisonAccepted = true, attemptTime = 0, attemptLabel, attemptFailure, onRetryAttempt, attemptUnavailable, direction, onDirection, timeScale, onTimeScale,
  holding, playingAttempt, onPlayAttempt, playbackSpeed, onSeekAttempt, referenceScrub, attemptScrub }: {
  /** The phrase itself, in its reading bubble, or null when no card is
   * selected: the bubble area says so, the plots stay empty frames and
   * nothing can be played. */
  target: ReactNode | null
  playbackSpeed?: ReactNode
  reference: AudioInspection | null
  referenceTime: number
  onSeekReference: (seconds: number) => void
  onPlayReference: () => void
  playingReference: boolean
  /** Why reference playback is the way it is, shown as the play control's tooltip. */
  referenceNote: string
  /** A failed reference request, drawn inside the reference frame. */
  referenceFailure?: ReactNode
  comparisonAccepted?: boolean
  attempt: AudioInspection | null
  attemptAudioReady?: boolean
  preview?: ClipPreview | null
  /** Static previews start at zero; live playback supplies its observed position. */
  attemptTime?: number
  /** The selected take's name, or null when there is no take yet. */
  attemptLabel: string | null
  attemptFailure: unknown
  onRetryAttempt: () => void
  /** Why the selected take has no audio to show, when it has none. */
  attemptUnavailable: string | null
  direction: TimeDirection
  onDirection: (direction: TimeDirection) => void
  timeScale: TimeScale
  onTimeScale: (scale: TimeScale) => void
  holding: boolean
  playingAttempt: boolean
  referenceScrub?: { start: (seconds: number, timestamp?: number) => void; move: (seconds: number, timestamp?: number) => void; end: () => void }
  attemptScrub?: { start: (seconds: number, timestamp?: number) => void; move: (seconds: number, timestamp?: number) => void; end: () => void }
  onSeekAttempt?: (seconds: number) => void
  onPlayAttempt: () => void
}) {
  const tr = useI18n()
  const mobile = useIsMobile()
  const card = target !== null
  const [controlsOpen, setControlsOpen] = useState(false)
  const [showTiming, setShowTiming] = useState(true)
  const alignment = useMemo(() => comparisonAccepted && reference && attempt
    ? wordAlignment(reference.wordTiming, attempt.wordTiming, reference.duration, attempt.duration) : null,
  [comparisonAccepted, reference, attempt])
  const matches = useMemo(() => comparisonAccepted && reference && attempt
    ? matchTimedWords(reference.wordTiming, attempt.wordTiming, reference.duration, attempt.duration) : null, [comparisonAccepted, reference, attempt])
  const aligned = timeScale === 'words' && alignment !== null
  const effectiveScale = timeScale === 'words' && !aligned ? 'fit' : timeScale
  const rawDuration = attempt?.duration ?? preview?.spectrum.endSeconds ?? 1
  const mapTime = useAnimatedAlignment(aligned ? alignment : null, rawDuration, reference?.duration ?? 1, preview?.recordingId ?? attempt?.recordingId)
  const arrival = useClipArrival(preview?.recordingId)
  const attemptDuration = aligned ? reference!.duration : rawDuration
  const attemptSpectrum = attempt?.spectrogram ?? preview?.spectrum.data
  const spectra = [reference?.spectrogram, attemptSpectrum].filter(entry => entry !== undefined)
  const scale = spectra.length ? sharedScale(spectra) : null
  const span = Math.max(reference?.duration ?? 0, attemptSpectrum ? rawDuration : 0, 0.001)
  const width = (duration: number) => `${effectiveScale === 'shared' ? duration / span * 100 : 100}%`
  // Mobile can hide labels while preserving the measured plot.
  const showWords = !mobile || showTiming
  // Each plot keeps the height the learner drags it to. At full width the attempt
  // takes what the reference leaves; stacked, both plots have their own height.
  const [referenceHeight, setReferenceHeight] = useStoredSize('drill-reference')
  const [attemptHeight, setAttemptHeight] = useStoredSize('drill-attempt')
  const referenceTrack = useRef<HTMLDivElement>(null)
  const attemptTrack = useRef<HTMLDivElement>(null)
  const heightOf = (track: RefObject<HTMLDivElement | null>) => () => track.current?.getBoundingClientRect().height ?? 0
  const px = (size: number | null) => size === null ? undefined : `${Math.round(size)}px`

  const controls = <>
    {mobile && <label><input type="checkbox" checked={showTiming} onChange={event => setShowTiming(event.target.checked)} />{tr("Word timing overlays")}</label>}
        <div className="drill-timing-controls">
          <SegmentedChoice label={tr("Time scale")} value={effectiveScale} onChange={onTimeScale} options={[
            ['fit', tr("Fit"), tr("Each recording fills the width")],
            ['shared', tr("Same scale"), tr("One time scale and one colour scale for both")],
            ['words', tr("Align words"), tr("Requires matching word timestamps in both recordings"), !alignment],
          ]} />
          <SegmentedChoice label={tr("Time direction")} value={direction} onChange={onDirection} options={[
            ['ltr', tr("Time →"), tr("Time runs left to right")],
            ['rtl', tr("← Time"), tr("Time runs right to left")],
          ]} />
        </div>
  </>

  return (
    <section className="drill-comparison-panel" aria-label={tr("Reference and your attempt")}
      style={{ '--drill-reference-row': px(referenceHeight), '--drill-attempt-row': px(attemptHeight) } as CSSProperties}>
      <div className="drill-reference">
      <div className="drill-target-card">{target ?? <p className="drill-target-none">{tr("No card selected")}</p>}</div>

      <div className="drill-media">
        <button type="button" className="btn drill-play" aria-label={tr(playingReference ? (mobile && reference ? "Pause" : "Stop") : "Play reference")} disabled={holding || !card} onClick={onPlayReference} title={referenceNote}>
          <ToolbarIcon name={playingReference ? (mobile && reference ? "pause" : "stop") : "play"} size={14} />{tr("Reference")}
        </button>
        {!mobile && playbackSpeed}
        <PlaybackProgress time={referenceTime} duration={reference?.duration ?? 0} displayDuration={effectiveScale === "shared" ? span : reference?.duration ?? 0} direction={direction} label={tr("Seek reference audio")}
          onSeek={holding || !reference ? undefined : onSeekReference} scrub={referenceScrub} />
        {mobile ? <button type="button" className="btn drill-media-options" aria-label={tr('Comparison settings')} aria-haspopup="dialog" aria-expanded={controlsOpen} onClick={() => setControlsOpen(true)}><ToolbarIcon name="settings" size={18} /></button> : controls}
        {controlsOpen && <DetailDialog title={tr("Comparison settings")} capture="preserve" onClose={() => setControlsOpen(false)}>
          <div className="drill-comparison-settings"><h2>{tr("Comparison settings")}</h2>{playbackSpeed}{controls}</div>
        </DetailDialog>}
      </div>

      <div className="drill-timelines" data-time={direction}>
        {reference && scale ? <div className="drill-track" ref={referenceTrack}>
          <RecordingTrack spectrogram={reference.spectrogram} duration={reference.duration} sourceDuration={reference.duration} scale={scale} mapTime={undefined}
            words={showWords ? reference.wordTiming : null} outcomes={matches?.reference ?? []} time={referenceTime} direction={direction} label={tr("Reference")}
            onSeek={holding ? undefined : onSeekReference} onWordSeek={holding ? undefined : onSeekReference} scrub={referenceScrub}
            style={{ width: width(reference.duration) }} plotRef={undefined} attempt={false} />
        </div> : <div className="drill-track" ref={referenceTrack}>
          <div className="drill-plot-frame" data-state={referenceFailure ? 'failed' : playingReference ? 'loading' : 'empty'}>
            {referenceFailure ?? (card && <p role="status">{playingReference ? <ActivityIndicator announce={false} label={tr("Loading reference…")} />
              : attemptLabel ? tr("Play the reference to compare it with this attempt.") : tr("Play the reference once to draw it here.")}</p>)}
          </div>
        </div>}
        <ResizeHandle className="drill-plot-resize drill-reference-resize" label={tr("Resize the reference")} axis="y" grow={1}
          size={referenceHeight} min={96} max={800} measure={heightOf(referenceTrack)} onResize={setReferenceHeight} />
      </div>
      </div>
      <div className="drill-timelines" data-time={direction}>
        <>
          <div className="drill-media drill-media-take">
            <button type="button" className="btn drill-play" aria-label={tr(playingAttempt ? (mobile ? "Pause" : "Stop") : "Play yours")} disabled={!attemptAudioReady || holding} onClick={onPlayAttempt}>
              <ToolbarIcon name={playingAttempt ? (mobile ? "pause" : "stop") : "play"} size={14} />{tr("Attempt")}
            </button>
            <PlaybackProgress time={attemptTime} duration={attempt?.duration ?? 0} displayDuration={effectiveScale === "shared" ? span : attemptDuration} mapTime={mapTime} direction={direction}
              label={tr("Seek attempt audio")} onSeek={holding || !attempt ? undefined : onSeekAttempt} scrub={attemptScrub} />
          </div>
          {attemptSpectrum && scale
            ? <div className="drill-track" ref={attemptTrack}>
              <RecordingTrack spectrogram={attemptSpectrum} duration={attemptDuration} sourceDuration={rawDuration} scale={scale} mapTime={mapTime ?? undefined}
                words={showWords && attempt ? attempt.wordTiming : null} outcomes={matches?.take ?? []} time={attemptTime} direction={direction} label={tr("Attempt")}
                onSeek={holding ? undefined : onSeekAttempt} onWordSeek={undefined} scrub={attemptScrub}
                style={{ width: width(attemptDuration) }} plotRef={arrival} attempt />
            </div>
            : <div className="drill-track" ref={attemptTrack}>
              {card
                ? <div className="drill-plot-frame" data-state={attemptUnavailable ? 'empty' : 'loading'}><p role="status">{attemptUnavailable ?? tr(attemptLabel ? "Loading…" : "No attempts yet. Record one to compare.")}</p></div>
                : <div className="drill-plot-frame" data-state="empty" />}
            </div>}
          <ResizeHandle className="drill-plot-resize drill-attempt-resize" label={tr("Resize the attempt")} axis="y" grow={1}
            size={attemptHeight} min={96} max={800} measure={heightOf(attemptTrack)} onResize={setAttemptHeight} />
        </>

        {effectiveScale === 'shared' && spectra.length > 0 && <div className="drill-track drill-axis"><TimeAxis span={span} /></div>}
      </div>

      {attemptFailure != null && <ErrorNotice as="p" error={attemptFailure}>{errorMessage(attemptFailure)}
        <button type="button" className="btn" onClick={onRetryAttempt}>{tr("Try again")}</button></ErrorNotice>}
      <div className="drill-comparison-foot">{attempt && <details className="drill-info"><summary aria-label={tr("Detection details")}><span aria-hidden="true">i</span></summary><div dir="auto">
        {matches && <p>{aligned && <>{tr("Word-aligned display; playback uses original timing.")}{' '}</>}{tr("Alignment matching ignores case and punctuation.")}</p>}
        {attempt.activity.regions.length > 1 && <p>{tr(" · {value0} speech segments", { value0: String(attempt.activity.regions.length) })}</p>}
        <DetectionDetails activity={attempt.activity} spectrogram={attempt.spectrogram}>
          <p>{tr("This recording is kept until the storage limit removes it.")}</p>
        </DetectionDetails>
      </div></details>}</div>
    </section>
  )
}
