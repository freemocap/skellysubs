import type { TurnView } from '../../generated/contracts'

/// How many fixes led to a message: the replacements behind its turn, each a
/// revision the learner sent in place of the message before. The chain is read
/// from the loaded turns; a replaced turn older than them still counts, and
/// ends the count.
export function fixCount(turnId: string | null | undefined, turns: ReadonlyMap<string, Pick<TurnView, 'replacesTurnId'>>): number {
  const seen = new Set<string>()
  let count = 0
  let previous = turnId ? turns.get(turnId)?.replacesTurnId ?? null : null
  while (previous && !seen.has(previous)) {
    seen.add(previous)
    count++
    previous = turns.get(previous)?.replacesTurnId ?? null
  }
  return count
}
