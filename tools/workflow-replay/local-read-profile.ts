/** Focused IPC measurements on an explicitly supplied consistent workspace export.
 * Restores a disposable copy, blocks provider traffic and never changes the source.
 * This is a local-read workload, not a successful full-session replay receipt.
 */
import { writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { SessionCapture } from './session.ts'
import { captureWorkspace } from './workspace-bundle.ts'
import { loadBundle } from './bundle-import.ts'
import { launchDesktop } from './desktop-launch.ts'
import { buildIdentity } from './build-identity.ts'

const source = process.argv[2]
if (!source) throw Error('Supply a consistent workspace export directory.')
const options=process.argv.slice(3)
if(options.some(option=>!['--profile','--startup-only','--warm'].includes(option)) || new Set(options).size!==options.length)
  throw Error('Supported profiling options: --profile --startup-only --warm')
const capture = new SessionCapture({ contentConsent: true, build: await buildIdentity(), maxBytes: 256 * 1024 * 1024, maxEvents: 10 })
await captureWorkspace(resolve(source), capture)
capture.setProviderTape(Buffer.from(JSON.stringify({ format: 1, records: [] })))
capture.append({ kind: 'display', target: 'initial-browser-state', text: JSON.stringify({ storage: {} }) }, 0)
capture.stop()
const exportPath = resolve('.local', `read-profile-${Date.now()}`)
await capture.export(exportPath)
const trace: unknown[] = []
let traceOverflow=false
let finishTrace!: () => void
const traceFinished = new Promise<void>(resolve => { finishTrace = resolve })
const desktop = await launchDesktop((method, params) => {
  if (method === 'Tracing.tracingComplete') finishTrace()
  if (method === 'Tracing.dataCollected') {
    for (const event of (params as {value:Array<{name:string;ph:string;ts:number;dur?:number;pid:number;tid:number}>}).value)
      if (event.ph === 'X') {
        if(trace.length>=200000)traceOverflow=true
        else trace.push({name:event.name,ts:event.ts,dur:event.dur,pid:event.pid,tid:event.tid})
      }
  }
}, await loadBundle(exportPath), { mute: true })
try {
  const profile = process.argv.includes('--profile')
  if (process.argv.includes('--warm')) {
    await new Promise(resolve=>setTimeout(resolve,1500))
    const previousOrigin=await desktop.connection.evaluate('performance.timeOrigin')
    await desktop.connection.command('Page.reload')
    const deadline=Date.now()+5000
    while (true) {
      try { if(await desktop.connection.evaluate(`performance.timeOrigin !== ${Number(previousOrigin)} && !!document.getElementById('root') && typeof window.__TAURI_INTERNALS__?.invoke==='function'`)) break } catch {}
      if(Date.now()>deadline)throw Error('Warm reload did not become ready.')
      await new Promise(resolve=>setTimeout(resolve,50))
    }
  }
  if (profile) {
    await desktop.connection.command('Profiler.enable')
    await desktop.connection.command('Profiler.start')
    await desktop.connection.command('Tracing.start', {categories:'devtools.timeline,toplevel,blink,cc',transferMode:'ReportEvents'})
    await desktop.connection.evaluate(`window.__TAURI_INTERNALS__.invoke('workflow_control',{action:'local_timings_start'})`)
  }
  const result = await desktop.connection.evaluate(`(async () => {
    const originalInvoke = window.__TAURI_INTERNALS__.invoke;
    const invocations = [];
    window.__workflowIpc = command => {
      const start=performance.now();
      return ()=>invocations.push({command,start,duration:performance.now()-start,application:true});
    };
    const invoke = (command,args) => {
      const start=performance.now();
      const result=originalInvoke(command,args);
      const dispatched=performance.now();
      return result.finally(()=>invocations.push({command,start,dispatch:dispatched-start,duration:performance.now()-start}));
    };
    const snapshot = await invoke('get_snapshot');
    const target = snapshot.conversations[0]?.languageId ?? snapshot.languageProfiles[0]?.languageId ?? snapshot.languages[0].id;
    const samples = [];
    const longFrames = [];
    const observer = PerformanceObserver.supportedEntryTypes.includes('long-animation-frame')
      ? new PerformanceObserver(list => longFrames.push(...list.getEntries().map(entry => ({
        start:entry.startTime,duration:entry.duration,blocking:entry.blockingDuration,
        renderStart:entry.renderStart,styleAndLayoutStart:entry.styleAndLayoutStart,
        scripts:entry.scripts.map(script => ({duration:script.duration,invoker:script.invoker,
          function:script.sourceFunctionName,url:script.sourceURL,position:script.sourceCharPosition,
          layout:script.forcedStyleAndLayoutDuration}))
      })))) : null;
    observer?.observe({type:'long-animation-frame',buffered:true});
    const frames = [];
    const taskDelays = [];
    let taskAt=performance.now();
    const taskTimer=setInterval(()=>{const now=performance.now();taskDelays.push(now-taskAt-10);taskAt=now},10);
    let alive = true, lastFrame = performance.now();
    const frame = now => { frames.push(now - lastFrame); lastFrame = now; if (alive) requestAnimationFrame(frame); };
    requestAnimationFrame(frame);
    const operations = [['get_snapshot', {}], ['get_language_totals', {}], ['get_skill_evidence', {target}]];
    const measure = async ([command, args], mode) => {
      const start = performance.now();
      await invoke(command, args);
      samples.push({command, mode, millis: performance.now() - start});
    };
    // Startup-only separates app hydration from the synthetic repeated-read load.
    const startupOnly = ${process.argv.includes('--startup-only')};
    if (!startupOnly) for (const operation of operations) await measure(operation, 'warmup');
    if (!${profile}) await invoke('workflow_control', {action:'local_timings_start'});
    if (startupOnly) await new Promise(resolve=>setTimeout(resolve,3500));
    else {
      for (let i=0;i<20;i++) for (const operation of operations) await measure(operation, 'serial');
      for (let i=0;i<20;i++) await Promise.all(operations.map(operation => measure(operation, 'concurrent')));
    }
    const native = await invoke('workflow_control', {action:'local_timings_finish'});
    alive = false;
    clearInterval(taskTimer);
    const totals = await invoke('get_language_totals');
    for (const total of totals) {
      const evidence = await invoke('get_skill_evidence', {target:total.target});
      if (evidence.profile.xp !== total.xp || evidence.conversation_count !== total.conversations)
        throw Error('Totals do not match the corresponding evidence projection.');
    }
    observer?.disconnect();
    return {startupOnly,samples,native,frames,longFrames,invocations,taskDelays,visibility:document.visibilityState,
      dom:{elements:document.querySelectorAll('*').length,turns:document.querySelectorAll('.turn-stack').length,
        fonts:Array.from(document.fonts).filter(face=>face.status==='loaded').map(face=>face.family)},verifiedLanguages:totals.length};
  })()`)
  if (profile) {
    await writeFile(join(desktop.directory, 'startup.cpuprofile'), JSON.stringify(await desktop.connection.command('Profiler.stop')))
    await desktop.connection.command('Tracing.end')
    let timeout: ReturnType<typeof setTimeout> | undefined
    try { await Promise.race([traceFinished,new Promise((_,reject)=>{timeout=setTimeout(()=>reject(Error('Trace completion timed out.')),5000)})]) }
    finally { clearTimeout(timeout) }
    if(traceOverflow)throw Error('Startup trace exceeded its event bound.')
    await writeFile(join(desktop.directory,'startup-trace.json'),JSON.stringify(trace))
  }
  desktop.verifyNetwork()
  await writeFile(join(desktop.directory, 'local-read-profile.json'), JSON.stringify({build:desktop.build, options, result}, null, 2))
  // Include only build and timing data in command output.
  console.log(JSON.stringify({directory:desktop.directory, build:desktop.build}))
} finally { await desktop.close() }
