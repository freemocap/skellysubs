/** Real native IPC: identical concurrent calls, capture export, and offline replay. */
import assert from 'node:assert/strict'
import { createServer, type ServerResponse } from 'node:http'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { launchDesktop } from './desktop-launch.ts'
import { workflowControl } from './control.ts'
import { SessionCapture } from './session.ts'
import { captureWorkspace } from './workspace-bundle.ts'
import { loadBundle } from './bundle-import.ts'

const pending: ServerResponse[] = []
let requests = 0
const server = createServer((request, response) => {
  assert.equal(request.headers.authorization, undefined)
  assert.equal(request.url, '/v1/protocol?verify_providers=true')
  requests++
  pending.push(response)
  setTimeout(() => {
    if (!response.writableEnded) { response.writeHead(503); response.end('{}') }
  }, 2000).unref()
  // Neither request can finish until both are genuinely in flight.
  if (pending.length !== 2) return
  for (const reply of pending) {
    reply.setHeader('content-type', 'application/json')
    reply.end(JSON.stringify({ protocol: 'skellyspeak', version: 1, operations_versions: [3], max_items: 8,
      audio: { version: 3 }, providers: ['OPENROUTER', 'GROQ'].map(provider => ({ provider, state: 'accepted', status: 200, durationMs: 0 })) }))
  }
})
await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
const address = server.address()
if (!address || typeof address === 'string') throw Error('Fixture did not bind.')
const session = await launchDesktop()
const checks = `(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke;const access=await invoke('get_access_settings');const check=()=>invoke('check_access',{expectedRevision:access.revision}).then(value=>({ok:true,value}),()=>({ok:false}));return Promise.all([check(),check()])})()`
let results: unknown
try {
  await session.connection.evaluate(`(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke;const access=await invoke('get_access_settings');await invoke('save_access_settings',{expectedRevision:access.revision,custom:{baseUrl:'http://127.0.0.1:${address.port}/v1',bearerAuth:false},sessionToken:null,removeToken:false})})()`)
  const start = await workflowControl<{ workspace: string }>(session.connection, 'start')
  const capture = new SessionCapture({ contentConsent: true, build: session.build, maxBytes: 128 * 1024 * 1024, maxEvents: 10 })
  await captureWorkspace(start.workspace, capture)
  capture.append({ kind: 'display', target: 'initial-browser-state', text: JSON.stringify({ storage: {}, width: 1000, height: 800, scale: 1 }) }, 0)
  results = await session.connection.evaluate(checks)
  assert.ok(Array.isArray(results) && results.every(result => result.ok), 'Both identical concurrent native calls must succeed')
  assert.equal(requests, 2)
  const stop = await workflowControl<{ providerTape: string }>(session.connection, 'stop')
  const bytes = await readFile(stop.providerTape)
  const tape = JSON.parse(bytes.toString()) as { records: { fingerprint: string; complete: boolean }[] }
  assert.equal(tape.records.length, 2)
  assert.equal(tape.records[0].fingerprint, tape.records[1].fingerprint)
  assert.ok(tape.records.every(record => record.complete))
  capture.setProviderTape(bytes)
  capture.stop()
  await capture.export(join(session.directory, 'recording'))
} finally {
  await session.close()
  server.closeAllConnections()
  await new Promise<void>(resolve => server.close(() => resolve()))
}
const replay = await launchDesktop(undefined, await loadBundle(join(session.directory, 'recording')), { mute: true })
try {
  // Native replay starts during isolated application initialization.
  assert.deepEqual(await replay.connection.evaluate(checks), results)
  await workflowControl(replay.connection, 'stop')
  replay.verifyNetwork()
  const proof = { capture: session.directory, replay: replay.directory, concurrentIdenticalCalls: 2, completed: true, liveProvider: false }
  await writeFile(join(replay.directory, 'overlap-result.json'), JSON.stringify(proof))
  console.log(JSON.stringify(proof))
} finally { await replay.close() }
