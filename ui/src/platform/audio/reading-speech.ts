import type { ReadingInput, ReadingResult, SpeechAlignment } from '../../generated/contracts'
import { readSelection } from '../ipc/reading'
import { playSpeechAudio, type PlaybackObserver } from './speech-player'
import { interruptSpeech, onSpeechInterrupted, speechPlaybackPermit } from './speech'

async function withPlayback<T>(signal: AbortSignal, work: (lifetime: AbortSignal, permit: object) => Promise<T>): Promise<T> {
  signal.throwIfAborted()
  const permit = interruptSpeech()
  if (!permit) throw new Error('Speech playback is currently unavailable.')
  const interrupted = new AbortController()
  const stopListening = onSpeechInterrupted(() => interrupted.abort())
  const cancel = () => interrupted.abort(signal.reason)
  signal.addEventListener('abort', cancel, { once: true })
  try { return await work(interrupted.signal, permit) } finally {
    interrupted.abort()
    signal.removeEventListener('abort', cancel)
    stopListening()
  }
}

async function play(base64: string, signal: AbortSignal, permit: object, onPlayback: () => void, rate: number, volume: number, observer?: PlaybackObserver, alignment?: SpeechAlignment | null): Promise<void> {
  signal.throwIfAborted()
  if (speechPlaybackPermit() !== permit) throw new DOMException('Speech was stopped', 'AbortError')
  onPlayback()
  await new Promise<void>((resolve, reject) => {
    const finish = () => { signal.removeEventListener('abort', cancel); resolve() }
    const fail = (error: Error) => { signal.removeEventListener('abort', cancel); reject(error) }
    const player = playSpeechAudio({ mime: 'audio/wav', audioBase64: base64, alignment }, finish, fail, rate, volume, observer)
    const cancel = () => { player.stop(); finish() }
    signal.addEventListener('abort', cancel, { once: true })
    if (signal.aborted) cancel()
    else void player.play().catch(error => { player.stop(); fail(error) })
  })
  signal.throwIfAborted()
}

/** Replay retained audio with the same cancellation and exclusion as fetched speech. */
export function replaySelectionAudio(base64: string, signal: AbortSignal, onPlayback: () => void, rate: number, volume: number, observer?: PlaybackObserver, alignment?: SpeechAlignment | null): Promise<void> {
  return withPlayback(signal, (lifetime, permit) => play(base64, lifetime, permit, onPlayback, rate, volume, observer, alignment))
}

/** Fetch once; callers may keep the result and replay it without another request. */
export function speakSelection(input: ReadingInput, signal: AbortSignal, onPlayback: () => void, rate: number, volume: number, observer?: PlaybackObserver & { onAudio?: (result: ReadingResult) => Promise<void> }): Promise<ReadingResult> {
  return withPlayback(signal, async (lifetime, permit) => {
    const result = await readSelection(input, lifetime)
    lifetime.throwIfAborted()
    if (!result.audioBase64) throw new Error('The speech service returned no audio.')
    // Inspection supplies display data; ready audio does not depend on it.
    await Promise.all([
      play(result.audioBase64, lifetime, permit, onPlayback, rate, volume, { ...observer, sourceText: input.text }, result.audioAlignment),
      Promise.resolve().then(() => observer?.onAudio?.(result)),
    ])
    lifetime.throwIfAborted()
    return result
  })
}
