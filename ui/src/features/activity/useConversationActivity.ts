import { useCallback, useEffect, useRef, useState } from 'react'
import type { ConversationSnapshot, TurnView } from '../../generated/contracts'
import { listTurnHistory, nativeError, readWorkspace, selectedConversation, watchConversation } from '../../platform/ipc/workspace'

const HISTORY_PAGE = 40

export interface ConversationActivity {
  snapshot: ConversationSnapshot | null
  /// Every loaded turn, newest first: the snapshot's live window, then older
  /// pages fetched by turn.
  turns: TurnView[]
  hasOlder: boolean
  loadingOlder: boolean
  loadOlder: () => void
  error: string | null
}

/// Watch the selected conversation's durable operations. Inspecting never
/// dispatches inference: it only reads what the scheduler recorded.
export function useConversationActivity(): ConversationActivity {
  const [snapshot, setSnapshot] = useState<ConversationSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [older, setOlder] = useState<TurnView[]>([])
  const [olderExhausted, setOlderExhausted] = useState(false)
  const [loadingOlder, setLoadingOlder] = useState(false)
  const scope = useRef<string | null>(null)

  useEffect(() => {
    let stopped = false
    void (async () => {
      let workspace = await readWorkspace()
      let conversationId: string | null = null
      let revision = -1
      while (!stopped) {
        const desired = selectedConversation(workspace)?.id ?? null
        if (desired !== conversationId) {
          conversationId = desired; revision = -1; scope.current = desired
          setSnapshot(null); setError(null); setOlder([]); setOlderExhausted(false)
        }
        if (!conversationId) {
          // No conversation exists to long-poll yet. Keep this inspector alive
          // so opening/creating one in the main window supplies its scope.
          await new Promise(resolve => setTimeout(resolve, 500))
          if (stopped) return
          workspace = await readWorkspace()
          continue
        }
        let next: ConversationSnapshot
        try { next = await watchConversation(conversationId, revision) }
        catch (failure) {
          if (stopped) return
          workspace = await readWorkspace()
          if (stopped) return
          // Deleting/archiving the abandoned conversation may reject its wait.
          if (selectedConversation(workspace)?.id !== conversationId) continue
          throw failure
        }
        if (stopped) return
        if (next.conversationId !== conversationId) throw new Error('AI activity returned a different conversation.')
        if (next.revision !== revision) setSnapshot(next)
        revision = next.revision
        // Native selection changes increment the same global revision that
        // wakes this wait. Reconcile selection without withholding available data.
        workspace = await readWorkspace()
        if (stopped) return
        if (selectedConversation(workspace)?.id !== conversationId) continue
      }
    })().catch(failure => { if (!stopped) { setSnapshot(null); setError(nativeError(failure)) } })
    return () => { stopped = true }
  }, [])

  const live = snapshot?.turns ?? []
  const liveIds = new Set(live.map(turn => turn.id))
  const turns = [...live, ...older.filter(turn => !liveIds.has(turn.id))]
  // The snapshot carries at most its latest 50 turns; fewer means none older.
  const hasOlder = !olderExhausted && (older.length > 0 || live.length >= 50)

  const oldestId = turns.at(-1)?.id ?? null
  const conversationId = snapshot?.conversationId ?? null
  const loadOlder = useCallback(() => {
    if (!conversationId || loadingOlder) return
    const requested = conversationId
    setLoadingOlder(true)
    listTurnHistory(requested, oldestId, HISTORY_PAGE)
      .then(page => {
        if (scope.current !== requested) return
        setOlder(current => {
          const known = new Set(current.map(turn => turn.id))
          return [...current, ...page.turns.filter(turn => !known.has(turn.id))]
        })
        setOlderExhausted(!page.hasOlder)
      })
      .catch(failure => { if (scope.current === requested) setError(nativeError(failure)) })
      .finally(() => setLoadingOlder(false))
  }, [conversationId, oldestId, loadingOlder])

  // Older pages are refreshed from native when the conversation changes, so a
  // retried or replaced old turn never shows stale state.
  const revision = snapshot?.revision
  const loadedOlder = older.length
  useEffect(() => {
    if (!conversationId || !loadedOlder) return
    const requested = conversationId
    const anchor = live.at(-1)?.id ?? null
    if (!anchor) return
    let cancelled = false
    void (async () => {
      const refreshed: TurnView[] = []
      let before: string | null = anchor
      let exhausted = false
      while (refreshed.length < loadedOlder && !exhausted) {
        const page = await listTurnHistory(requested, before, Math.min(100, loadedOlder - refreshed.length))
        refreshed.push(...page.turns)
        exhausted = !page.hasOlder
        before = page.turns.at(-1)?.id ?? null
        if (!before) break
      }
      if (!cancelled && scope.current === requested) { setOlder(refreshed); setOlderExhausted(exhausted) }
    })().catch(failure => { if (!cancelled && scope.current === requested) setError(nativeError(failure)) })
    return () => { cancelled = true }
    // Refresh on each new revision only; the loaded count is read at that time.
  }, [revision, conversationId])

  return { snapshot, turns, hasOlder, loadingOlder, loadOlder, error }
}
