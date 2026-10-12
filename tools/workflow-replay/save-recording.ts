import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { SessionCapture, type Event } from './session.ts'
import { latencyReport } from './latency-report.ts'
import { nativeTimingReport } from './native-timing.ts'

/** Incomplete evidence is inspectable but remains ineligible for replay. */
export async function preserveCapture(capture: SessionCapture | undefined, directory: string, stage: string) {
  if (!capture) return
  const reason = `Recording failed during ${stage}.`
  capture.stop(reason)
  await capture.export(join(directory, 'partial-recording'), reason)
  await writeFile(join(directory, 'partial-latency.json'), JSON.stringify({
    status: 'incomplete', stage, ...latencyReport(capture.snapshot(), true),
  }, null, 2), { flag: 'wx' })
}

export async function saveRecording(capture: SessionCapture, directory: string, elapsed: () => number, ports: {
  timings: () => Promise<Event[]>
  stop: () => Promise<{ providerTape: string }>
  text: () => Promise<string>
  checkpoint: () => Promise<unknown>
  stage: (stage: string) => void
}) {
  ports.stage('timing_collection')
  for (const timing of await ports.timings()) capture.append(timing, elapsed())
  ports.stage('native_stop')
  const result = await ports.stop()
  ports.stage('provider_tape')
  const tape = await readFile(result.providerTape)
  capture.setProviderTape(tape)
  ports.stage('final_display')
  capture.append({ kind: 'display', target: '#root', text: await ports.text() }, elapsed())
  ports.stage('final_checkpoint')
  capture.append({ kind: 'display', target: 'final-checkpoint', text: JSON.stringify(await ports.checkpoint()) }, elapsed())
  ports.stage('validation')
  const nativeLatency = nativeTimingReport(tape)
  capture.stop()
  const latency = latencyReport(capture.snapshot())
  ports.stage('export')
  const destination = join(directory, 'recording')
  await capture.export(destination)
  await writeFile(join(directory, 'latency.json'), JSON.stringify(latency, null, 2))
  await writeFile(join(directory, 'native-latency.json'), JSON.stringify(nativeLatency, null, 2))
  return destination
}
