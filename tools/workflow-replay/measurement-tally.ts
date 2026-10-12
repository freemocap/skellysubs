/** Content-free timing summaries. Totals across concurrent spans are not elapsed savings. */
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { loadBundle } from './bundle-import.ts'
import { digest } from './session.ts'
import { latencyReport, type SpanSample } from './latency-report.ts'

export function summarize(samples: SpanSample[]) {
  const groups = new Map<string, number[]>()
  for (const sample of samples) {
    if (!Number.isFinite(sample.durationMicros) || sample.durationMicros < 0) throw Error('Invalid duration.')
    const key = JSON.stringify([sample.clock, sample.span])
    const values = groups.get(key) ?? []
    values.push(sample.durationMicros / 1000); groups.set(key, values)
  }
  return [...groups].map(([key, values]) => {
    values.sort((a,b) => a-b)
    const [clock, span] = JSON.parse(key) as [string,string]
    return { clock, span, count: values.length, p50Ms: values[Math.ceil(values.length*.5)-1],
      p95Ms: values[Math.ceil(values.length*.95)-1], maxMs: values.at(-1)!,
      interpretation: span==='watch_conversation' ? 'long-poll wait included' :
        span==='next-frame' ? 'animation-frame callback; not proof of visible content' : 'IPC round trip; may include provider, lock and queue wait' }
  }).sort((a,b) => a.span.localeCompare(b.span))
}

export async function tally(recording: string, candidates: string[]) {
  const { bundle } = await loadBundle(recording)
  const recordingDigest = digest(await readFile(join(recording,'session.json')))
  const baseline = latencyReport(bundle)
  const runs = []
  for (const directory of candidates) {
    const receipt = JSON.parse(await readFile(join(directory,'replay-result.json'),'utf8'))
    if (receipt.completed!==true || receipt.recordingDigest!==recordingDigest) throw Error('Candidate must verify this exact recording before comparison.')
    const comparison = JSON.parse(await readFile(join(directory,'comparison.json'),'utf8'))
    if (comparison.baselineBuild!==bundle.build) throw Error('Candidate baseline identity does not match.')
    const report = JSON.parse(await readFile(join(directory,'replay-latency.json'),'utf8'))
    runs.push({ directory, build:comparison.candidateBuild, summaries:summarize(report.samples),
      missingEndpoints:report.missing.length, matched:comparison.pairs.length, unmatched:comparison.unmatched.length,
      pairedChanges:comparison.summaries.map((row: {clock:string;span:string;baseline:{count:number;p50Micros:number;p95Micros:number};candidate:{count:number;p50Micros:number;p95Micros:number}}) => ({
        clock:row.clock,span:row.span,count:row.baseline.count,
        p50SavedMs:(row.baseline.p50Micros-row.candidate.p50Micros)/1000,
        p95SavedMs:(row.baseline.p95Micros-row.candidate.p95Micros)/1000,
      })) })
  }
  return { format:1, recordingDigest, baselineBuild:bundle.build,
    status:runs.length ? 'verified candidates available' : 'capture observations only; replay not established',
    baseline:summarize(baseline.samples), missingEndpoints:baseline.missing.length, runs,
    limits:['No sum of overlapping timings is reported as wall-clock savings.',
      'Single runs do not establish statistical significance.',
      'Provider completion-to-visible-content is not yet causally measured.',
      'Use repeated same-build replays to establish variance before attributing gains.'] }
}

if (import.meta.main) {
  const [recording,...candidates] = process.argv.slice(2)
  if (!recording) throw Error('Usage: measurement-tally.ts RECORDING [VERIFIED_REPLAY ...]')
  console.log(JSON.stringify(await tally(recording,candidates),null,2))
}
