import type { ConversationSnapshot } from '../../generated/contracts'
import type { StoredTurn } from '../../types'
import { fixCount } from './fixes'
import { replyHelp } from './reply-help'
import { replyState } from './reply-state'

/** Read-only presentation of durable messages. Empty assistance is absent, never a completed analysis. */
export function conversationTurns(snapshot: ConversationSnapshot): StoredTurn[] {
  const turns: StoredTurn[] = []
  const byIdentity = new Map<string, StoredTurn>()
  const executions = new Map(snapshot.turns.map(turn => [turn.id, turn]))
  for (const message of snapshot.messages) {
    if (!message.turnId) throw new Error('Conversation message is missing its durable turn identity.')
    if (message.wordGloss && message.wordGloss.sourceMessageId !== message.id) {
      throw new Error('Saved word meanings do not belong to this message.')
    }
    let turn = byIdentity.get(message.turnId)
    if (!turn) {
      const execution = executions.get(message.turnId)
      turn = { replyState: replyState(execution, snapshot), execution, id: message.sequence, turnId: message.turnId, replacesTurnId: message.replacesTurnId, replacedBy: message.replacedBy, fixes: fixCount(message.turnId, executions), user: null, assistant: null, analysisState: null }
      byIdentity.set(message.turnId, turn)
      turns.push(turn)
    }
    if (message.role === 'user') {
      turn.userMessageId = message.id
      const feedback = message.feedback
      Object.assign(turn, { assessmentState: message.assessmentState, assessmentError: message.assessmentError, feedbackState: message.feedbackState, userGlossOperationId: message.glossOperationId, userSavedGloss: message.wordGloss, userTranslation: message.translation, userTranslationState: message.translationState, userGlossError: message.glossError, userGlossState: message.glossState, id: message.sequence, user: message.text, conversationFeedback: message.conversationFeedback, feedbackContext: message.feedbackContext, analysisState: feedback || message.conversationFeedback ? 'done' : ['ready', 'running', 'waiting_dependencies'].includes(message.feedbackState ?? '') || ['ready', 'running', 'waiting_dependencies'].includes(message.assessmentState ?? '') ? 'pending' : null, coachError: message.feedbackError ?? undefined,
        ...(feedback ? { coach: feedback } : {}), ...(message.coachDecision ? { coachDecision: message.coachDecision } : {}) })
    } else if (message.role === 'assistant') {
      turn.reaction = message.reaction ?? undefined
      turn.reactionError = message.reactionError ?? undefined
      turn.assistant = {
        messageId: message.id,
        help: replyHelp(message, turn.execution),
        reply: message.text, translation: message.translation, translationState: message.translationState,
        savedGloss: message.wordGloss, glossError: message.glossError, glossState: message.glossState, glossOperationId: message.glossOperationId,
        tokens: [], user_tokens: [], user_translation: null, mechanics: message.replyExplanations?.cards.map(card => ({ ...card, cefr: null })) ?? [],
        assistance: message.replyAssistance, explanationsState: message.explanationsState, explanationsError: message.explanationsError,
        scaffolds: { replies: message.suggestedReplies ?? [], frames: message.replyAssistance?.frames ?? [], starters: message.replyAssistance?.starters ?? [] },
        suggestionsState: message.suggestionsState ?? null,
        errors: message.suggestionsError ? [message.suggestionsError] : [],
      }
    } else throw new Error('Unexpected conversation message role.')
  }
  return turns
}
