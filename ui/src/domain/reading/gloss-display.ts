import type { GlossSegment } from '../../generated/contracts'
import type { GuidedToken } from '../../types'
import { sourceToken } from './source-token'
import { requiresWholeWordShaping } from '../language/script-text'

/** Morphological anchors remain unchanged. Only their presentation shares a word box.
 * Include marks and joining controls, including uncovered prefixes/suffixes, so
 * annotations can never interrupt shaping inside a word with joining or combining characters.
 */
export function glossDisplayGroups(text: string, segments: GlossSegment[]) {
  if (!segments.length) return []
  const words = [...text.matchAll(/[\p{L}\p{M}\p{N}\u200c\u200d]+/gu)]
    .filter(match => requiresWholeWordShaping(match[0]))
    .map(match => ({ start: match.index, end: match.index + match[0].length }))
  const groups: { start: number; end: number; parts: GlossSegment[] }[] = []
  for (const segment of segments) {
    const overlaps = words.filter(word => word.start < segment.end && word.end > segment.start)
    const start = Math.min(segment.start, ...overlaps.map(word => word.start))
    const end = Math.max(segment.end, ...overlaps.map(word => word.end))
    const previous = groups.at(-1)
    if (previous && start < previous.end) {
      previous.end = Math.max(previous.end, end)
      previous.parts.push(segment)
    } else groups.push({ start, end, parts: [segment] })
  }
  return groups
}

export function anchoredTokenGlosses(text: string, tokens: GuidedToken[]): GlossSegment[] {
  let cursor = 0
  return tokens.flatMap(token => {
    const match = sourceToken(text, token.text, cursor)
    if (!match) return []
    cursor = match.end
    return [{ start: match.start, end: match.end, kind: token.gloss ? 'gloss' as const : 'literal' as const,
      gloss: token.gloss, romanization: token.romanization ?? undefined, pronunciation: token.pronunciation ?? undefined }]
  })
}
