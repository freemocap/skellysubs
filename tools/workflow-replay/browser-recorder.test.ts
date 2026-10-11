import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import test from 'node:test'
import { installBrowserRecorder } from './browser-recorder.ts'

test('named control targets survive icon and wrapper changes without choosing ambiguous controls', () => {
  const dom = new JSDOM('<div id="root"><div><button aria-label="Record audio"><svg></svg></button></div></div>', { runScripts: 'outside-only', pretendToBeVisual: true, url: 'http://localhost' })
  const window = dom.window
  const events: {event:{kind?:string;target?:string}}[] = []
  try {
    window.__workflowSend = (value:string) => events.push(JSON.parse(value))
    window.CSS ??= {}; window.CSS.escape = (value:string) => value
    window.eval(`(${installBrowserRecorder.toString()})()`)
    window.__workflowState('recording')
    const button = window.document.querySelector('button[aria-label]')
    button.querySelector('svg').dispatchEvent(new window.MouseEvent('click', {bubbles:true}))
    const target = events.find(item=>item.event.kind==='action')!.event.target!
    assert.equal(window.document.querySelector(target), button)
    const wrapper = window.document.createElement('section')
    window.document.getElementById('root').prepend(wrapper)
    wrapper.append(button)
    button.innerHTML='<span>Ready</span>'
    assert.equal(window.document.querySelector(target), button)
    const duplicate=button.cloneNode(true)
    wrapper.append(duplicate)
    events.length=0
    button.querySelector('span').dispatchEvent(new window.MouseEvent('click',{bubbles:true}))
    const precise=events.find(item=>item.event.kind==='action')!.event.target!
    assert.equal(window.document.querySelectorAll(precise).length,1)
    assert.equal(window.document.querySelector(precise),button.querySelector('span'))
  } finally { window.close() }
})

// Reuse the UI's existing DOM test dependency without adding another runtime.
const { JSDOM } = createRequire(new URL('../../ui/package.json', import.meta.url))('jsdom')

test('replay-derived focus has an unpaired identity and cannot collide with a later explicit action', async () => {
  const dom = new JSDOM('<div id="root"><input id="field"></div>', { runScripts: 'outside-only', pretendToBeVisual: true, url: 'http://localhost' })
  const window = dom.window
  const events: { event: { kind?: string; value?: string } }[] = []
  try {
    window.__workflowSend = (value: string) => events.push(JSON.parse(value))
    window.CSS ??= {}; window.CSS.escape = (value: string) => value
    window.eval(`(${installBrowserRecorder.toString()})()`)
    window.__workflowState('recording')
    const field = window.document.getElementById('field')
    window.__workflowReplayAction('action-1')
    field.dispatchEvent(new window.KeyboardEvent('keydown', { bubbles: true, key: 'Tab', code: 'Tab', keyCode: 9 }))
    field.dispatchEvent(new window.FocusEvent('focusin', { bubbles: true }))
    window.__workflowReplayAction('action-2')
    field.dispatchEvent(new window.InputEvent('input', { bubbles: true, data: 'a' }))
    const ids = events.filter(value => value.event.kind === 'action').map(value => JSON.parse(value.event.value!).id as string)
    assert.equal(ids[0], 'action-1')
    assert.match(ids[1], /^derived-/)
    assert.equal(ids[2], 'action-2')
    await new Promise(resolve => setTimeout(resolve, 40))
    const timings = await window.__workflowTimings() as { trace: string; phase: string }[]
    assert.equal(new Set(timings.map(value => `${value.trace}:${value.phase}`)).size, timings.length)
  } finally { window.close() }
})

test('action ownership survives capture microtasks, expires after dispatch and rejects uncommanded replay input', async () => {
  const dom = new JSDOM('<div id="root"><input id="field"></div>', { runScripts: 'outside-only', pretendToBeVisual: true, url: 'http://localhost' })
  const window = dom.window
  const events: { event: { control?: string } }[] = []
  try {
    window.__workflowSend = (value: string) => events.push(JSON.parse(value))
    window.CSS ??= {}; window.CSS.escape = (value: string) => value
    window.eval(`(${installBrowserRecorder.toString()})()`)
    window.__workflowState('replaying')
    assert.equal(window.document.querySelectorAll('#workflow-recorder button')[1].disabled, true)
    const field = window.document.getElementById('field')
    // Model the capture-to-bubble microtask checkpoint and the stopped-bubble
    // fallback without relying on jsdom's different event-loop checkpoints.
    field.addEventListener('keydown', (event: Event) => event.stopPropagation())
    window.__workflowReplayAction('action-1')
    field.dispatchEvent(new window.KeyboardEvent('keydown', { bubbles: true, key: 'Enter', code: 'Enter', keyCode: 13 }))
    await Promise.resolve()
    window.__workflowIpc('execute_command')()
    await new Promise(resolve => setTimeout(resolve, 10))
    window.__workflowIpc('get_snapshot')()
    const timings = await window.__workflowTimings() as { trace: string; phase: string }[]
    assert.ok(timings.some(value => value.trace === 'action-1:ipc:execute_command:1'))
    assert.ok(timings.some(value => /^unscoped-.*:ipc:get_snapshot:1$/.test(value.trace)))
    field.dispatchEvent(new window.KeyboardEvent('keyup', { bubbles: true, key: 'Enter', code: 'Enter', keyCode: 13 }))
    assert.ok(events.some(value => value.event.control === 'failure'))
  } finally { window.close() }
})

test('derived focus before a commanded key cannot consume that key identity', async () => {
  const dom = new JSDOM('<div id="root"><input id="field"></div>', { runScripts: 'outside-only', pretendToBeVisual: true, url: 'http://localhost' })
  const window = dom.window
  const events: {event:{control?:string;kind?:string;value?:string}}[]=[]
  try {
    window.__workflowSend=(value:string)=>events.push(JSON.parse(value))
    window.CSS ??= {};window.CSS.escape=(value:string)=>value
    window.eval(`(${installBrowserRecorder.toString()})()`)
    window.__workflowState('replaying')
    const field=window.document.getElementById('field')
    window.__workflowReplayAction('action-1','keydown')
    field.dispatchEvent(new window.FocusEvent('focusin',{bubbles:true}))
    field.dispatchEvent(new window.KeyboardEvent('keydown',{bubbles:true,key:'Tab',code:'Tab',keyCode:9}))
    assert.ok(!events.some(e=>e.event.control==='failure'))
    const actions=events.filter(e=>e.event.kind==='action').map(e=>JSON.parse(e.event.value!))
    assert.match(actions[0].id,/^derived-/)
    assert.equal(actions[1].id,'action-1')
  } finally { window.close() }
})

test('hardware acknowledgment remains pending through derived events until the expected event arrives', async () => {
  const dom=new JSDOM('<div id="root"><input id="field"></div>',{runScripts:'outside-only',pretendToBeVisual:true,url:'http://localhost'})
  const window=dom.window
  try {
    window.__workflowSend=()=>{}
    window.CSS ??= {};window.CSS.escape=(value:string)=>value
    window.eval(`(${installBrowserRecorder.toString()})()`)
    window.__workflowState('replaying')
    const field=window.document.getElementById('field')
    window.__workflowReplayAction('action-1','wheel')
    await new Promise(resolve=>setTimeout(resolve,10))
    field.dispatchEvent(new window.FocusEvent('focusin',{bubbles:true}))
    assert.equal(window.__workflowReplayPending(),'action-1')
    field.dispatchEvent(new window.WheelEvent('wheel',{bubbles:true,deltaY:100,deltaMode:0}))
    assert.equal(window.__workflowReplayPending(),undefined)
  } finally {window.close()}
})

test('an unrelated wheel event cannot consume an armed action with different coordinates',()=>{
  const dom=new JSDOM('<div id="root"><div id="field"></div></div>',{runScripts:'outside-only',pretendToBeVisual:true,url:'http://localhost'})
  const window=dom.window
  const events:{event:{control?:string}}[]=[]
  try {
    window.__workflowSend=(value:string)=>events.push(JSON.parse(value))
    window.CSS ??= {};window.CSS.escape=(value:string)=>value
    window.eval(`(${installBrowserRecorder.toString()})()`)
    window.__workflowState('replaying')
    window.__workflowReplayAction('action-1','wheel',{x:10,y:20,dx:0,dy:100})
    window.document.getElementById('field').dispatchEvent(new window.WheelEvent('wheel',{bubbles:true,clientX:99,clientY:20,deltaY:100}))
    assert.equal(window.__workflowReplayPending(),'action-1')
    assert.ok(events.some(e=>e.event.control==='failure'))
  }finally{window.close()}
})
