import type { Bundle } from './session.ts'

type Clock = 'ui' | 'native' | 'runner'
export type SpanSample = { trace: string; span: string; clock: Clock; durationMicros: number }
export type MissingSpan = { trace: string; span: string; clock: Clock; missing: 'start' | 'end' }
const identity = (value: { trace: string; span: string; clock: Clock }): string => JSON.stringify([value.clock, value.trace, value.span])

/** Durations are always measured within one clock. No cross-process clock
 * subtraction or rendering-completion inference from IPC completion. */
export function latencyReport(bundle: Bundle, allowIncomplete = false): { samples: SpanSample[]; missing: MissingSpan[] } {
  if (bundle.status !== 'complete' && !allowIncomplete) throw new Error('Latency report requires a complete recording.')
  const spans = new Map<string, { trace: string; span: string; clock: Clock; start?: number; end?: number }>()
  for (const event of bundle.events) {
    if (event.kind !== 'timing') continue
    const key = identity(event)
    const entry = spans.get(key) ?? { trace: event.trace, span: event.span, clock: event.clock }
    if (entry[event.phase] !== undefined) throw new Error('Duplicate timing phase for a span identity.')
    if (!Number.isSafeInteger(event.micros) || event.micros < 0) throw new Error('Invalid timing measurement.')
    entry[event.phase] = event.micros
    spans.set(key, entry)
  }
  const samples: SpanSample[] = []
  const missing: MissingSpan[] = []
  for (const entry of spans.values()) {
    const { trace, span, clock, start, end } = entry
    if (start === undefined || end === undefined) {
      missing.push({ trace, span, clock, missing: start === undefined ? 'start' : 'end' })
    } else {
      if (end < start) throw new Error('Timing span ends before it starts.')
      samples.push({ trace, span, clock, durationMicros: end - start })
    }
  }
  return { samples, missing }
}

export type Distribution = { count: number; p50Micros: number; p95Micros: number; maxMicros: number }
function distribution(samples: number[]): Distribution {
  const sorted = [...samples].sort((a, b) => a - b)
  const nearestRank = (fraction: number): number => sorted[Math.ceil(sorted.length * fraction) - 1]
  return { count: sorted.length, p50Micros: nearestRank(.5), p95Micros: nearestRank(.95), maxMicros: sorted.at(-1)! }
}
function workload(bundle: Bundle): string {
  return JSON.stringify({
    workspace: bundle.workspace,
    actions: bundle.events.filter(event => event.kind === 'action').map(({ kind, target, action, value }) => ({ kind, target, action, value })),
    providers: bundle.events.filter(event => event.kind === 'provider' && event.phase === 'request').map(event => {
      if (event.kind !== 'provider') throw new Error('Invalid provider workload.')
      return [event.request, event.fingerprint]
    }).sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))),
  })
}

/** Compare only recordings with the same starting workspace, ordered semantic
 * actions and provider request identities. Build IDs may differ deliberately. */
export function compareLatency(baseline: Bundle, candidate: Bundle) {
  if (workload(baseline) !== workload(candidate)) throw new Error('Cannot compare different recorded workloads.')
  const before = latencyReport(baseline)
  const after = latencyReport(candidate)
  const candidateSamples = new Map(after.samples.map(sample => [identity(sample), sample]))
  const pairs: { trace: string; span: string; clock: Clock; baselineMicros: number; candidateMicros: number; deltaMicros: number }[] = []
  const unmatched: { side: 'baseline' | 'candidate'; sample: SpanSample }[] = []
  for (const sample of before.samples) {
    const match = candidateSamples.get(identity(sample))
    if (!match) { unmatched.push({ side: 'baseline', sample }); continue }
    candidateSamples.delete(identity(sample))
    pairs.push({ trace: sample.trace, span: sample.span, clock: sample.clock, baselineMicros: sample.durationMicros, candidateMicros: match.durationMicros, deltaMicros: match.durationMicros - sample.durationMicros })
  }
  for (const sample of candidateSamples.values()) unmatched.push({ side: 'candidate', sample })
  const groups = new Map<string, typeof pairs>()
  for (const pair of pairs) {
    const key = JSON.stringify([pair.clock, pair.span])
    const group = groups.get(key) ?? []
    group.push(pair); groups.set(key, group)
  }
  return {
    baselineBuild: baseline.build, candidateBuild: candidate.build,
    pairs, unmatched, missing: { baseline: before.missing, candidate: after.missing },
    summaries: [...groups.values()].map(group => ({
      clock: group[0].clock, span: group[0].span,
      baseline: distribution(group.map(pair => pair.baselineMicros)),
      candidate: distribution(group.map(pair => pair.candidateMicros)),
    })),
  }
}
