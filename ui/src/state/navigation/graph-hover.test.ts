import { beforeEach, expect, it } from 'vitest'
import type { GraphHoverTarget } from '../../domain/activity/graph-surface'
import { useGraphHoverStore } from './graph-hover'

const target = (result: string): GraphHoverTarget => ({ key: { scope: 'conversation', conversationId: 'chat', turnId: 'run' }, results: [result] })
const show = (session: string, seq: number, value: GraphHoverTarget | null) => useGraphHoverStore.getState().show({ session, seq, replyTo: 'ai', target: value })

beforeEach(() => useGraphHoverStore.setState({ target: null, origin: null, latest: {}, replies: {} }))

it('moves from one node to the next and clears on leave', () => {
  show('a', 1, target('first'))
  show('a', 2, target('second'))
  expect(useGraphHoverStore.getState().target).toEqual(target('second'))
  show('a', 3, null)
  expect(useGraphHoverStore.getState().target).toBeNull()
})

it('drops a delayed hover that arrives after the clear that followed it', () => {
  show('a', 2, null)
  show('a', 1, target('late'))
  expect(useGraphHoverStore.getState().target).toBeNull()
})

it('lets one view clear only its own hint', () => {
  show('a', 1, target('mine'))
  show('b', 1, null)
  expect(useGraphHoverStore.getState().target).toEqual(target('mine'))
})

it('keeps a reply only for the latest message of its session', () => {
  show('a', 1, target('first'))
  show('a', 2, target('second'))
  useGraphHoverStore.getState().reply({ session: 'a', seq: 1, matched: 1, visible: 1 })
  expect(useGraphHoverStore.getState().replies).toEqual({})
  useGraphHoverStore.getState().reply({ session: 'a', seq: 2, matched: 2, visible: 0 })
  expect(useGraphHoverStore.getState().replies.a).toEqual({ session: 'a', seq: 2, matched: 2, visible: 0 })
})
