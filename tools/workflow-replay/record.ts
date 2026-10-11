import { writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { launchDesktop } from './desktop-launch.ts'
import { installBrowserRecorder } from './browser-recorder.ts'
import { SessionCapture, type Event } from './session.ts'
import { captureWorkspace } from './workspace-bundle.ts'
import { preserveCapture, saveRecording } from './save-recording.ts'
import { presentationStorage } from './browser-state.ts'
import { installBrowserObservation } from './browser-observation.ts'
import { calibrate } from './clock-calibration.ts'
import { buildIdentity } from './build-identity.ts'
import { readyAfterReload } from './browser-ready.ts'
import {workflowControl} from './control.ts'
import {replayOptions} from './options.ts'

/** Local explicit recording. Complete export requires native transport stop and
 * a consistent starting workspace; failure never produces a complete manifest. */
export async function recordingSession() {
  let capture: SessionCapture | undefined
  let startTime = 0n
  let uiOffsetMicros = 0
  let accepting = false
  let queue = Promise.resolve()
  let queuedBytes = 0
  let queuedEvents = 0
  let savedDirectory:string|undefined
  const failures: string[] = []
  let stage = 'initialization'
  let failureSaving: Promise<void> | undefined
  function failRecording(error: unknown): Promise<void> {
    if (failureSaving) return failureSaving
    accepting = false
    failures.push(error instanceof Error ? error.message : 'Recording failed.')
    failureSaving = (async () => {
      try { await preserveCapture(capture, session.directory, stage) }
      catch { failures.push('Partial capture export failed; inspect retained session files.') }
      await writeFile(join(session.directory, 'incomplete.json'), JSON.stringify({ complete: false, stage, failures }))
      await session.connection.evaluate(`window.__workflowState('incomplete — see session report')`).catch(() => {})
    })()
    return failureSaving
  }
  const session = await launchDesktop((method, params) => {
    if (method !== 'Runtime.bindingCalled') return
    const message = params as { name?: string; payload?: string }
    if (message.name !== '__workflowSend' || !message.payload) return
    const payload = message.payload
    const receivedMicros=startTime ? Number((process.hrtime.bigint()-startTime)/1000n) : 0
    const size = Buffer.byteLength(payload)
    if (failures.length) return
    if (queuedBytes + size > 8 * 1024 * 1024 || queuedEvents >= 10000) {
      stage = 'event_queue'
      void failRecording(new Error('Browser capture queue exceeded its limit.'))
      return
    }
    queuedBytes += size; queuedEvents++
    queue = queue.then(async () => {
      if (failures.length) return
      if (Buffer.byteLength(payload) > 1024 * 1024) throw new Error('Browser event exceeds capture bound.')
      const item = JSON.parse(payload) as { micros:number;event: Event | { control: string; reason?: string } }
      if ('control' in item.event) {
        if (item.event.control === 'start') {
          stage = 'capture_start'
          if (capture) throw new Error('This session already has a recording.')
          const build = session.build
          if(await buildIdentity()!==build)throw Error('Build changed before recording started.')
          const result = await workflowControl<{workspace:string}>(session.connection,'start')
          capture = new SessionCapture({ contentConsent: true, build, maxBytes: 256 * 1024 * 1024, maxEvents: 100000 })
          await captureWorkspace(result.workspace, capture)
          const browser = await session.connection.evaluate<{ storage: Record<string,string>; width: number; height: number; scale: number }>(`({storage:(${presentationStorage.toString()})(Object.fromEntries(Object.entries(localStorage))),width:innerWidth,height:innerHeight,scale:devicePixelRatio})`)
          const browserState = JSON.stringify(browser)
          startTime = process.hrtime.bigint()
          capture.append({ kind: 'display', target: 'initial-browser-state', text: browserState }, elapsed())
          await session.connection.command('Page.enable')
          await session.connection.evaluate(`window.__workflowReloadMarker=true`)
          await session.connection.command('Page.reload')
          await readyAfterReload(session.connection)
          await session.connection.evaluate(`(${installBrowserObservation.toString()})();(${installBrowserRecorder.toString()})()`)
          capture.append({kind:'display',target:'clock-calibration',text:JSON.stringify(await calibrate(session.connection))},elapsed())
          accepting = true
          const uiStart=await session.connection.evaluate<number>(`(()=>{window.__workflowState('recording');return Math.round(performance.now()*1000)})()`)
          const started=elapsed()
          uiOffsetMicros=started-uiStart
          capture.append({kind:'display',target:'recording-start',text:JSON.stringify({uiMicros:uiStart})},started)
          stage = 'recording'
        } else if (item.event.control === 'stop') {
          if (!capture) throw new Error('Recording has not started.')
          stage = 'build_identity'
          if(await buildIdentity()!==session.build)throw Error('Build changed during recording.')
          accepting = false
          savedDirectory = await saveRecording(capture, session.directory, elapsed, {
            timings: () => session.connection.evaluate<Event[]>(`window.__workflowTimings()`),
            stop: () => workflowControl<{providerTape:string}>(session.connection, 'stop'),
            text: () => session.connection.evaluate<string>(`document.getElementById('root').innerText`),
            checkpoint: async () => {
              // Only authored observation errors cross this diagnostic boundary.
              const result = await session.connection.evaluate<{value?:unknown;failure?:string}>(`window.__workflowCheckpoint().then(value=>({value}),error=>({failure:['Playback observation did not finish.','Playback bytes could not be observed.','Unhandled browser error.','Unhandled browser rejection.','Unsupported audio worklet observation.','Wait for playback to finish before saving the recording.'].includes(error?.message)?error.message:'Unclassified checkpoint failure.'}))`)
              if (result.failure) throw Error(result.failure)
              return result.value
            },
            stage: value => { stage = value },
          })
          // The export is authoritative even if its window has just closed.
          await session.connection.evaluate(`window.__workflowState('saved locally')`).catch(() => {})
        } else throw new Error(item.event.reason ?? 'Recording failed.')
      } else if (accepting && capture) {
        if(!Number.isSafeInteger(item.micros)||item.micros<0)throw Error('Browser event clock is invalid.')
        capture.append(item.event,item.event.kind==='action'?item.micros+uiOffsetMicros:receivedMicros)
      }
    }).catch(failRecording).finally(() => { queuedBytes -= size; queuedEvents-- })
  })
  const elapsed = () => Number((process.hrtime.bigint() - startTime) / 1000n)
  await session.connection.command('Runtime.addBinding', { name: '__workflowSend' })
  await session.connection.evaluate(`(${installBrowserRecorder.toString()})()`)
  return { ...session, flush: () => queue, failures,waitForCompletion:async()=>{
    while(!savedDirectory){
      if(failures.length){await failureSaving;throw Error(failures[0])}
      if(!session.isRunning()){await queue;stage='application_closed';await failRecording(Error('Recording application closed before export.'));throw Error(failures[0])}
      await new Promise(yes=>setTimeout(yes,100))
    }
    await queue
    return savedDirectory
  } }
}

if (import.meta.main) {
  const options=replayOptions(process.argv.slice(2),false)
  const session = await recordingSession()
  console.log(`Recording controls installed. Local session: ${session.directory}`)
  console.log('Configure the isolated app, then Start recording. After Stop, leave the replay window alone while the captured workflow is verified.')
  process.once('SIGINT', () => { void session.flush().then(() => session.close()).then(() => process.exit()) })
  try{
    const directory=await session.waitForCompletion()
    await session.close()
    console.log('Capture saved. Replaying offline; please do not interact with the replay window.')
    const {replayRecording}=await import('./replay.ts')
    console.log(`Recording verified. Bundle: ${directory}\nComparison: ${await replayRecording(directory,options)}`)
  }finally{await session.close()}
}
