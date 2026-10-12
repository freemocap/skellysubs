import type { GlossSegment, ReadingInput, SavedGlossSource as AcceptedGlossSource } from '../../generated/contracts'
import { glossDisplayGroups } from './gloss-display'
import { readingWords } from './word-boundaries'

export type GlossScope = Omit<ReadingInput, 'text' | 'aid'>
export type SavedGlossSource = Pick<AcceptedGlossSource, 'text' | 'segments' | 'scope'> & { dictionary?: boolean }
export type SavedGlossSegment = GlossSegment & { dictionary?: boolean }
export const glossScopeKey = (scope: GlossScope) => JSON.stringify([scope.language, scope.variety ?? null, scope.explanation, scope.explanationVariety ?? null])

/** A read index over existing annotations, not a second source of truth.
 * Exact contextual passages win, followed by contextual words, then dictionary
 * meanings. Dictionary entries never count as exact passage analyses.
 * Repeated surface forms retain alternate saved meanings;
 * accents, case and language varieties are never silently collapsed. */
export function savedGlossIndex(sources: SavedGlossSource[]) {
  const passages = new Map<string, GlossSegment[]>()
  const words = new Map<string, SavedGlossSegment[][]>()
  const scopesWithWords = new Set<string>()
  for (const source of sources) {
    const scope = glossScopeKey(source.scope)
    const valid = source.segments.filter(part => part.kind === 'gloss' && part.gloss && part.start >= 0 && part.end <= source.text.length && part.end > part.start)
    if (!source.dictionary) passages.set(JSON.stringify([scope, source.text]), valid)
    for (const group of glossDisplayGroups(source.text, valid)) {
      scopesWithWords.add(scope)
      const key = JSON.stringify([scope, source.text.slice(group.start, group.end)])
      const parts = group.parts.map(part => ({ ...part, ...(source.dictionary ? { dictionary: true } : {}), start: part.start - group.start, end: part.end - group.start }))
      const existing = words.get(key) ?? []
      if (!existing.some(previous => JSON.stringify(previous) === JSON.stringify(parts))) existing.push(parts)
      words.set(key, existing)
    }
  }
  return (text: string, scope: GlossScope): SavedGlossSegment[] => {
    const key = glossScopeKey(scope)
    // There can be no exact or reusable gloss in this scope. Avoid segmenting
    // every displayed passage just to discover that there are no candidates.
    if (!scopesWithWords.has(key)) return []
    const exact = passages.get(JSON.stringify([key, text]))
    const parts = readingWords(text).flatMap(word => {
      if (!word.word || exact?.some(part => part.start < word.end && part.end > word.start)) return []
      const available = words.get(JSON.stringify([key, text.slice(word.start, word.end)])) ?? []
      const contextual = available.filter(parts => !parts.some(part => part.dictionary))
      const candidates = contextual.length ? contextual : available
      const partitions = new Set(candidates.map(parts => JSON.stringify(parts.map(part => [part.start, part.end]))))
      if (partitions.size > 1) {
        const alternatives = (field: 'gloss' | 'romanization' | 'pronunciation') => [...new Set(candidates.map(parts => parts.map(part => part[field]).filter(Boolean).join(field === 'gloss' ? ' ' : '')).filter(Boolean))].join(' / ')
        return [{...(candidates.some(parts => parts.some(part => part.dictionary)) ? { dictionary: true } : {}),start:word.start,end:word.end,kind:'gloss' as const,gloss:alternatives('gloss'),romanization:alternatives('romanization') || undefined,pronunciation:alternatives('pronunciation') || undefined}]
      }
      const matches = candidates.flat()
      // Preserve morphological subparts, showing alternative saved senses together.
      const grouped = new Map<string, SavedGlossSegment[]>()
      for (const match of matches) {
        const anchor = `${match.start}:${match.end}`
        grouped.set(anchor, [...grouped.get(anchor) ?? [], match])
      }
      return [...grouped.values()].map(values => {
        const join = (field: 'gloss' | 'romanization' | 'pronunciation') => [...new Set(values.map(value => value[field]).filter(Boolean))].join(' / ')
        return { ...values[0], start: word.start + values[0].start, end: word.start + values[0].end, gloss: join('gloss'), romanization: join('romanization') || undefined, pronunciation: join('pronunciation') || undefined }
      })
    })
    return [...exact ?? [], ...parts].sort((a,b) => a.start - b.start || a.end - b.end)
  }
}
