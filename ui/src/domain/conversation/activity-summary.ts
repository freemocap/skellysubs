import type { TurnView } from '../../generated/contracts'

/// Operation states, grouped by what a surface should do with them. Every state
/// the scheduler stores is here, plus `held`, which snapshots derive for ready
/// work while inference or the turn is paused.
export type OperationPhase = 'waiting' | 'held' | 'running' | 'succeeded' | 'failed' | 'unknown' | 'ended'

export function operationPhase(state: string | null | undefined): OperationPhase | null {
  switch (state) {
    case 'waiting_dependencies': case 'ready': return 'waiting'
    case 'held': return 'held'
    case 'running': return 'running'
    case 'succeeded': return 'succeeded'
    case 'failed': return 'failed'
    case 'unknown': case 'unknown_outcome': return 'unknown'
    case 'cancelled': case 'invalidated': return 'ended'
    default: return null
  }
}

/// Operation kinds are shown as the scheduler names them, underscores removed.
/// There is deliberately no label table: the kind is the name.
export function humanizeKind(kind: string): string {
  return kind.replaceAll('_', ' ')
}

/// Words in a streaming reply, counted by the platform's word segmenter so
/// spaceless scripts count correctly.
export function countWords(text: string, locale?: string): number {
  if (typeof Intl !== 'undefined' && 'Segmenter' in Intl) {
    let count = 0
    for (const segment of new Intl.Segmenter(locale, { granularity: 'word' }).segment(text)) if (segment.isWordLike) count++
    return count
  }
  return text.split(/\s+/).filter(Boolean).length
}

export interface TurnActivity {
  total: number
  done: number
  waiting: number
  held: number
  failed: number
  /// Running executable node identities in snapshot order.
  running: string[]
  /// Whether a prose reply is being generated in this turn.
  replyRunning: boolean
  /// Words received so far for the running reply, when streamed text is known.
  replyWords: number | null
  /// The operation that finished most recently, humanized.
  lastFinished: string | null
  /// Nothing is waiting, held or running.
  settled: boolean
  /// First start to last finish, once settled.
  elapsedMs: number | null
}

/** Summarize authoritative dispositions without inferring activation or timing. */
export function turnActivity(turn: Pick<TurnView, 'nativeGraph' | 'nativePreview'>, replyText: string | null = null, locale?: string): TurnActivity {
  const graph = turn.nativeGraph
  const entries = Object.entries(graph?.nodes ?? {})
  const count = (...states: string[]) => entries.filter(([, state]) => states.includes(state)).length
  const running = entries.filter(([, state]) => state === 'Running').map(([name]) => name)
  const waiting = count('Ready', 'Waiting', 'Prepared', 'Available')
  const held = count('Paused', 'Held')
  const preview = turn.nativePreview
  const replyRunning = !!preview && running.some(node => graph?.attempts[node]?.some(attempt =>
    attempt.id === preview.attempt && attempt.execution === preview.execution && attempt.state === 'Running'))
  const text = replyText ?? preview?.capture.text ?? null
  return { total: entries.length, done: count('Adopted'), waiting, held, failed: count('Failed', 'Unknown'),
    running, replyRunning, replyWords: replyRunning && text !== null ? countWords(text, locale) : null,
    lastFinished: null, settled: !waiting && !held && !running.length, elapsedMs: null }
}

/** Captured reply text retains the native attempt and execution association. */
export function retainedReplyText(turn: Pick<TurnView, 'nativePreview'> | undefined): string | null {
  return turn?.nativePreview?.capture.text ?? null
}
