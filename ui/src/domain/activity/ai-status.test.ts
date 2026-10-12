import { expect, it } from 'vitest'
import type { Disposition, InspectionSnapshot } from '../../generated/graph-contracts'
import definition from '../../generated/coach-graph.json'
import english from '../localization/locales/english.json'
import { aiStatus, STEP_WORDS, type AiStatusInput } from './ai-status'

type Turn = AiStatusInput['turns'][number]

function turn(id: string, nodes: Record<string, Disposition>, reply?: string): Turn {
  const graph: InspectionSnapshot = { ...definition, engine: 'engine', revision: '1', run: id,
    nodes, activation: {}, paused: false, active: true, stepping: null, step_available: false, attempts: {}, reasons: {},
    artifact: definition.artifact as InspectionSnapshot['artifact'] }
  if (reply) graph.attempts[reply] = [{ id: 'attempt', execution: 'execution', state: 'Running', acquisition: 'Produced' }]
  return { id, nativeGraph: graph, nativePreview: reply ? {
    attempt: 'attempt', execution: 'execution', capture: { session: 'session', sequence: '1', text: '', failure: null },
    live: true, complete: false, retained_sequence: null, uncommitted: false,
  } : undefined }
}

function input(overrides: Partial<AiStatusInput> = {}): AiStatusInput {
  return { transcribing: false, scheduling: false, turns: [], audio: null, connection: 'connected',
    models: { transcription: 'whisper-large-v3', speech: 'voice-model' }, ...overrides }
}

it('names nothing while the AI is idle', () => {
  expect(aiStatus(input())).toEqual({ busy: false, line: null })
})

it('reports transcription as a learner-started step, with the transcription model', () => {
  expect(aiStatus(input({ transcribing: true }))).toEqual({ busy: true, line: {
    id: 'transcribe', tone: 'work', announce: true, kinds: ['speech_transcription'], models: ['whisper-large-v3'],
    words: ['Transcribing recorded audio…', 'Transcribing audio…', 'Transcribing…'],
  } })
  expect(aiStatus(input({ transcribing: true, models: { transcription: null, speech: null } })).line?.models).toEqual([])
})

it('reports a send as scheduling a turn until native storage holds it', () => {
  const { busy, line } = aiStatus(input({ scheduling: true }))
  expect(busy).toBe(true)
  expect(line).toMatchObject({ id: 'schedule', announce: true, words: ['Scheduling reply turn…', 'Scheduling turn…', 'Scheduling…'] })
})

it('identifies a streaming reply by its attempt and execution, ahead of sibling work', () => {
  const running = turn('t1', { reply: 'Running', feedback: 'Running' }, 'reply')
  expect(aiStatus(input({ turns: [running] })).line).toMatchObject({
    id: 'request:t1:reply', announce: true, kinds: ['reply'], models: [], words: STEP_WORDS.replyRequest,
  })
  running.nativePreview!.capture.text = 'Hola'
  expect(aiStatus(input({ turns: [running] })).line).toMatchObject({ id: 'stream:t1:reply', announce: false, words: STEP_WORDS.replyStream })
  running.channel = 'coach'
  expect(aiStatus(input({ turns: [running] })).line?.words).toEqual(STEP_WORDS.coachStream)
  running.nativePreview!.execution = 'another-execution'
  expect(aiStatus(input({ turns: [running] })).line).toMatchObject({ words: null, kinds: ['reply', 'feedback'] })
})

it('reports concurrent nodes without inventing a model or start order', () => {
  const running = turn('t1', { reply: 'Adopted', translation: 'Running', gloss: 'Running', brief: 'Waiting' })
  expect(aiStatus(input({ turns: [running] }))).toEqual({ busy: true, line: {
    id: 'run:t1:translation:gloss', tone: 'work', announce: false, kinds: ['translation', 'gloss'], models: [], words: null,
  } })
})

it('reads the newest turn that has running work', () => {
  expect(aiStatus(input({ turns: [turn('t2', { reply: 'Adopted' }), turn('t1', { gloss: 'Running' })] })).line?.kinds).toEqual(['gloss'])
})

it('names an unfamiliar executable node without a separate catalog', () => {
  expect(aiStatus(input({ turns: [turn('t1', { new_check: 'Running' })] })).line).toMatchObject({ words: null, kinds: ['new_check'] })
})

it.each<Disposition>(['Ready', 'Prepared', 'Available'])('reports local work awaiting dispatch or adoption: %s', state => {
  expect(aiStatus(input({ turns: [turn('t1', { node: state })] }))).toMatchObject({ busy: true, line: { id: 'dispatch' } })
})

it.each<Disposition>(['Held', 'Paused', 'Unrequested', 'Disabled', 'Skipped', 'Blocked', 'Failed', 'Unknown', 'Cancelled', 'Adopted'])('does not call inactive work busy: %s', state => {
  expect(aiStatus(input({ turns: [turn('t1', { node: state })] }))).toEqual({ busy: false, line: null })
})

it('reports requested speech and prioritizes playback buffering over graph work', () => {
  expect(aiStatus(input({ audio: 'partner' }))).toMatchObject({ busy: true, line: { id: 'synthesize', models: ['voice-model'] } })
  expect(aiStatus(input({ turns: [turn('t1', { synthesis: 'Running' })], audio: 'buffering' })).line?.id).toBe('buffering')
})

it('reports a practice card’s audio as fetched, since native may answer it from its cache', () => {
  expect(aiStatus(input({ audio: 'card' }))).toEqual({ busy: true, line: {
    id: 'card-audio', tone: 'work', announce: false, kinds: [], models: ['voice-model'],
    words: ['Fetching card audio…', 'Fetching audio…', 'Fetching…'],
  } })
})

it('reports the connection only while no work runs, and never as work', () => {
  expect(aiStatus(input({ connection: 'checking' }))).toEqual({ busy: false, line: {
    id: 'check', tone: 'check', announce: false, kinds: [], models: [], words: ['Checking AI connection…', 'Checking connection…', 'Checking…'],
  } })
  expect(aiStatus(input({ connection: 'disconnected' })).line).toMatchObject({ id: 'offline', tone: 'offline', words: ['AI not connected', 'Not connected', 'Disconnected'] })
  expect(aiStatus(input({ connection: 'disconnected', transcribing: true })).line?.id).toBe('transcribe')
})

it('words every status in at most three words, then two, then one, each a catalog message', () => {
  const catalog = english as Record<string, unknown>
  for (const [kind, tiers] of Object.entries(STEP_WORDS)) {
    tiers.forEach((key, tier) => {
      expect(catalog[key], `${kind}: ${key}`).toBe(key)
      const words = key.replace('…', '').trim().split(/\s+/).length
      expect(words, `${kind}: ${key}`).toBeLessThanOrEqual(3 - tier)
    })
  }
})
