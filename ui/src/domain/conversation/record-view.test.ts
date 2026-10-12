import { describe, expect, it } from 'vitest'
import type { ChatMessage, ConversationSnapshot, TurnView } from '../../generated/contracts'
import { recordFields, recordMessages, stepProducts } from './record-view'

const message = (fields: Partial<ChatMessage>): ChatMessage => ({
  coachDecision: null, wordGloss: null, glossState: null, glossError: null, glossOperationId: null, turnId: 't', replacesTurnId: null, replacedBy: null,
  id: 'm', sequence: 1, role: 'user', text: 'hola', createdAt: '2026-10-11T14:41:02Z', translation: null, translationState: null, ...fields,
})
const turn = { id: 't', state: 'succeeded', route: 'hosted', paused: false, hold: null, replacesTurnId: null, replacedBy: null, nativeSurfaces: { 'learner.translation': ['learner_translation'], 'reply.translation': ['reply_translation'], audio: ['audio'] }, speech: { sourceMessageId: 'reply', id: 'speech-1', state: 'succeeded' } } as TurnView

describe('recordFields', () => {
  it('names every filled field as the data model does, pairing state and error fields with their owners', () => {
    const fields = recordFields(message({ translation: 'hello', translationState: 'succeeded', glossState: 'failed', glossError: 'boom', assessmentState: 'running' }), turn)
    expect(fields.map(field => [field.name, field.state, field.error, field.value]).sort()).toEqual([
      ['assessmentState', null, null, 'running'],
      ['translation', 'succeeded', null, 'hello'],
      ['wordGloss', 'failed', 'boom', null],
    ])
  })

  it('tags a field with the graph results its turn surface maps to, by the message role', () => {
    const learner = recordFields(message({ translation: 'hello', translationState: 'succeeded' }), turn).find(field => field.name === 'translation')!
    expect(learner.surface).toBe('learner.translation')
    expect(learner.results).toEqual(['learner_translation'])
    const partner = recordFields(message({ role: 'assistant', translation: 'hello', translationState: 'succeeded' }), turn).find(field => field.name === 'translation')!
    expect(partner.results).toEqual(['reply_translation'])
  })

  it('leaves out fields that are empty and have no state', () => {
    expect(recordFields(message({}), null)).toEqual([])
  })
})

describe('recordMessages', () => {
  it('orders messages by sequence and gives the reply its speech request as a field', () => {
    const snapshot = { messages: [message({ id: 'reply', sequence: 2, role: 'assistant' }), message({ id: 'm', sequence: 1 })], turns: [turn] } as unknown as ConversationSnapshot
    const records = recordMessages(snapshot)
    expect(records.map(record => record.id)).toEqual(['m', 'reply'])
    expect(records[1].fields.map(field => [field.name, field.state, field.surface, field.results])).toEqual([['speech', 'succeeded', 'audio', ['audio']]])
  })
})

describe('stepProducts', () => {
  it('finds the message fields a step fills through the turn surfaces, and the reply text for the text result', () => {
    const view = { results: [{ port: 'learner_translation', value: { kind: 'wired', producer: { node: 'translate' } } }, { port: 'text', value: { kind: 'wired', producer: { node: 'reply' } } }, { port: 'audio', value: { kind: 'wired', producer: { node: 'speech' } } }] }
    const surfaces = { ...turn, nativeSurfaces: { 'learner.translation': ['learner_translation'], text: ['text'], audio: ['audio'] } }
    const messages = [message({ translation: 'hello', translationState: 'succeeded' }), message({ id: 'reply', sequence: 2, role: 'assistant', text: 'hola de nuevo' })]
    expect(stepProducts(view, surfaces, messages, 'translate')).toEqual([{ result: 'learner_translation', surface: 'learner.translation', role: 'user', field: 'translation', value: 'hello' }])
    expect(stepProducts(view, surfaces, messages, 'reply')).toEqual([{ result: 'text', surface: 'text', role: 'assistant', field: 'text', value: 'hola de nuevo' }])
    expect(stepProducts(view, surfaces, messages, 'speech').map(product => product.field)).toEqual(['speech'])
    expect(stepProducts(view, surfaces, messages, 'other')).toEqual([])
  })
})
