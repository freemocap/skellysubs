import type { TurnView } from '../../generated/contracts'
import { graphTargetAttributes } from './graph-surface'

/** The native publication adapters declare product-field associations. Components
 * attach them only to the element actually showing that field. They are structural
 * hints, never evidence of which attempt produced the displayed value. */
export function turnGraphTarget(conversationId: string | null | undefined, turn: TurnView | undefined, field: string): Record<string, string> {
  const results = turn?.nativeSurfaces?.[field]
  if (!conversationId || !turn?.nativeGraph || !results?.length) return {}
  for (const result of results) {
    if (!Object.hasOwn(turn.nativeGraph.artifact.definition.results, result)) throw new Error(`Unknown native surface result: ${result}`)
  }
  return graphTargetAttributes({ scope: 'conversation', conversationId, turnId: turn.id }, results)
}
