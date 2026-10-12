import { expect, it } from 'vitest'
import type { ChatMessage, TurnView } from '../../generated/contracts'
import { replyHelp } from './reply-help'
const message: ChatMessage = {
  id: 'm', turnId: 't', sequence: 1, role: 'assistant', text: 'Who joined you?', createdAt: '2026-09-20',
  replacesTurnId: null, replacedBy: null, coachDecision: null,
  wordGloss: null, glossState: null, glossError: null, glossOperationId: null,
  translationState: null, translation: null,
  replyBrief: { explanation: 'Who joined you?' }, briefState: 'succeeded',
  replyExplanations: { cards: [] }, explanationsState: 'succeeded',
}
it('uses published message state and preserves empty saved grammar and hold metadata', () => {
  const turn = {hold:{message:'Budget hold'}} as unknown as TurnView
  const view=replyHelp({...message, explanationsState: 'held'},turn)
  expect(view.grammar?.cards).toEqual([])
  expect(view.lanes.grammar.state).toBe('held')
  expect(view.lanes.grammar.details).toMatchObject({hold:{message:'Budget hold'}})
  expect(view.lanes.replies.state).toBeNull()
})
it('never treats succeeded without its result as a new request', () => {
  const view=replyHelp({...message,replyExplanations:undefined})
  expect(view.lanes.grammar.state).toBe('succeeded')
  expect(view.lanes.grammar.error).toBeTruthy()
})
it('invalidates replaced sources without discarding already saved help', () => {
  const turn={replacedBy:'replacement',operations:[],attempts:[]} as unknown as TurnView
  const view=replyHelp(message,turn)
  expect(view.lanes.grammar.state).toBe('invalidated')
  expect(view.grammar?.cards).toEqual([])
})
