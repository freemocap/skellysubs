import { executeAction, readWorkspace } from '../../../platform/ipc/workspace'
import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { useI18n } from '../../../components/localization/i18n'
import type { ConversationSnapshot } from '../../../generated/contracts'
import { ReplyStatus } from '../messages/ReplyStatus'

/** Partner-first work has no learner bubble on which to attach its state. */
export function OpeningStatus({ snapshot, onActivity }: { snapshot: ConversationSnapshot; onActivity: () => void }) {
  const tr = useI18n()
  if (!snapshot.opening || snapshot.opening.kind === 'learner') return null
  const turn = snapshot.turns.find(item => !item.replacedBy && item.channel === 'persona_opening')
  let error = turn?.hold?.message ?? null
  if (turn && !error && ['failed', 'unknown', 'unknown_outcome', 'cancelled'].includes(turn.state)) {
    error = (turn.state === 'cancelled' ? tr('Conversation opening was cancelled.') : turn.state === 'failed' ? tr('Conversation opening failed.') : tr('The outcome of the conversation opening is unknown.'))
  }
  if (!error && !turn?.paused && !snapshot.connection.paused) return <section aria-label={tr("Conversation opening")}><ReplyStatus reply={{ state: 'pending', error: null, control: null }} /></section>
  return <section className="opening-status" aria-label={tr("Conversation opening")}>
    {error ? <ErrorNotice as="p" onRetry={turn ? async () => { await executeAction(await readWorkspace(), {kind:'controlTurn', turnId:turn.id, control:'retry'}) } : null} error={error}>{error}</ErrorNotice> : <p role="status">{tr("Conversation opening is paused.")}</p>}
    <button type="button" className="btn" onClick={onActivity}>{tr("Open AI activity")}</button>
  </section>
}
