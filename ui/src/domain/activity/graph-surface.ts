/// Identity of an app surface that displays results of one graph run, and the
/// hover message that asks the main window to hint where those results appear.
/// A hint states structural association only: the results downstream of the
/// hovered step, not the execution that produced what is currently displayed.

export type GraphSurfaceKey =
  | { scope: 'conversation'; conversationId: string; turnId: string }
  | { scope: 'workspace'; engine: string; run: string }

export interface GraphHoverTarget {
  key: GraphSurfaceKey
  /// Graph result port names (`definition.results` keys).
  results: string[]
}

export interface GraphHoverMessage {
  /// One identity per AI view instance; with `seq` it orders that view's messages.
  session: string
  seq: number
  /// Window label that receives the visibility reply.
  replyTo: string
  target: GraphHoverTarget | null
}

export interface GraphHoverReply {
  session: string
  seq: number
  matched: number
  visible: number
}

const MAX_RESULTS = 64
const MAX_TEXT = 256

const text = (value: unknown, field: string): string => {
  if (typeof value !== 'string' || !value || value.length > MAX_TEXT) throw new Error(`Invalid graph hover ${field}.`)
  return value
}
const count = (value: unknown, field: string): number => {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`Invalid graph hover ${field}.`)
  return value
}

export function parseGraphSurfaceKey(value: unknown): GraphSurfaceKey {
  if (!value || typeof value !== 'object') throw new Error('Invalid graph surface key.')
  const key = value as Record<string, unknown>
  if (key.scope === 'conversation') return { scope: 'conversation', conversationId: text(key.conversationId, 'conversation'), turnId: text(key.turnId, 'turn') }
  if (key.scope === 'workspace') return { scope: 'workspace', engine: text(key.engine, 'engine'), run: text(key.run, 'run') }
  throw new Error('Invalid graph surface scope.')
}

export function parseGraphHoverMessage(value: unknown): GraphHoverMessage {
  if (!value || typeof value !== 'object') throw new Error('Invalid graph hover message.')
  const message = value as Record<string, unknown>
  let target: GraphHoverTarget | null = null
  if (message.target !== null) {
    if (!message.target || typeof message.target !== 'object') throw new Error('Invalid graph hover target.')
    const raw = message.target as Record<string, unknown>
    if (!Array.isArray(raw.results) || raw.results.length > MAX_RESULTS) throw new Error('Invalid graph hover results.')
    target = { key: parseGraphSurfaceKey(raw.key), results: raw.results.map(result => text(result, 'result')) }
  }
  return { session: text(message.session, 'session'), seq: count(message.seq, 'sequence'), replyTo: text(message.replyTo, 'reply window'), target }
}

export function parseGraphHoverReply(value: unknown): GraphHoverReply {
  if (!value || typeof value !== 'object') throw new Error('Invalid graph hover reply.')
  const reply = value as Record<string, unknown>
  return { session: text(reply.session, 'session'), seq: count(reply.seq, 'sequence'), matched: count(reply.matched, 'match count'), visible: count(reply.visible, 'visible count') }
}

export function sameSurface(a: GraphSurfaceKey, b: GraphSurfaceKey): boolean {
  if (a.scope === 'conversation' && b.scope === 'conversation') return a.conversationId === b.conversationId && a.turnId === b.turnId
  if (a.scope === 'workspace' && b.scope === 'workspace') return a.engine === b.engine && a.run === b.run
  return false
}

/// Data attributes that mark an element as displaying the named results of one run.
export function graphTargetAttributes(key: GraphSurfaceKey, results: readonly string[]): Record<string, string> {
  if (!results.length) throw new Error('A graph target must name at least one result.')
  for (const result of results) if (!/^\S+$/.test(result)) throw new Error(`Graph result names cannot contain spaces: ${result}`)
  const scope: Record<string, string> = key.scope === 'conversation'
    ? { 'data-graph-scope': 'conversation', 'data-graph-conversation': key.conversationId, 'data-graph-turn': key.turnId }
    : { 'data-graph-scope': 'workspace', 'data-graph-engine': key.engine, 'data-graph-run': key.run }
  return { ...scope, 'data-graph-results': results.join(' ') }
}

/// Whether an element's data attributes name the target's run and any of its results.
export function matchesTarget(dataset: Record<string, string | undefined>, target: GraphHoverTarget): boolean {
  const { key } = target
  if (dataset.graphScope !== key.scope) return false
  if (key.scope === 'conversation' && (dataset.graphConversation !== key.conversationId || dataset.graphTurn !== key.turnId)) return false
  if (key.scope === 'workspace' && (dataset.graphEngine !== key.engine || dataset.graphRun !== key.run)) return false
  const shown = (dataset.graphResults ?? '').split(' ').filter(Boolean)
  return target.results.some(result => shown.includes(result))
}
