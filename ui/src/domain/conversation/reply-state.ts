import type { ConversationSnapshot, TurnView } from '../../generated/contracts'

export interface ReplyState {
  state: 'pending' | 'paused' | 'held' | 'failed' | 'unknown' | 'cancelled' | 'unavailable'
  error: string | null
  control: 'retry' | 'resume' | null
}

/** Native turn state follows the declared reply publication effect. */
export function replyState(turn: TurnView | undefined, snapshot: Pick<ConversationSnapshot, 'connection' | 'turns'>): ReplyState {
  if (!turn) return { state: 'unavailable', error: null, control: null }
  const retry = ['failed', 'unknown'].includes(turn.state) && snapshot.turns[0]?.id === turn.id ? 'retry' : null
  if (turn.state === 'failed') return { state: 'failed', error: turn.hold?.message ?? null, control: retry }
  if (turn.state === 'unknown') return { state: 'unknown', error: turn.hold?.message ?? null, control: retry }
  if (turn.state === 'cancelled' || turn.state === 'invalidated') return { state: 'cancelled', error: null, control: null }
  if (turn.hold) return { state: 'held', error: turn.hold.message, control: snapshot.connection.paused ? null : 'resume' }
  if (turn.paused || snapshot.connection.paused) return { state: 'paused', error: null, control: snapshot.connection.paused ? null : 'resume' }
  return { state: turn.state === 'pending' ? 'pending' : 'unavailable', error: null, control: null }
}
