import assert from 'node:assert/strict'
import test from 'node:test'
import { nativeTimingReport } from './native-timing.ts'

const record = () => ({ kind: 'text', fingerprint: 'a'.repeat(64), start_micros: 100, frames: [{ elapsed_micros: 20, payload: { content: 'private text' } }, { elapsed_micros: 80, payload: null }], complete: true, cancelled_micros: null as number | null })
const tape = (records: unknown[]) => Buffer.from(JSON.stringify({ format: 1, records }))
test('native timing separates transport receipt timing from unknown adoption and UI completion', () => {
  const samples = nativeTimingReport(tape([record(), record(), { ...record(), kind: 'microphone' }]))
  assert.deepEqual(samples.map(row => row.occurrence), [1, 2, 1])
  assert.equal(samples[0].durationMicros, 80)
  assert.equal(samples[0].firstRecordedFrameMicros, 20)
  assert.equal(samples[0].endMicros, 180)
  assert.equal(samples[0].adoptionMicros, null)
  assert.equal(samples[0].uiCompletionMicros, null)
  assert.equal(samples[2].boundary, 'microphone-capture')
  assert.ok(!JSON.stringify(samples).includes('private text'))
})
test('native timing checks bounded envelope, kind, clock ordering and terminal facts', () => {
  for (const bad of [
    { ...record(), kind: 'unknown' }, { ...record(), complete: false },
    { ...record(), frames: [] }, { ...record(), unexpected: 1 },
    { ...record(), start_micros: Number.MAX_SAFE_INTEGER },
    { ...record(), cancelled_micros: 1 },
    { ...record(), frames: [...record().frames].reverse() },
  ]) assert.throws(() => nativeTimingReport(tape([bad])))
  assert.throws(() => nativeTimingReport(Buffer.from([255])))
  const cancelled = nativeTimingReport(tape([{ ...record(), frames: [], cancelled_micros: 25 }]))[0]
  assert.equal(cancelled.cancelled, true)
  assert.equal(cancelled.durationMicros, 25)
  assert.equal(cancelled.firstRecordedFrameMicros, null)
})
