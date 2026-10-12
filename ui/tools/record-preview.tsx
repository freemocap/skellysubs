import { createRoot } from 'react-dom/client'
import { RecordView } from '../src/features/conversation/record/RecordView'
import type { ChatMessage, ConversationSnapshot, TurnView } from '../src/generated/contracts'
import '../src/styles/index.css'

/// Visual review of the conversation record: two exchanges with filled fields,
/// one still running, and a draft-free end.
const message = (fields: Partial<ChatMessage> & Pick<ChatMessage, 'id' | 'sequence' | 'role' | 'text' | 'turnId'>): ChatMessage => ({
  coachDecision: null, wordGloss: null, glossState: null, glossError: null, glossOperationId: null, replacesTurnId: null, replacedBy: null,
  createdAt: '2026-10-11T14:41:02Z', translation: null, translationState: null, ...fields,
})
const gloss = { sourceMessageId: 'm', targetLanguageId: 'spanish', explanationLanguageId: 'english', formatVersion: '1', templateVersion: '1', boundaryPolicy: 'word', operationId: 'op', attemptId: 'a', coverage: 'full' as const, segments: [] }
const turn = (id: string, surfaces: Record<string, string[]>, speech?: TurnView['speech']): TurnView => ({
  id, channel: 'persona_reply', state: 'succeeded', route: 'hosted', paused: false, hold: null, replacesTurnId: null, replacedBy: null, nativeSurfaces: surfaces,
  nativeGraph: { protocol: 1, engine: 'engine', run: `run-${id}`, revision: '261', artifact_id: 'artifact', artifact: { types: {}, operations: {}, definition: { contract: 'c', inputs: {}, outputs: {}, nodes: {}, results: Object.fromEntries(Object.values(surfaces).flat().map(result => [result, { Absent: 't' }])), compositions: {} } }, nodes: {}, activation: {}, paused: false, active: false, stepping: null, step_available: false, attempts: {}, reasons: {} },
  ...(speech ? { speech } : {}),
})
const snapshot: ConversationSnapshot = {
  opening: null, topicChoices: [], starterGreeting: { text: 'hola', romanized: null }, revisionSuffixCounts: [], conversationId: 'chat', sessionId: 's', revision: 12, hasOlder: false, coachMessages: [], transcriptionAttempts: [],
  connection: { paused: false } as ConversationSnapshot['connection'],
  messages: [
    message({ id: 'm12', sequence: 12, role: 'user', text: 'El sábado fui al mercado de pulgas y compré una lámpara.', turnId: 't6', translation: 'On Saturday I went to the flea market and bought a lamp.', translationState: 'succeeded', wordGloss: gloss, glossState: 'succeeded', assessmentState: 'adopted', feedbackState: 'succeeded',
      feedback: { summary: 'Good past tense.' } as unknown as ChatMessage['feedback'], coachDecision: { exposedMove: null, shown: null, retryInvited: false, alsoNoticed: [], keptGoing: true }, conversationFeedback: { grammar: 4, conversation: 5, answers: {} } }),
    message({ id: 'm13', sequence: 13, role: 'assistant', text: 'Fui al mercado de pulgas el sábado. Encontré una lámpara muy bonita. ¿Te gustan los mercados?', turnId: 't6', translation: 'I went to the flea market on Saturday…', translationState: 'succeeded', wordGloss: gloss, glossState: 'succeeded', briefState: 'ready', replyAssistance: { replies: [], frames: ['Me gustan…'], starters: ['Sí,'] }, explanationsState: 'running' }),
    message({ id: 'm14', sequence: 14, role: 'user', text: 'Sí, me encantan. ¿Qué compraste tú?', turnId: 't7', translationState: 'running', glossState: 'running', assessmentState: 'running' }),
  ],
  turns: [
    turn('t6', { 'learner.translation': ['learner_translation'], 'reply.translation': ['reply_translation'], 'learner.gloss': ['learner_gloss'], 'reply.gloss': ['reply_gloss'], coach_feedback: ['feedback'], skill_assessment: ['assessment'], attribution: ['attribution'], reply_assistance: ['assistance'], audio: ['audio'] }, { sourceMessageId: 'm13', id: 'speech-1', state: 'succeeded' }),
    turn('t7', {}),
  ],
}
createRoot(document.getElementById('root')!).render(<div style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
  <RecordView snapshot={snapshot} rtl={false} languageTag="es" onClose={() => console.log('close')}><div className="composer" style={{ height: 160 }}>recorder</div></RecordView>
</div>)
