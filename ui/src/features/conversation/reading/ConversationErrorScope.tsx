import { AiRetryContext } from '../../../components/feedback/AiRetry'
import { executeAction, readWorkspace } from '../../../platform/ipc/workspace'
import type { ReactNode } from 'react'
import type { TurnView } from '../../../generated/contracts'
import { ErrorInspectionContext } from '../../../components/feedback/ErrorDetails'
import { useNavigationStore } from '../../../state/navigation/navigation'

function errorNode(turn: TurnView, error: string): string | null {
  try {
    const details: unknown = JSON.parse(error)
    if (details && typeof details === 'object' && 'diagnostics' in details) {
      const diagnostics = details.diagnostics
      if (diagnostics && typeof diagnostics === 'object' && 'node' in diagnostics && typeof diagnostics.node === 'string'
        && 'engine' in diagnostics && diagnostics.engine === turn.nativeGraph?.engine
        && 'run' in diagnostics && diagnostics.run === turn.nativeGraph?.run
        && Object.hasOwn(turn.nativeGraph?.nodes ?? {}, diagnostics.node)) return diagnostics.node
    }
  } catch { /* Plain error text has no executable identity. */ }
  return null
}

/** Keep error inspection tied to the recorded exchange, including older messages. */
export function ConversationErrorScope({ conversationId, turn, onInspect, children }: {
  conversationId?: string; turn?: TurnView; onInspect?: () => void; children: ReactNode
}) {
  const retry = turn && ['failed', 'unknown'].includes(turn.state)
    ? async () => { await executeAction(await readWorkspace(), {kind:'controlTurn', turnId:turn.id, control:'retry'}) } : null
  return <AiRetryContext value={retry}><ErrorInspectionContext value={conversationId && turn ? error => {
    onInspect?.()
    useNavigationStore.getState().inspectAi({ conversationId, turnId: turn.id, operationKind: errorNode(turn, error) })
  } : null}>{children}</ErrorInspectionContext></AiRetryContext>
}
