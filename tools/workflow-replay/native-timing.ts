/** Native transport-boundary timing, independent of UI/adoption clocks.
 * Payloads remain opaque: this report does not validate or reproduce responses. */
export type NativeKind = 'text' | 'protocol' | 'speech' | 'transcription' | 'microphone'
export type NativeTiming = {
  kind: NativeKind
  fingerprint: string
  occurrence: number
  startMicros: number
  endMicros: number
  durationMicros: number
  firstRecordedFrameMicros: number | null
  cancelled: boolean
  boundary: 'provider-transport' | 'microphone-capture'
  adoptionMicros: null
  uiCompletionMicros: null
}
function object(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid native timing object.')
  const record = value as Record<string, unknown>
  if (Object.keys(record).length !== keys.length || keys.some(key => !Object.hasOwn(record, key))) throw new Error('Invalid native timing fields.')
  return record
}
function micros(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error('Invalid native timing measurement.')
  return value
}
const kinds = new Set<string>(['text', 'protocol', 'speech', 'transcription', 'microphone'])

/** Limits match the native tape envelope. The first frame is a callback/receipt,
 * not necessarily visible content; total transport time includes local decoding.
 * Never label these durations as adoption, paint, audible playback or pure HTTP time. */
export function nativeTimingReport(bytes: Uint8Array): NativeTiming[] {
  if (bytes.byteLength > 64 * 1024 * 1024) throw new Error('Native tape exceeds timing report limit.')
  const tape = object(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as unknown, ['format', 'records'])
  if (tape.format !== 1 || !Array.isArray(tape.records) || tape.records.length > 10000) throw new Error('Invalid native tape format.')
  const occurrences = new Map<string, number>()
  return tape.records.map((value: unknown) => {
    const record = object(value, ['kind', 'fingerprint', 'start_micros', 'frames', 'complete', 'cancelled_micros'])
    if (typeof record.kind !== 'string' || !kinds.has(record.kind) || typeof record.fingerprint !== 'string' || !/^[a-f0-9]{64}$/.test(record.fingerprint)) throw new Error('Invalid native timing identity.')
    if (record.complete !== true || !Array.isArray(record.frames) || record.frames.length > 100000) throw new Error('Incomplete native timing record.')
    const startMicros = micros(record.start_micros)
    let previous = 0
    let firstRecordedFrameMicros: number | null = null
    for (const value of record.frames) {
      const frame = object(value, ['elapsed_micros', 'payload'])
      const elapsed = micros(frame.elapsed_micros)
      if (elapsed < previous) throw new Error('Native frame timing moved backwards.')
      firstRecordedFrameMicros ??= elapsed
      previous = elapsed
    }
    const cancelled = record.cancelled_micros !== null
    const durationMicros = cancelled ? micros(record.cancelled_micros) : previous
    if (durationMicros < previous || (!cancelled && record.frames.length === 0)) throw new Error('Native terminal timing is missing or invalid.')
    const endMicros = micros(startMicros + durationMicros)
    const key = JSON.stringify([record.kind, record.fingerprint])
    const occurrence = (occurrences.get(key) ?? 0) + 1
    occurrences.set(key, occurrence)
    return {
      kind: record.kind as NativeKind, fingerprint: record.fingerprint, occurrence,
      startMicros, endMicros, durationMicros, firstRecordedFrameMicros, cancelled,
      boundary: record.kind === 'microphone' ? 'microphone-capture' : 'provider-transport',
      adoptionMicros: null, uiCompletionMicros: null,
    }
  })
}
