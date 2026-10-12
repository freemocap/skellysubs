import { expect, it } from 'vitest'
import type { TurnView } from '../../generated/contracts'
import { pipelineSnapshot } from '../../../tests/fixtures/graph-artifacts'
import { turnGraphTarget } from './graph-target'

it('uses native product associations rather than conventional result names', () => {
  const turn = { id: 'turn', nativeGraph: pipelineSnapshot(), nativeSurfaces: { text: ['written'] } } as unknown as TurnView
  expect(turnGraphTarget('chat', turn, 'text')).toEqual({ 'data-graph-scope': 'conversation', 'data-graph-conversation': 'chat', 'data-graph-turn': 'turn', 'data-graph-results': 'written' })
  expect(turnGraphTarget('chat', turn, 'missing')).toEqual({})
  expect(turnGraphTarget(null, turn, 'text')).toEqual({})
  expect(() => turnGraphTarget('chat', { ...turn, nativeSurfaces: { text: ['not-in-artifact'] } }, 'text')).toThrow('Unknown native surface result')
})
