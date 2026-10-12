import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useI18n } from '../../../components/localization/i18n'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import type { MicrophoneHealth } from '../../../domain/audio/microphone-health'
import { VoicePanel } from '../../../components/media/VoicePanel'
import type { RecorderLayout } from '../../../components/media/useRecorderLayout'
interface ComposerInputProps {
  transcriptionWarning?: string
  /** The live stream while recording, the waveform over the spectrogram; it fills the panel's face. */
  stream?: ReactNode
  /** What the empty face says instead of “Press the microphone to start”, such
   * as the greeting to say in a new conversation. */
  prompt?: ReactNode
  micShortcut?: string
  input: string
  available: boolean
  sending: boolean
  starting?: boolean
  health?: MicrophoneHealth | null
  deviceLabel?: string | null
  recording: boolean
  transcribing: boolean
  autoSend: boolean
  onAutoSend?: (enabled: boolean) => void
  mode?: 'tap' | 'hold'
  onMode?: (mode: 'tap' | 'hold') => void
  onHoldStart?: () => void
  onHoldEnd?: () => void
  /** Shared microphone picker, shown in the footer or recording settings. */
  microphoneSelector?: ReactNode
  /** The local check row, in recording settings under the picker. */
  microphoneCheck?: ReactNode
  /** The saved device choice, so the recorder's lamp can say when it is not connected. */
  device?: string | null
  /** The last recording never rose above the floor; the face offers the check. */
  silentTake?: boolean
  onDismissSilentTake?: () => void
  /** The pad's side and the stream's direction, each set in the recording settings. */
  layout?: RecorderLayout
  targetLanguageTag?: string
  targetLanguageName: string
  onInput: (value: string) => void
  onSend: (value: string) => void
  onDiscardRecording: () => void
  onToggleRecording: () => void
}

/** Chat's voice panel: microphone first, typing on request. A transcript lands
 * in the face as an editable draft with its own Send; Auto-send skips the draft.
 * Recording and request ownership stay with the caller. Chat's Auto (pause to
 * finish each line) needs native work, so it is shown and marked “Coming soon”. */
export function ComposerInput({ input, available, sending, starting = false, health, deviceLabel, recording, transcribing, autoSend, onAutoSend, mode = 'tap', onMode, onHoldStart, onHoldEnd, microphoneSelector, microphoneCheck, device, silentTake, onDismissSilentTake, layout,
  transcriptionWarning, targetLanguageTag, targetLanguageName, stream, prompt, micShortcut, onInput, onSend, onDiscardRecording, onToggleRecording,
}: ComposerInputProps) {
  const tr = useI18n()
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const [typing, setTyping] = useState(false)
  // The draft shows while there is text or the learner asked to type; the
  // stream takes its place while the microphone records.
  const drafting = !recording && (typing || input.length > 0)
  useEffect(() => { if (typing) inputRef.current?.focus({ preventScroll: true }) }, [typing])
  const canSend = available && !sending && !starting && !recording && !transcribing && input.trim().length > 0
  const padLabel = recording ? (autoSend ? tr("Stop and send recording") : tr("Stop and transcribe recording")) : tr("Record audio")
  const draft = <form className="voice-draft" onSubmit={event => { event.preventDefault(); if (canSend) onSend(input) }}>
    <textarea
      ref={inputRef}
      rows={1}
      aria-label={tr("Message")}
      title={tr("Shift+Enter: new line")}
      onKeyDown={event => {
        if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing && event.keyCode !== 229) {
          event.preventDefault(); event.currentTarget.form?.requestSubmit()
        }
      }}
      className="field composer-input voice-text"
      value={input}
      onChange={(e) => onInput(e.target.value)}
      placeholder={targetLanguageName ? tr("Write in {value0}…", { value0: String(targetLanguageName) }) : tr("Write…")}
      disabled={!available}
      lang={targetLanguageTag}
      dir="auto"
      enterKeyHint="send"
      autoCorrect="off"
      spellCheck={false}
    />
    <div className="voice-draft-actions">
      <button type="button" className="voice-draft-discard" aria-label={tr("Discard")} title={tr("Discard")} disabled={!input.trim() || sending}
        onClick={() => { onInput(''); setTyping(false) }}><ToolbarIcon name="trash" size={16} /></button>
      <button type="submit" className="voice-send" aria-label={tr("Send")} title={tr("Enter: send")} disabled={!canSend}>↑</button>
    </div>
  </form>
  return (
    <>
      <VoicePanel label={tr("Message")} className="composer-voice" health={health} deviceLabel={deviceLabel} starting={starting} phase={starting ? 'preparing' : recording ? 'recording' : transcribing ? 'working' : 'ready'}
        face={recording ? stream ?? null : drafting ? draft : null} prompt={prompt}
        mode={mode} onMode={next => { if (next !== 'auto') onMode?.(next) }} laterModes={['auto']} modesDisabled={starting || recording || transcribing || !onMode}
        onDiscard={onDiscardRecording}
        pad={{ label: padLabel, title: micShortcut ? `${padLabel} · ${micShortcut}` : padLabel, disabled: !available || sending || transcribing || (starting && mode !== 'hold'),
          action: mode === 'hold' && onHoldStart && onHoldEnd ? { kind: 'hold', onHoldStart, onHoldEnd } : { kind: 'press', onPress: onToggleRecording } }}
        microphoneSelector={microphoneSelector} microphoneCheck={microphoneCheck} device={device} silentTake={silentTake} onDismissSilentTake={onDismissSilentTake} layout={layout}
        controls={<>
          <button type="button" className="voice-mini voice-type" aria-pressed={drafting} disabled={starting || recording || !available}
            onClick={() => setTyping(value => !value)}><ToolbarIcon name="keyboard" size={15} /><span>{tr("Type")}</span></button>
          <label className="voice-switch"><input type="checkbox" checked={autoSend} disabled={!onAutoSend}
            onChange={event => onAutoSend?.(event.target.checked)} />{tr("Auto-send")}</label>
        </>} />
      {transcriptionWarning && <div className="transcription-warning" role="note">{transcriptionWarning}</div>}
    </>
  )
}
