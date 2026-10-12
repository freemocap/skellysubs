/** Injected only by the explicit desktop harness. No ordinary application import. */
export function installBrowserRecorder() {
  const host = window as unknown as { __workflowSend: (message: string) => void; __workflowState: (state: string) => void; __workflowObserve?: (active: boolean) => void; __workflowTimings: () => Promise<unknown[]>; __workflowIpc: (command:string)=>(()=>void)|undefined; __workflowReplayPending: () => string | undefined; __workflowReplayAction: (id: string, eventType?: string, detail?: Record<string,unknown>) => void }
  let active = false
  let sequence = 0
  let actionScope: string | undefined
  let replayScope: string | undefined
  let replayEventType: string | undefined
  let replayDetail: Record<string,unknown> = {}
  let replaying = false
  let scopeEpoch = 0
  // Native DOM dispatch can run a microtask checkpoint between capture and
  // React's bubbling listener. Keep ownership through the dispatch; the timer
  // is a fallback when an application listener stops propagation.
  const scope = (id: string) => { actionScope=id; const epoch=++scopeEpoch; setTimeout(()=>{if(scopeEpoch===epoch)actionScope=undefined},0) }
  host.__workflowReplayAction=(id,eventType,detail={})=>{if(!/^action-[1-9][0-9]*$/.test(id))throw Error('Invalid replay action identity.');replaying=true;replayScope=id;replayEventType=eventType;replayDetail=detail}
  host.__workflowReplayPending=()=>replayScope
  let lastPointer: unknown
  let lastPointerAction: string | undefined
  let lastKeyboardAction: string | undefined
  let pendingIpc = 0
  const commandCounts = new Map<string,number>()
  const timings: unknown[] = []
  let lastKeyTime = -Infinity
  const panel = document.createElement('aside')
  panel.id = 'workflow-recorder'
  panel.style.cssText = 'position:fixed;top:8px;left:8px;z-index:2147483647;padding:10px;background:#18232d;color:white;border:1px solid #8193a3;font:14px system-ui;display:flex;gap:10px;align-items:center'
  const label = document.createElement('span'); label.textContent = 'Workflow recording: ready'
  const start = document.createElement('button'); start.textContent = 'Start recording (reloads view)'
  const stop = document.createElement('button'); stop.textContent = 'Stop recording'; stop.disabled = true
  panel.append(label, start, stop); document.body.append(panel)
  label.style.cursor='move'
  let drag: {x:number;y:number;left:number;top:number}|undefined
  label.addEventListener('pointerdown',event=>{if(event.button!==0)return;const box=panel.getBoundingClientRect();drag={x:event.clientX,y:event.clientY,left:box.left,top:box.top};label.setPointerCapture(event.pointerId)})
  label.addEventListener('pointermove',event=>{if(!drag)return;panel.style.left=`${Math.max(0,Math.min(innerWidth-panel.offsetWidth,drag.left+event.clientX-drag.x))}px`;panel.style.top=`${Math.max(0,Math.min(innerHeight-panel.offsetHeight,drag.top+event.clientY-drag.y))}px`})
  label.addEventListener('pointerup',()=>{drag=undefined})
  label.addEventListener('pointercancel',()=>{drag=undefined})
  const send = (event: unknown) => {
    if ((event as {kind?:string}).kind==='timing') {
      if(timings.length>=100000) {host.__workflowState('incomplete');host.__workflowSend(JSON.stringify({event:{control:'failure',reason:'Timing observation limit exceeded.'}}));return}
      timings.push(event);return
    }
    host.__workflowSend(JSON.stringify({ micros: Math.round(performance.now() * 1000), event }))
  }
  host.__workflowTimings=async()=>{
    // Long-poll subscriptions may intentionally outlive recording. Their missing
    // endpoints stay explicit in the report; never invent a completion time.
    await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame)
    return timings
  }
  host.__workflowState = state => {
    active = state === 'recording' || state === 'replaying'; label.textContent = `Workflow recording: ${state}`
    if(state==='replaying')replaying=true
    host.__workflowObserve?.(active)
    start.disabled = state !== 'ready'; stop.disabled = state !== 'recording'
  }
  start.onclick = () => { host.__workflowState('preparing'); send({ control: 'start' }) }
  stop.onclick = () => { host.__workflowState('saving'); send({ control: 'stop' }) }
  const selector = (element: Element, control: boolean): string => {
    // Record a uniquely named control rather than its incidental icon child.
    // Ambiguous names retain the exact structural target; never pick a sibling.
    const owner = control ? element.closest('button,input,textarea,select,[role="button"],[role="radio"],[role="separator"]') : null
    const label = owner?.getAttribute('aria-label')
    if (owner && label) {
      const named = `${owner.tagName.toLowerCase()}[aria-label="${CSS.escape(label)}"]`
      const matches = document.querySelectorAll(named)
      if (matches.length === 1 && matches[0] === owner) return named
    }
    if (element.id) return `#${CSS.escape(element.id)}`
    const parts: string[] = []
    let item: Element | null = element
    while (item && item !== document.body) {
      const parent: Element | null = item.parentElement
      const siblings: Element[] = parent ? Array.from(parent.children).filter(sibling => sibling.tagName === item!.tagName) : []
      parts.unshift(`${item.tagName.toLowerCase()}:nth-of-type(${siblings.indexOf(item) + 1})`)
      item = parent
    }
    return `body>${parts.join('>')}`
  }
  const action = (kind: string, event: Event) => {
    if (!active || !(event.target instanceof Element) || panel.contains(event.target)) return
    const matchingFields = !(event instanceof MouseEvent || event instanceof KeyboardEvent) || (
      [['ctrl','ctrlKey'],['meta','metaKey'],['alt','altKey'],['shift','shiftKey']].every(([recorded,actual])=>replayDetail[recorded]===undefined || replayDetail[recorded]===(event as unknown as Record<string,unknown>)[actual]) &&
      (!(event instanceof MouseEvent) || [['x','clientX'],['y','clientY'],['buttons','buttons'],['button','button']].every(([recorded,actual])=>replayDetail[recorded]===undefined || replayDetail[recorded]===(event as unknown as Record<string,unknown>)[actual])) &&
      (!(event instanceof KeyboardEvent) || ['key','code','repeat','location'].every(key=>replayDetail[key]===undefined || replayDetail[key]===(event as unknown as Record<string,unknown>)[key])) &&
      (!(event instanceof WheelEvent) || [['dx','deltaX'],['dy','deltaY'],['mode','deltaMode']].every(([recorded,actual])=>replayDetail[recorded]===undefined || replayDetail[recorded]===(event as unknown as Record<string,unknown>)[actual]))
    )
    const commanded = !!replayScope && (!replayEventType || event.type===replayEventType) && matchingFields
    if(replaying && !commanded && ['key','pointer','wheel'].includes(kind)) {
      ;(window as unknown as {__workflowUnexpected:unknown}).__workflowUnexpected={type:event.type,expected:replayEventType,scope:replayScope,lastPointer,current:event instanceof MouseEvent?{x:event.clientX,y:event.clientY,buttons:event.buttons,movementX:event.movementX,movementY:event.movementY}:null}
      host.__workflowState('incomplete');send({control:'failure',reason:'Unexpected external input interrupted replay: '+event.type});return
    }
    if(kind==='pointer' && event instanceof MouseEvent)lastPointer={type:event.type,x:event.clientX,y:event.clientY,buttons:event.buttons,time:performance.now()}
    const element = event.target
    if ((kind === 'pointer' && event instanceof PointerEvent && event.pointerType !== 'mouse') || (event instanceof WheelEvent && event.deltaMode !== 0) || (element instanceof HTMLInputElement && element.type === 'file') || element.closest('[contenteditable=true]')) {
      host.__workflowState('incomplete'); send({control:'failure',reason:'Unsupported touch, pen, file, contenteditable or non-pixel wheel interaction.'});return
    }
    if (kind === 'input') {
      if (element instanceof HTMLInputElement && ['checkbox', 'radio', 'button', 'submit'].includes(element.type)) return
      if (element instanceof HTMLSelectElement && event.type !== 'change') return
      if (!(element instanceof HTMLSelectElement) && event.type === 'change') return
    }
    if (event instanceof KeyboardEvent) lastKeyTime = performance.now()
    if (kind === 'click' && event instanceof MouseEvent && event.type !== 'dblclick') {
      const derived = event.detail > 0 ? lastPointerAction : event.isTrusted && performance.now()-lastKeyTime<200 ? lastKeyboardAction : undefined
      if(derived){scope(derived);return} // Hardware dispatch supplies this activation.
    }
    if (element instanceof HTMLInputElement && (element.type === 'password' || /password|cc-number|cc-csc/.test(element.autocomplete))) {
      host.__workflowState('incomplete'); send({ control: 'failure', reason: 'Sensitive field interaction is excluded from recording.' }); return
    }
    // Browser-generated focus/input events during replay must never borrow a
    // future recorded action's identity. Keep them measurable but unpaired.
    const id = (commanded ? replayScope : undefined) ?? (replaying ? `derived-${performance.timeOrigin}-${++sequence}` : `action-${++sequence}`)
    if(commanded){replayScope=undefined;replayEventType=undefined}
    scope(id)
    if(kind==='pointer' && event.type==='pointerup')lastPointerAction=id
    if(kind==='key')lastKeyboardAction=id
    const target = selector(element, kind !== 'scroll' && kind !== 'wheel')
    let value: unknown = null
    if (event instanceof KeyboardEvent) {
      value = { type:event.type, key: event.key, code: event.code, keyCode:event.keyCode, location:event.location, repeat:event.repeat, composing:event.isComposing, ctrl: event.ctrlKey, meta: event.metaKey, alt: event.altKey, shift: event.shiftKey }
    } else if (event instanceof WheelEvent) value = {x:event.clientX,y:event.clientY,dx:event.deltaX,dy:event.deltaY,mode:event.deltaMode,ctrl:event.ctrlKey,shift:event.shiftKey,alt:event.altKey,meta:event.metaKey}
    else if (event instanceof MouseEvent) value = { x: event.clientX, y: event.clientY, button: event.button, buttons: event.buttons, type: event.type, clicks: event.detail, pointerType:event instanceof PointerEvent?event.pointerType:undefined, ctrl:event.ctrlKey,meta:event.metaKey,alt:event.altKey,shift:event.shiftKey }
    else if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) value = { value: element.value, checked: element instanceof HTMLInputElement ? element.checked : undefined, eventType: event.type, data: event instanceof CompositionEvent || event instanceof InputEvent ? event.data : null, composing: event instanceof InputEvent ? event.isComposing : false, inputType: event instanceof InputEvent ? event.inputType : undefined }
    else if (kind === 'scroll') value = { x: element.scrollLeft, y: element.scrollTop }
    const screen = kind === 'pointer' || kind === 'scroll' || kind==='wheel' ? undefined : document.getElementById('root')?.innerText
    send({ kind: 'action', action: kind, target, value: JSON.stringify({ id, detail: value, screen }) })
    if (event.type !== 'pointermove' && kind !== 'scroll') {
      const trace = id
      send({ kind: 'timing', trace, span: 'next-frame', phase: 'start', clock: 'ui', micros: Math.round(performance.now() * 1000) })
      requestAnimationFrame(() => send({ kind: 'timing', trace, span: 'next-frame', phase: 'end', clock: 'ui', micros: Math.round(performance.now() * 1000) }))
    }
  }
  for (const [name, kind] of [['click', 'click'], ['dblclick','click'], ['input', 'input'], ['change', 'input'], ['keydown', 'key'], ['keyup', 'key'], ['compositionstart', 'composition'], ['compositionupdate', 'composition'], ['compositionend', 'composition'], ['pointerdown', 'pointer'], ['pointerup', 'pointer'], ['pointermove', 'pointer'], ['wheel','wheel'], ['scroll', 'scroll'], ['focusin', 'focus']] as const) {
    document.addEventListener(name, event => action(kind, event), true)
    document.addEventListener(name, () => { actionScope=undefined })
  }
  for(const name of ['drop','dragstart','pointercancel'])document.addEventListener(name,event=>{if(active && event.target instanceof Element && !panel.contains(event.target)){host.__workflowState('incomplete');send({control:'failure',reason:'Unsupported drag, drop or pointer cancellation.'})}},true)
  host.__workflowIpc = command => {
    if (!active) return undefined
    const owner=actionScope ?? `unscoped-${performance.timeOrigin}`
    const key=`${owner}:${command}`
    const occurrence=(commandCounts.get(key)??0)+1;commandCounts.set(key,occurrence)
    const trace = `${owner}:ipc:${command}:${occurrence}`
    const mark = (phase: string) => send({ kind: 'timing', trace, span: command, phase, clock: 'ui', micros: Math.round(performance.now() * 1000) })
    mark('start')
    pendingIpc++
    return () => { mark('end');pendingIpc-- }
  }
  return { installed: true }
}





