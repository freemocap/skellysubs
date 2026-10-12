import { expect, it } from 'vitest'
import { pipelineSnapshot } from '../../../tests/fixtures/graph-artifacts'
import { turnStations } from './turn-stations'

const graph = () => {
  const value = pipelineSnapshot()
  value.artifact.definition.results.audio = { Output: { node: 'read', port: 'text' } }
  return value
}

it.each([
  ['waiting_dependencies', 'waiting'], ['ready', 'waiting'], ['pending', 'waiting'],
  ['held', 'held'], ['running', 'running'], ['succeeded', 'done'],
  ['failed', 'problem'], ['unknown', 'problem'], ['unknown_outcome', 'problem'],
  ['cancelled', 'ended'], ['invalidated', 'ended'], ['unrequested', 'ask'],
])('preserves the reported speech state %s while presenting its tone', (state, tone) => {
  const stations = turnStations(graph(), undefined, { id: 'request', sourceMessageId: 'message', state })
  expect(stations).toEqual([expect.objectContaining({ tone,
    rows: expect.arrayContaining([{ labelKey: 'state', value: { kind: 'text', text: state } }]),
  })])
})

it('still rejects an unsupported state instead of silently claiming success', () => {
  expect(() => turnStations(graph(), undefined, { id: 'request', sourceMessageId: 'message', state: 'unsupported' }))
    .toThrow('Unknown speech request state: unsupported')
})

it('does not invent a playback station without an audio result or speech request', () => {
  const value = graph()
  expect(turnStations(value, undefined, undefined)).toEqual([])
  delete value.artifact.definition.results.audio
  expect(turnStations(value, undefined, { id: 'request', sourceMessageId: 'message', state: 'ready' })).toEqual([])
})
