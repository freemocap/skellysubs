import { describe, expect, it } from 'vitest'
import type { ChatMessage, ConversationSnapshot } from '../../generated/contracts'
import { conversationTurns } from './conversation-view'

function message(sequence: number, role: string, text: string, translation: string | null = null): ChatMessage {
  return { coachDecision: null, turnId: `turn-${Math.ceil(sequence / 2)}`, replacesTurnId: null, replacedBy: null, wordGloss: null, glossError: null, glossState: null, glossOperationId: null, sequence, role, text, translation, translationState: translation ? 'succeeded' : null, id: `source-${sequence}`, createdAt: '2026-09-10' }
}
function snapshot(messages: ChatMessage[]): ConversationSnapshot {
  return {
    opening: null, topicChoices: [], starterGreeting: { text: 'hola', romanized: null }, revisionSuffixCounts: [], messages, turns: [], coachMessages: [], transcriptionAttempts: [],
    conversationId: 'conversation', sessionId: 'session', revision: 1, contentRevision: '1', hasOlder: false,
    connection: { route: 'hosted', signedIn: true, email: '', revision: 1, configured: true, assessmentAdapter: 'jev_choice' as const, standardModel: 'google/gemini-2.5-flash', fastModel: '', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false },
  }
}

describe('durable conversation projection', () => {
  it('keeps the learner source identity and exposes pending assessment independently of coaching', () => {
    const source = snapshot([message(1, 'user', 'Hola')])
    source.messages[0].assessmentState = 'running'
    const turn = conversationTurns(source)[0]
    expect(turn.userMessageId).toBe('source-1')
    expect(turn.analysisState).toBe('pending')
    expect(turn.conversationFeedback).toBeUndefined()
  })
  it('preserves exact multilingual text and saved translation without manufacturing analysis', () => {
    const source = snapshot([
      message(1, 'user', '  Sí, sí!\ne\u0301 你好 👩🏽‍💻  '),
      message(2, 'assistant', 'مرحبا\n你好。', 'Hello\nHello.'),
    ])
    const original = structuredClone(source)
    const projected = conversationTurns(source)
    expect(projected).toHaveLength(1)
    expect(projected[0].user).toBe(source.messages[0].text)
    expect(projected[0].assistant?.reply).toBe(source.messages[1].text)
    expect(projected[0].assistant?.translation).toBe('Hello\nHello.')
    expect(projected[0].analysisState).toBeNull()
    expect(projected[0].assistant).toMatchObject({ tokens: [], user_tokens: [], user_translation: null, mechanics: [], scaffolds: { replies: [], frames: [], starters: [] }, suggestionsState: null, errors: [] })
    expect(source).toEqual(original)
  })

  it('hydrates translation independently without changing source or older projections', () => {
    const initial = snapshot([message(1, 'user', 'Question'), message(2, 'assistant', 'Reply')])
    const before = conversationTurns(initial)
    const after = conversationTurns({ ...initial, revision: 2, messages: [initial.messages[0], { ...initial.messages[1], translation: 'Saved meaning', translationState: 'succeeded' }] })
    expect(before[0].assistant?.translation).toBeNull()
    expect(after[0].assistant?.translation).toBe('Saved meaning')
    expect(after[0].assistant?.reply).toBe(before[0].assistant?.reply)
    expect(after[0].id).toBe(before[0].id)
    expect(after[0].analysisState).toBeNull()
  })

  it('preserves an assistant at a page boundary and a pending learner message', () => {
    const result = conversationTurns(snapshot([message(100, 'assistant', 'Previous reply'), message(101, 'user', 'Next question')]))
    expect(result.map(turn => ({ id: turn.id, user: turn.user, reply: turn.assistant?.reply ?? null }))).toEqual([
      { id: 100, user: null, reply: 'Previous reply' },
      { id: 101, user: 'Next question', reply: null },
    ])
    expect(result.every(turn => turn.analysisState === null)).toBe(true)
  })

  it('does not include private coach messages in persona turns', () => {
    const source = snapshot([message(1, 'user', 'Persona question')])
    source.coachMessages = [message(2, 'assistant', 'Private coaching')]
    expect(conversationTurns(source)).toMatchObject([{ id: 1, user: 'Persona question', assistant: null, analysisState: null }])
  })

  it('fails on an unexpected native role rather than dropping content', () => {
    expect(() => conversationTurns(snapshot([message(1, 'system', 'Unexpected')]))).toThrow('Unexpected conversation message role.')
  })
})

it('passes native feedback through unchanged and projects suggestion state and failure', () => {
  const user: ChatMessage = { ...message(1, 'user', 'Yo fue ayer'), feedbackState: 'succeeded',
    feedback: { issues: [], corrections: [], notes: [], meaningRecovered: 'full', items: [], candidatesSent: 18, itemsReturned: 0 } }
  const reply: ChatMessage = { ...message(2, 'assistant', '¿Adónde fuiste?'), suggestedReplies: [{ text: 'Fui al mercado.', segments: [] }], suggestionsState: 'failed', suggestionsError: 'Coach feedback rejected: suggestions_schema.' }
  const [turn] = conversationTurns(snapshot([user, reply]))
  expect(turn.coach).toBe(user.feedback)
  expect(turn.analysisState).toBe('done')
  expect(turn.assistant).toMatchObject({ scaffolds: { replies: [{ text: 'Fui al mercado.', segments: [] }] }, suggestionsState: 'failed', errors: ['Coach feedback rejected: suggestions_schema.'] })
})

it('projects saved source gloss and independent operation state without token reconstruction', () => {
  const source = message(2, 'assistant', 'Hola hola')
  source.wordGloss = {sourceMessageId: source.id, targetLanguageId:'spanish', explanationLanguageId:'english', formatVersion:'format', templateVersion:'template', boundaryPolicy:'policy', operationId:'gloss', attemptId:'attempt', coverage:'partial', segments:[{start:5,end:9,kind:'gloss',gloss:'hello'}]}
  source.glossOperationId = 'gloss'
  source.glossState = 'failed'
  source.glossError = 'Word meanings request failed.'
  const projected = conversationTurns(snapshot([source]))[0].assistant!
  expect(projected.savedGloss).toBe(source.wordGloss)
  expect(projected).toMatchObject({reply:'Hola hola',tokens:[],glossOperationId:'gloss',glossState:'failed',glossError:source.glossError})
  source.wordGloss = {...source.wordGloss, sourceMessageId: 'another-message'}
  expect(() => conversationTurns(snapshot([source]))).toThrow('Saved word meanings do not belong to this message.')
})

it.each([null, 'ready', 'waiting_dependencies', 'running', 'succeeded', 'failed', 'unknown', 'cancelled', 'invalidated'])('preserves authoritative translation state %s independently of saved text', state => {
  const reply = { ...message(2, 'assistant', 'Reply', 'Saved translation'), translationState: state }
  const source = snapshot([message(1, 'user', 'Question'), reply])
  const before = structuredClone(source)
  const result = conversationTurns(source)[0].assistant!
  expect(result.translationState).toBe(state)
  expect(result.translation).toBe('Saved translation')
  expect(result.reply).toBe('Reply')
  expect(result.glossState).toBeNull()
  expect(result.errors).toEqual([])
  expect(source).toEqual(before)
})

it('projects human reading independently of the reply and rejects another source', () => {
  const human = message(1, 'user', 'Hola', 'Hello')
  human.wordGloss = { sourceMessageId: human.id, targetLanguageId:'spanish', explanationLanguageId:'english', formatVersion:'v1', templateVersion:'v1', boundaryPolicy:'v1', operationId:'human-gloss', attemptId:'attempt', coverage:'complete', segments:[{ start:0, end:4, kind:'gloss', gloss:'hello' }] }
  const result = conversationTurns(snapshot([human]))[0]
  expect(result.userSavedGloss).toBe(human.wordGloss)
  expect(result.userTranslation).toBe('Hello')
  expect(result.userTranslationState).toBe('succeeded')
  expect(conversationTurns(snapshot([{ ...human, translation: null, translationState: 'failed' }]))[0].userTranslationState).toBe('failed')
  expect(result.assistant).toBeNull()
  human.wordGloss.sourceMessageId = 'different'
  expect(() => conversationTurns(snapshot([human]))).toThrow('Saved word meanings do not belong to this message.')
})

it('groups by durable identity despite interleaving and retains repeated revision links', () => {
  const source = snapshot([
    { ...message(1, 'user', 'Original'), turnId: 'first', replacedBy: 'second' },
    { ...message(2, 'user', 'Repair'), turnId: 'second', replacesTurnId: 'first', replacedBy: 'third' },
    { ...message(3, 'assistant', 'Original reply'), turnId: 'first', replacedBy: 'second' },
    { ...message(4, 'assistant', 'Repair reply'), turnId: 'second', replacesTurnId: 'first', replacedBy: 'third' },
    { ...message(5, 'user', 'Latest'), turnId: 'third', replacesTurnId: 'second' },
  ])
  expect(conversationTurns(source)).toMatchObject([
    { turnId: 'first', user: 'Original', assistant: { reply: 'Original reply' }, replacedBy: 'second' },
    { turnId: 'second', user: 'Repair', assistant: { reply: 'Repair reply' }, replacesTurnId: 'first', replacedBy: 'third' },
    { turnId: 'third', user: 'Latest', replacesTurnId: 'second' },
  ])
})

it('retains reply failure and pause state independently of successful saved assistance', () => {
  const source = snapshot([message(1, 'user', 'Question')])
  source.turns = [{ id: 'turn-1', state: 'failed', paused: false, hold: null, route: 'hosted', replacesTurnId: null, replacedBy: null,   }]
  source.messages[0].feedback = { issues: [], corrections: [], notes: [], meaningRecovered: 'full', items: [], candidatesSent: 1, itemsReturned: 0 }
  expect(conversationTurns(source)[0]).toMatchObject({ analysisState: 'done', assistant: null, replyState: { state: 'failed', control: 'retry' } })
  source.turns[0].state = 'pending'; source.turns[0].paused = true
  expect(conversationTurns(source)[0]).toMatchObject({ analysisState: 'done', replyState: { state: 'paused', control: 'resume' } })
})

it('projects independent support on its own exchange without manufacturing skill evidence', () => {
  const feedback={answers:{},grammar:10,conversation:10}
  const assistance={explanation:'A greeting.',replies:[],frames:['Soy ___.'],starters:['Hola…']}
  const cards={cards:[{quote:'Hola',title:'Greeting',body:'A greeting.',example:'Hola, Ana.',contrast:''}]}
  const turns=conversationTurns(snapshot([
    {...message(1,'user','Hola'),conversationFeedback:feedback},
    {...message(2,'assistant','Hola'),replyAssistance:assistance,replyExplanations:cards,explanationsState:'succeeded'},
    message(3,'user','Otro mensaje'),
  ]))
  expect(turns[0].conversationFeedback).toEqual(feedback)
  expect(turns[0].coach).toBeUndefined()
  expect(turns[0].analysisState).toBe('done')
  expect(turns[0].assistant?.assistance).toEqual(assistance)
  expect(turns[0].assistant?.mechanics[0].quote).toBe('Hola')
  expect(turns[1].conversationFeedback).toBeUndefined()
})

it('counts replacement history from the same loaded executions used for display', () => {
  const source = snapshot([message(1, 'user', 'Earlier'), message(3, 'user', 'Revised'), message(5, 'user', 'Unrelated')])
  source.turns = [
    { id: 'turn-1', replacesTurnId: 'outside-page', replacedBy: 'turn-2', state: 'succeeded', paused: false, hold: null, route: 'hosted' },
    { id: 'turn-2', replacesTurnId: 'turn-1', replacedBy: null, state: 'pending', paused: false, hold: null, route: 'hosted' },
  ]
  const projected = conversationTurns(source)
  expect(projected.map(turn => turn.fixes)).toEqual([1, 2, 0])
  expect(projected[1].execution).toBe(source.turns[1])
  expect(projected[2].execution).toBeUndefined()
})
