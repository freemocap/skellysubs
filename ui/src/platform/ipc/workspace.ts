import { batchRead } from './batch-read'
import { bindRewardOrigin, captureRewardOrigin, inheritRewardOrigin } from './reward-origin'
import { effortPublished } from './effort-events'
import { errorMessage } from '../diagnostics/error-details'
import { invoke } from './native'
import type { Action, Command, Conversation, ConversationSnapshot, PersonaDetails, PersonaGenerationActivity, Receipt, Snapshot, TurnHistoryPage } from '../../generated/contracts'

/** Coalesce one synchronous notification burst; subsequent reads always observe
 * fresh state, including invalidations received while a prior read is pending. */
export const readWorkspace = batchRead(() => invoke<Snapshot>('get_snapshot'))

/** Commands carry the native session and a unique delivery identity; never retry automatically. */
export function executeAction(snapshot: Pick<Snapshot, 'sessionId'>, action: Action): Promise<Receipt> {
  const command: Command = { sessionId: snapshot.sessionId, actionId: crypto.randomUUID(), action }
  const origin = captureRewardOrigin()
  return invoke<Receipt>('execute_command', { command }).then(receipt => {
    if (['startConversation', 'sendMessage', 'reviseTurn', 'askCoach', 'requestSuggestions', 'requestExplanations', 'reassessFeedback', 'retryReplyHelp', 'coachControl'].includes(action.kind)) bindRewardOrigin(receipt.entityId, origin)
    if (action.kind === 'startConversation') bindRewardOrigin('steering:' + action.conversationId, origin)
    return receipt
  })
}

/** Reserve an owned generation before any provider work starts. */
export function beginPersonaGeneration(languageId: string, brief: string): Promise<string> {
  const trimmed = brief.trim()
  const origin = captureRewardOrigin()
  return invoke<string>('begin_persona_generation', { languageId, brief: trimmed ? trimmed : null }).then(id => { bindRewardOrigin('persona:' + id, origin); return id })
}

/** Fill a proposal for review; creating the contact remains an explicit action. */
export async function runPersonaGeneration(generationId: string): Promise<PersonaDetails> {
  const result = await invoke<PersonaDetails>('run_persona_generation', { generationId })
  effortPublished()
  return result
}

export function cancelPersonaGeneration(generationId: string): Promise<void> {
  return invoke<void>('cancel_persona_generation', { generationId })
}

/** Global retained generation metadata only; inspecting never generates a persona. */
export function readPersonaGenerationActivity(): Promise<PersonaGenerationActivity> {
  return invoke<PersonaGenerationActivity>('get_persona_generation_activity')
}

/** The one creation action: a persona, its contact, and their first conversation.
 * Returns the conversation to open. */
export async function createContact(languageId: string, details: PersonaDetails): Promise<string> {
  const snapshot = await readWorkspace()
  const receipt = await executeAction(snapshot, { kind: 'createContact', languageId, details })
  return receipt.entityId
}

const conversationReads = new Map<string, Promise<ConversationSnapshot>>()

export function watchConversation(conversationId: string, afterRevision = -1, before: number | null = null): Promise<ConversationSnapshot> {
  const key = JSON.stringify([conversationId, afterRevision, before])
  const pending = conversationReads.get(key)
  if (pending) return pending
  const read = invoke<ConversationSnapshot>('watch_conversation', { conversationId, afterRevision, before }).then(snapshot => {
    for (const turn of snapshot.turns) {
      for (const source of turn.awardSources ?? []) inheritRewardOrigin(source, turn.id)
      if (turn.speech) inheritRewardOrigin(turn.speech.id, turn.id)
    }
    return snapshot
  }).finally(() => conversationReads.delete(key))
  conversationReads.set(key, read)
  return read
}

export function selectedConversation(snapshot: Snapshot, languageId?: string): Conversation | null {
  return snapshot.conversations.filter(c => !c.archived && (!languageId || c.languageId === languageId))
    .sort((a, b) => b.lastUsed - a.lastUsed || a.id.localeCompare(b.id))[0] ?? null
}

export function nativeError(error: unknown): string {
  return errorMessage(error)
}

/** Older turns keyed by turn, so history reaches turns that never produced a message. */
export function listTurnHistory(conversationId: string, before: string | null, limit = 40): Promise<TurnHistoryPage> {
  return invoke<TurnHistoryPage>('list_turn_history', { conversationId, before, limit })
}
