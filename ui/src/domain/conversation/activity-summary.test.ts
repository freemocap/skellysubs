import { expect, it } from 'vitest'
import type { TurnView } from '../../generated/contracts'
import { countWords, humanizeKind, operationPhase, retainedReplyText, turnActivity } from './activity-summary'

import type { InspectionSnapshot } from '../../generated/graph-contracts'
import definition from '../../generated/coach-graph.json'

function turn(nodes: InspectionSnapshot['nodes']): Pick<TurnView, 'nativeGraph' | 'nativePreview'> {
  return { nativeGraph: { ...definition, artifact: definition.artifact as InspectionSnapshot['artifact'],
    engine: 'engine', revision: '1', run: 'run', nodes, activation: {}, paused: false, active: true,
    stepping: null, step_available: false, attempts: {}, reasons: {} } }
}

it('groups every recorded state, including held', () => {
  expect(['waiting_dependencies', 'ready', 'held', 'running', 'succeeded', 'failed', 'unknown', 'cancelled', 'invalidated'].map(operationPhase))
    .toEqual(['waiting', 'waiting', 'held', 'running', 'succeeded', 'failed', 'unknown', 'ended', 'ended'])
  expect(operationPhase('something_new')).toBeNull()
})

it('names operations only by their kind', () => {
  expect(humanizeKind('persona_word_gloss')).toBe('persona word gloss')
})

it('summarizes graph dispositions and leaves unavailable timing unknown', () => {
  expect(turnActivity(turn({ context: 'Adopted', reply: 'Running', translation: 'Running', gloss: 'Waiting', assessment: 'Held', help: 'Unrequested', speech: 'Failed' })))
    .toMatchObject({ total: 7, done: 1, waiting: 1, held: 1, failed: 1, running: ['reply', 'translation'], replyRunning: false, lastFinished: null, settled: false, elapsedMs: null })
  expect(turnActivity(turn({ reply: 'Adopted', help: 'Unrequested' }))).toMatchObject({ settled: true, elapsedMs: null })
})

it('counts only the reply preview associated with the running execution', () => {
  const current = turn({ reply: 'Running' })
  current.nativeGraph!.attempts.reply = [{ id: 'a', execution: 'e', state: 'Running', acquisition: 'Produced' }]
  current.nativePreview = { attempt: 'a', execution: 'e', live: true, complete: false, uncommitted: false, retained_sequence: null,
    capture: { session: 's', sequence: '1', text: '¡Qué bien! Fuiste al mercado.', failure: null } }
  expect(turnActivity(current)).toMatchObject({ replyRunning: true, replyWords: 5 })
  expect(retainedReplyText(current)).toBe('¡Qué bien! Fuiste al mercado.')
  current.nativePreview.execution = 'other'
  expect(turnActivity(current)).toMatchObject({ replyRunning: false, replyWords: null })
})

it('counts words with the platform segmenter, including spaceless scripts', () => {
  expect(countWords('Hola, ¿qué tal?')).toBe(3)
  expect(countWords('我去了市场', 'zh')).toBeGreaterThan(1)
  expect(countWords('')).toBe(0)
})

it('does not invent activity or retained text without graph evidence', () => {
  expect(turnActivity({})).toMatchObject({ total: 0, running: [], replyWords: null, elapsedMs: null })
  expect(retainedReplyText(undefined)).toBeNull()
})
