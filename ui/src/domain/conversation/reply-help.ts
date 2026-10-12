import type { ChatMessage, ReplyHelpKind, TurnView } from '../../generated/contracts'

export interface HelpLane {
  state: string | null
  revision?: string
  error?: string | null
  details?: unknown
}
export interface ReplyHelpView {
  brief: ChatMessage['replyBrief']
  assistance: ChatMessage['replyAssistance']
  grammar: ChatMessage['replyExplanations']
  scope: ChatMessage['readingScope']
  lanes: Record<ReplyHelpKind, HelpLane>
}

/** Results and lifecycle have separate authority: an empty result is still success. */
export function replyHelp(message: ChatMessage, turn?: TurnView): ReplyHelpView {
  function lane(state: string | null | undefined, error: string | null | undefined, present: boolean): HelpLane {
    const status = turn?.replacedBy || ['cancelled', 'invalidated'].includes(turn?.state ?? '') ? 'invalidated'
      : state ?? null
    return { state: status, revision: `${status}:${present}:${turn?.nativeGraph?.revision}`, error: status === 'succeeded' && !present ? 'Saved reply help is missing its result.' : error,
      details: { graph: turn?.nativeGraph, hold: turn?.hold } }
  }
  return {
    brief: message.replyBrief, assistance: message.replyAssistance, grammar: message.replyExplanations, scope: message.readingScope,
    lanes: {
      brief: lane(message.briefState, message.briefError, message.replyBrief != null),
      replies: lane(message.suggestionsState, message.suggestionsError, message.replyAssistance != null),
      grammar: lane(message.explanationsState, message.explanationsError, message.replyExplanations != null),
    },
  }
}
