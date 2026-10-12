import assert from 'node:assert/strict'
import test from 'node:test'
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { SessionCapture } from './session.ts'
import { loadBundle } from './bundle-import.ts'
import { preserveCapture, saveRecording } from './save-recording.ts'

test('failed final checkpoint preserves actions, timing and native assets without permitting replay', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'workflow-save-'))
  try {
    const capture = new SessionCapture({ contentConsent: true, build: 'fixture', maxBytes: 100000, maxEvents: 100 })
    capture.setWorkspace(Buffer.from('workspace fixture'))
    capture.append({ kind: 'action', target: 'button', action: 'click', value: '{}' }, 0)
    const providerTape = join(dir, 'native-provider.json')
    await writeFile(providerTape, JSON.stringify({ format: 1, records: [] }))
    let stage = 'start'
    await assert.rejects(saveRecording(capture, dir, () => 1, {
      timings: async () => [
        { kind: 'timing', trace: '1', span: 'read', clock: 'ui', phase: 'start', micros: 0 },
        { kind: 'timing', trace: '1', span: 'read', clock: 'ui', phase: 'end', micros: 250 },
      ],
      stop: async () => ({ providerTape }), text: async () => 'fixture',
      checkpoint: async () => { throw Error('Unhandled browser error.') }, stage: value => { stage = value },
    }), /Unhandled browser error/)
    assert.equal(stage, 'final_checkpoint')
    await preserveCapture(capture, dir, stage)
    const saved = JSON.parse(await readFile(join(dir, 'partial-recording/session.json'), 'utf8'))
    assert.equal(saved.status, 'incomplete')
    assert.equal(saved.events.filter((e: { kind: string }) => e.kind === 'action').length, 1)
    assert.ok(saved.providerTape)
    const timing = JSON.parse(await readFile(join(dir, 'partial-latency.json'), 'utf8'))
    assert.equal(timing.samples[0].durationMicros, 250)
    await assert.rejects(loadBundle(join(dir, 'partial-recording')), /incomplete/)
  } finally { await rm(dir, { recursive: true, force: true }) }
})

test('failure after validation cannot label a partial export complete', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'workflow-save-'))
  try {
    const capture = new SessionCapture({ contentConsent: true, build: 'fixture', maxBytes: 10000, maxEvents: 10 })
    capture.setWorkspace(Buffer.from('workspace'))
    capture.stop()
    await preserveCapture(capture, dir, 'export')
    await assert.rejects(loadBundle(join(dir, 'partial-recording')), /incomplete/)
    assert.equal(capture.snapshot().status, 'complete')
  } finally { await rm(dir, { recursive: true, force: true }) }
})
