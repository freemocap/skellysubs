import { turnGraphTarget } from '../../../domain/activity/graph-target'
import type { StoredTurn } from '../../../types'
import type { ReplyHelpKind } from '../../../generated/contracts'
import { executeAction, readWorkspace } from '../../../platform/ipc/workspace'
import { useNavigationStore } from '../../../state/navigation/navigation'
import { MessageReadingScope } from '../reading/MessageReadingScope'
import { ReplyHelp } from './ReplyHelp'

export async function requestReplyHelp(messageId: string, helpKind: ReplyHelpKind, retry = false) {
  // Native admission validates ownership and treats already requested work as a no-op.
  const workspace = await readWorkspace()
  if (helpKind === 'brief') {
    await executeAction(workspace, { kind: 'requestMessageHelp', messageId, help: 'reply_brief', retry })
  } else {
    await executeAction(workspace, retry
      ? { kind: 'retryReplyHelp', messageId, helpKind }
      : { kind: helpKind === 'grammar' ? 'requestExplanations' : 'requestSuggestions', messageId })
  }
}

export function TurnReplyHelp({ turn, conversationId, busy, onAsk, onUse, inline = false }: {
  inline?: boolean; turn?: StoredTurn; conversationId?: string; busy: boolean; onAsk?: (question: string) => void
  onUse: (text: string, source: 'suggestion' | 'scaffold') => void
}) {
  const a = turn?.assistant, help = a?.help
  if (!a?.messageId || !help || !conversationId) return null
  const eligible = !turn?.replacedBy && !turn?.execution?.replacedBy && !['cancelled','invalidated'].includes(turn?.execution?.state ?? '')
  const request = (kind: ReplyHelpKind, retry = false) => requestReplyHelp(a.messageId!, kind, retry)
  return <MessageReadingScope scope={help.scope}><ReplyHelp graphTargets={{ brief: turnGraphTarget(conversationId, turn?.execution, 'reply_brief'), grammar: turnGraphTarget(conversationId, turn?.execution, 'reply_explanations'), replies: turnGraphTarget(conversationId, turn?.execution, 'reply_assistance') }} inline={inline} key={a.messageId}
    brief={help.brief?.explanation} briefPending={['ready','running','waiting_dependencies'].includes(help.lanes.brief.state ?? '')}
    grammar={help.grammar?.cards} replies={help.assistance?.replies}
    starters={help.assistance ? [...help.assistance.frames, ...help.assistance.starters] : undefined}
    lanes={help.lanes} onExplainGrammar={eligible ? () => request('grammar') : undefined}
    onRequestBrief={eligible ? () => request('brief') : undefined}
    onSuggestReply={eligible ? () => request('replies') : undefined} onRetry={eligible ? kind => request(kind, true) : undefined}
    onInspect={() => useNavigationStore.getState().inspectAi({conversationId, turnId:turn!.turnId!, operationKind:null})}
    onAsk={onAsk ? question => onAsk(`${question}\nPartner message: ${a.reply}`) : undefined}
    busy={busy} errors={[]} onUse={onUse} /></MessageReadingScope>
}
