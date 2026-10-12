import { expect, it } from 'vitest'
import { fixCount } from './fixes'

const turns = new Map([
  { id: 'first', replacesTurnId: null },
  { id: 'second', replacesTurnId: 'first' },
  { id: 'third', replacesTurnId: 'second' },
  { id: 'other', replacesTurnId: null },
].map(turn => [turn.id, turn]))

it('counts the fixes behind a message: each revision sent in place of the one before', () => {
  expect(fixCount('first', turns)).toBe(0)
  expect(fixCount('second', turns)).toBe(1)
  expect(fixCount('third', turns)).toBe(2)
  expect(fixCount('other', turns)).toBe(0)
  expect(fixCount(undefined, turns)).toBe(0)
})

it('counts a replaced turn older than the loaded window, then stops', () => {
  expect(fixCount('late', new Map([['late', { replacesTurnId: 'unloaded' }]]))).toBe(1)
})

it('never loops on a malformed chain', () => {
  expect(fixCount('a', new Map([['a', { replacesTurnId: 'b' }], ['b', { replacesTurnId: 'a' }]]))).toBe(2)
})
