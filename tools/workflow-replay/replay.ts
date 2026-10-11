import { writeFile, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { loadBundle } from './bundle-import.ts'
import { launchDesktop } from './desktop-launch.ts'
import { installBrowserRecorder } from './browser-recorder.ts'
import { latencyReport, compareLatency } from './latency-report.ts'
import type { Bundle, TimedEvent, Event } from './session.ts'
import { digest } from './session.ts'
import { presentationStorage } from './browser-state.ts'
import { installBrowserObservation } from './browser-observation.ts'
import { calibrate } from './clock-calibration.ts'
import { buildIdentity } from './build-identity.ts'
import { readyAfterReload } from './browser-ready.ts'
import { wheelTargetMatches } from './wheel-target.ts'

import { hardwareAction, recordedWheel, type ActionDetail } from './browser-actions.ts'
import { replayOptions } from './options.ts'

export async function replayRecording(directory: string, options:{mute?:boolean}={}) {
  const recording = await loadBundle(directory)
  const observations: TimedEvent[] = []
  let observationBytes = 0
  let failure: string | undefined
  const origin = process.hrtime.bigint()
  const session = await launchDesktop((method, params) => {
    if (method !== 'Runtime.bindingCalled') return
    const payload = params as { name?: string; payload?: string }
    if (payload.name !== '__workflowSend' || !payload.payload) return
    if (failure) return
    observationBytes += Buffer.byteLength(payload.payload)
    if (observationBytes > 16 * 1024 * 1024 || observations.length >= 100000) { failure = 'Replay observation bound exceeded.'; return }
    const message = JSON.parse(payload.payload) as { event: TimedEvent | { control: string; reason?: string } }
    if ('control' in message.event) { failure = message.event.reason ?? 'Unexpected replay control event.'; return }
    if (message.event.kind === 'timing') observations.push({ ...message.event, sequence: observations.length, elapsedMicros: Number((process.hrtime.bigint() - origin) / 1000n) })
  }, recording, options)
  let currentAction: string | undefined
  let waitingFor = 'initialization'
  let actionGeometry: {target:string;x?:number;y?:number}|undefined
  try {
    const initial = recording.bundle.events.find(event => event.kind === 'display' && event.target === 'initial-browser-state')
    if (!initial || initial.kind !== 'display') throw new Error('Initial browser state is missing.')
    const state = JSON.parse(initial.text) as { storage: Record<string, string>; width: number; height: number; scale: number }
    if (!state.storage || !Number.isInteger(state.width) || !Number.isInteger(state.height) || state.width < 1 || state.height < 1 || state.width > 10000 || state.height > 10000) throw new Error('Invalid initial browser state.')
    for (const value of Object.values(state.storage)) if (typeof value !== 'string') throw new Error('Invalid browser storage value.')
    if (JSON.stringify(presentationStorage(state.storage)) !== JSON.stringify(state.storage)) throw new Error('Recording contains unsupported browser storage.')
    if (!Number.isFinite(state.scale) || state.scale<=0 || state.scale>10) throw new Error('Invalid display scale.')
    await session.connection.command('Emulation.setDeviceMetricsOverride', { width: state.width, height: state.height, deviceScaleFactor: state.scale, mobile: false })
    await session.connection.command('Runtime.addBinding', { name: '__workflowSend' })
    await session.connection.command('Page.enable')
    await readyAfterReload(session.connection)
    await session.connection.evaluate(`(${installBrowserObservation.toString()})();(${installBrowserRecorder.toString()})()`)
    await writeFile(join(session.directory,'clock-calibration.json'),JSON.stringify(await calibrate(session.connection),null,2))
    await session.connection.evaluate(`window.__workflowState('replaying')`)
    const actions = recording.bundle.events.filter(event => event.kind === 'action')
    const recordingStart=recording.bundle.events.find(event=>event.kind==='display'&&event.target==='recording-start')
    if(!recordingStart)throw Error('Recording activation boundary is missing.')
    const firstAction = recordingStart.elapsedMicros
    const replayStarted = performance.now()
    let dispatched = 0
    for (const event of recording.bundle.events) {
      if (event.kind !== 'action') continue
      if (failure) throw new Error(failure)
      const remaining = (event.elapsedMicros - firstAction) / 1000 - (performance.now() - replayStarted)
      if (remaining > 0) await new Promise(yes => setTimeout(yes, remaining))
      if (failure) throw new Error(failure)
      const value = JSON.parse(event.value) as { id: string; detail: ActionDetail | null; screen?: string }
      currentAction = value.id
      const target = JSON.stringify(event.target)
      const detail = value.detail ?? {}
      actionGeometry = {target:event.target,x:detail.x,y:detail.y}
      if (!/^action-[1-9][0-9]*$/.test(value.id)) throw Error('Recording lacks an explicit action identity.')
      // Whole-screen intermediate text includes independently updating connection
      // and progress labels. Preserve it as evidence, not an ordering dependency.
      // The target and final rendered/media checkpoints are acceptance boundaries.
      // Cursor trajectories are coordinate observations, not semantic selections.
      // Content under a moving pointer may change while independent work finishes.
      const cursorMove = event.action === 'pointer' && detail.type === 'pointermove'
      waitingFor = 'target-exists'
      if (!cursorMove) await waitFor(`!!document.querySelector(${target})`)
      if (!cursorMove && (event.action === 'pointer' || event.action === 'wheel')) {
        waitingFor = 'target-hit-test'
        await waitFor(event.action === 'wheel'
          ? `(${wheelTargetMatches.toString()})(${target},${JSON.stringify(detail.x)},${JSON.stringify(detail.y)})`
          : `(()=>{const e=document.querySelector(${target});const hit=document.elementFromPoint(${JSON.stringify(detail.x)},${JSON.stringify(detail.y)});return e===hit || e.contains(hit)})()`)
      }
      const eventType = detail.type ?? detail.eventType ?? (event.action === 'focus' ? 'focusin' : event.action)
      await session.connection.evaluate(`window.__workflowReplayAction(${JSON.stringify(value.id)},${JSON.stringify(eventType)},${JSON.stringify(detail)})`)
      const clipboardKey = event.action==='key' && (detail.ctrl || detail.meta) && ['v','x','a','z','y'].includes(detail.key?.toLowerCase()??'')
      const textEditingKey = event.action==='key' && (detail.key?.length===1 || ['Backspace','Delete','Enter'].includes(detail.key??'')) && await session.connection.evaluate<boolean>(`(()=>{const e=document.querySelector(${target});return e instanceof HTMLTextAreaElement || (e instanceof HTMLInputElement && ['text','search','email','url','tel','number'].includes(e.type))})()`)
      // Recorded input events own text edits. Synthetic key events still execute
      // application shortcuts without duplicating native editing side effects.
      const hardware = clipboardKey || textEditingKey || detail.composing || event.action==='wheel' ? null : hardwareAction(event.action,detail)
      if (hardware) {
        await session.connection.command(hardware.method,hardware.params)
        // CDP acknowledgment can precede DOM delivery (notably wheel events).
        // Do not overwrite the event identity until the page consumes it.
        waitingFor = 'input-acknowledgment'
        await waitFor(`window.__workflowReplayPending()!==${JSON.stringify(value.id)}`)
      }
      else if (event.action === 'wheel') {
        await session.connection.evaluate(`document.querySelector(${target}).dispatchEvent(new WheelEvent('wheel',${JSON.stringify(recordedWheel(detail))}))`)
      }
      else {
        await session.connection.evaluate(`(()=>{const e=document.querySelector(${target});const v=${JSON.stringify(detail)};switch(${JSON.stringify(event.action)}){case 'key':e.dispatchEvent(new KeyboardEvent(v.type,{bubbles:true,cancelable:true,key:v.key,code:v.code,keyCode:v.keyCode,location:v.location,repeat:v.repeat,isComposing:v.composing,ctrlKey:v.ctrl,metaKey:v.meta,altKey:v.alt,shiftKey:v.shift}));break;case 'click':if(e.disabled)throw Error('Disabled target');if(v.type==='dblclick')e.dispatchEvent(new MouseEvent('dblclick',{bubbles:true,detail:2,ctrlKey:v.ctrl,metaKey:v.meta,altKey:v.alt,shiftKey:v.shift}));else e.click();break;case 'focus':e.focus();break;case 'scroll':e.scrollTo({left:v.x,top:v.y,behavior:'instant'});break;case 'composition':e.dispatchEvent(new CompositionEvent(v.eventType,{bubbles:true,data:v.data??''}));break;case 'input':{const p=e instanceof HTMLTextAreaElement?HTMLTextAreaElement.prototype:e instanceof HTMLSelectElement?HTMLSelectElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(p,'value').set.call(e,v.value);if(typeof v.checked==='boolean')e.checked=v.checked;e.dispatchEvent(v.eventType==='change'?new Event('change',{bubbles:true}):new InputEvent('input',{bubbles:true,data:v.data,inputType:v.inputType,isComposing:v.composing}));break;}}})()`)
      }
      dispatched++
    }
    const final = [...recording.bundle.events].reverse().find(event => event.kind === 'display' && event.target === '#root')
    if (!final || final.kind !== 'display') throw new Error('Final rendered checkpoint is missing.')
    waitingFor = 'final-rendered-state'
    await waitFor(`document.getElementById('root')?.innerText===${JSON.stringify(final.text)}`)
    const checkpoint = recording.bundle.events.find(event=>event.kind==='display'&&event.target==='final-checkpoint')
    if (!checkpoint || checkpoint.kind!=='display') throw new Error('Final behavior checkpoint is missing.')
    waitingFor = 'final-behavior-checkpoint'
    await waitFor(`window.__workflowCheckpoint().then(value=>JSON.stringify(value)===${JSON.stringify(checkpoint.text)})`)
    await session.connection.evaluate(`window.__workflowState('saving')`)
    for(const event of await session.connection.evaluate<Event[]>(`window.__workflowTimings()`))observations.push({...event,sequence:observations.length,elapsedMicros:Number((process.hrtime.bigint()-origin)/1000n)})
    await session.connection.evaluate(`window.__TAURI_INTERNALS__.invoke('workflow_control',{action:'stop'})`)
    session.verifyNetwork()
    if (failure) throw new Error(failure)
    if (await buildIdentity() !== session.build) throw new Error('Build changed during replay; comparison rejected.')
    const measured: Bundle = { ...recording.bundle, build:session.build, events: [...recording.bundle.events.filter(event=>event.kind!=='timing'),...observations] }
    await writeFile(join(session.directory, 'replay-latency.json'), JSON.stringify(latencyReport(measured), null, 2))
    await writeFile(join(session.directory,'comparison.json'),JSON.stringify(compareLatency(recording.bundle,measured),null,2))
    await writeFile(join(session.directory,'builds.json'),JSON.stringify({baseline:recording.bundle.build,candidate:measured.build},null,2))
    await writeFile(join(session.directory, 'replay-result.json'), JSON.stringify({ completed: true, outputMuted:options.mute??false, recordingDigest:digest(await readFile(join(directory,'session.json'))), recordedActions: actions.length, dispatchedActions:dispatched, renderedCheckpoint: true, playbackCheckpoint:true, providerTapeConsumed: true }, null, 2))
    return session.directory
  } catch (error) {
    const screen = await session.connection.evaluate<string>(`document.getElementById('root')?.innerText ?? ''`).catch(() => 'Unavailable')
    const geometry = actionGeometry && await session.connection.evaluate(`(()=>{const a=${JSON.stringify(actionGeometry)};const e=document.querySelector(a.target);const hit=Number.isFinite(a.x)&&Number.isFinite(a.y)?document.elementFromPoint(a.x,a.y):null;const describe=e=>e?{tag:e.tagName,box:e.getBoundingClientRect().toJSON(),scrollTop:e.scrollTop,scrollLeft:e.scrollLeft}:null;return {target:describe(e),hit:describe(hit),pending:window.__workflowReplayPending?.()}})()`).catch(()=>null)
    await writeFile(join(session.directory, 'replay-checkpoint.json'), JSON.stringify({ currentAction, waitingFor, geometry, screen, input: await session.connection.evaluate(`window.__workflowUnexpected ?? null`).catch(()=>null) }, null, 2))
    await writeFile(join(session.directory, 'replay-result.json'), JSON.stringify({ completed: false, reason: error instanceof Error ? error.message : 'Replay failed.' }))
    throw error
  } finally { await session.close() }

  async function waitFor(expression: string) {
    const deadline = Date.now() + 30000
    while (!await session.connection.evaluate<boolean>(expression).catch(() => false)) {
      if (failure) throw new Error(failure)
      if (Date.now() > deadline) throw new Error('Recorded application state was not reached; replay stopped.')
      await new Promise(yes => setTimeout(yes, 10))
    }
  }
}

if (import.meta.main) {
  const options=replayOptions(process.argv.slice(2),true)
  console.log(`Replay verified. Evidence: ${await replayRecording(options.recording,options)}`)
}





