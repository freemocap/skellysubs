// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { graphTargetAttributes, matchesTarget, parseGraphHoverMessage, parseGraphHoverReply, parseGraphSurfaceKey, sameSurface, type GraphSurfaceKey } from './graph-surface'

const turn: GraphSurfaceKey = { scope: 'conversation', conversationId: 'chat', turnId: 'run' }
const reading: GraphSurfaceKey = { scope: 'workspace', engine: 'engine', run: 'reading' }

function dataset(attributes: Record<string, string>): Record<string, string> {
  const element = document.createElement('div')
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value)
  return { ...element.dataset } as Record<string, string>
}

it('round-trips a hover message and rejects malformed fields', () => {
  const message = { session: 's', seq: 3, replyTo: 'ai', target: { key: turn, results: ['text', 'notes'] } }
  expect(parseGraphHoverMessage(message)).toEqual(message)
  expect(parseGraphHoverMessage({ ...message, target: null }).target).toBeNull()
  expect(() => parseGraphHoverMessage({ ...message, seq: -1 })).toThrow('sequence')
  expect(() => parseGraphHoverMessage({ ...message, target: { key: turn, results: [''] } })).toThrow('result')
  expect(() => parseGraphHoverMessage({ ...message, target: { key: turn, results: Array(65).fill('x') } })).toThrow('results')
  expect(() => parseGraphSurfaceKey({ scope: 'elsewhere' })).toThrow('scope')
  expect(() => parseGraphHoverReply({ session: 's', seq: 1, matched: 1.5, visible: 0 })).toThrow('match count')
})

it('matches only elements of the same run that show one of the results', () => {
  const target = { key: turn, results: ['text'] }
  expect(matchesTarget(dataset(graphTargetAttributes(turn, ['notes', 'text'])), target)).toBe(true)
  expect(matchesTarget(dataset(graphTargetAttributes(turn, ['notes'])), target)).toBe(false)
  expect(matchesTarget(dataset(graphTargetAttributes({ ...turn, turnId: 'other' }, ['text'])), target)).toBe(false)
  expect(matchesTarget(dataset(graphTargetAttributes(reading, ['text'])), target)).toBe(false)
  expect(matchesTarget(dataset(graphTargetAttributes(reading, ['text'])), { key: reading, results: ['text'] })).toBe(true)
})

it('refuses targets that name no result or a result with spaces', () => {
  expect(() => graphTargetAttributes(turn, [])).toThrow('at least one')
  expect(() => graphTargetAttributes(turn, ['two words'])).toThrow('spaces')
})

it('compares surfaces by scope and identity', () => {
  expect(sameSurface(turn, { ...turn })).toBe(true)
  expect(sameSurface(turn, reading)).toBe(false)
  expect(sameSurface(reading, { ...reading, run: 'other' })).toBe(false)
})
