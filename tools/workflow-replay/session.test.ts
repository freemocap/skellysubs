import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { SessionCapture, digest, validateBundle, type TimedEvent } from './session.ts'
import { ProviderTape } from './provider-tape.ts'
import { discoverDesktop, loopbackEndpoint } from './desktop-preflight.ts'

const workspace = Buffer.from('synthetic workspace fixture, not a database')
function capture(maxBytes = 10000, maxEvents = 100): SessionCapture {
  const result = new SessionCapture({ contentConsent: true, build: 'synthetic-build', maxBytes, maxEvents })
  result.setWorkspace(workspace)
  return result
}
const fingerprint = digest(Buffer.from('credential-free synthetic request'))
function provider(request: string, phase: 'request' | 'chunk' | 'success' | 'failure' | 'cancelled', elapsedMicros: number): TimedEvent {
  return { kind: 'provider', request, fingerprint, phase, body: phase === 'chunk' ? '你好 مرحبا é' : '', sequence: 0, elapsedMicros }
}

test('requires explicit consent and rejects a missing starting workspace', () => {
  assert.throws(() => new SessionCapture({ contentConsent: false, build: 'test', maxBytes: 1, maxEvents: 1 }), /consent/)
  const session = new SessionCapture({ contentConsent: true, build: 'test', maxBytes: 1, maxEvents: 1 })
  assert.throws(() => session.stop(), /workspace/)
  assert.equal(session.snapshot().status, 'incomplete')
})

test('preserves Unicode and exports assets without overwriting a recording', async () => {
  const root = await mkdtemp(join(tmpdir(), 'skelly-replay-test-'))
  try {
    const session = capture()
    const input = { kind: 'action' as const, target: 'composer', action: 'input' as const, value: 'é 你好 مرحبا' }
    session.append(input, 4)
    input.value = 'changed after capture'
    session.stop()
    const directory = join(root, 'recording')
    await session.export(directory)
    const saved = JSON.parse(await readFile(join(directory, 'session.json'), 'utf8'))
    assert.equal(saved.events[0].value, 'é 你好 مرحبا')
    assert.deepEqual(await readFile(join(directory, 'assets', saved.workspace)), workspace)
    await assert.rejects(session.export(directory), /EEXIST/)
  } finally { await rm(root, { recursive: true, force: true }) }
})

test('overload stops capture visibly, retains evidence, and cannot be completed', () => {
  const session = capture(10000, 1)
  session.append({ kind: 'display', target: 'reply', text: 'one' }, 1)
  assert.throws(() => session.append({ kind: 'display', target: 'reply', text: 'two' }, 2), /limit/)
  session.stop()
  assert.equal(session.snapshot().status, 'incomplete')
  assert.equal(session.snapshot().events.length, 1)
  assert.throws(() => session.append({ kind: 'display', target: 'reply', text: 'three' }, 3), /stopped/)
  const bytes = capture(workspace.byteLength)
  assert.throws(() => bytes.addAsset('audio-input', Buffer.from('audio')), /limit/)
})

test('rejects missing audio, clock reversal, and corrupted asset contents', () => {
  const session = capture()
  session.append({ kind: 'audio', action: 'started', role: 'audio-output', asset: 'unknown' }, 2)
  assert.throws(() => session.append({ kind: 'display', target: 'reply', text: '' }, 1), /monotonic/)
  assert.throws(() => session.stop(), /audio/)
  const good = capture()
  good.stop()
  assert.throws(() => validateBundle(good.snapshot(), new Map([[digest(workspace), Buffer.from('wrong')]])), /damaged/)
})

test('replays independent provider requests concurrently and retains first-response delay', async () => {
  const tape = new ProviderTape([provider('a', 'request', 10), provider('b', 'request', 12), provider('b', 'success', 15), provider('a', 'chunk', 20), provider('a', 'success', 23)])
  let release!: () => void
  const blocked = new Promise<void>(resolve => { release = resolve })
  const a: string[] = []
  const delays: number[] = []
  const running = tape.replay('a', fingerprint, async event => { a.push(event.body); if (event.phase === 'chunk') await blocked }, async micros => { delays.push(micros) })
  const b: string[] = []
  await tape.replay('b', fingerprint, async event => { b.push(event.phase) })
  assert.deepEqual(b, ['success'])
  assert.throws(() => tape.assertConsumed(), /active/)
  release()
  await running
  assert.deepEqual(delays, [10, 3])
  assert.equal(a[0], '你好 مرحبا é')
  tape.assertConsumed()
  await assert.rejects(tape.replay('a', fingerprint, async () => {}), /matching/)
  assert.throws(() => tape.assertConsumed(), /failed/)
})

test('provider failures and cancellation are outcomes; mismatch/incomplete/delivery failures cannot pass', async () => {
  assert.throws(() => new ProviderTape([provider('a', 'request', 1)]), /terminal/)
  assert.throws(() => new ProviderTape([provider('a', 'success', 1)]), /sequence/)
  for (const phase of ['failure', 'cancelled'] as const) {
    const tape = new ProviderTape([provider('a', 'request', 1), provider('a', phase, 2)])
    const outcomes: string[] = []
    await tape.replay('a', fingerprint, async event => { outcomes.push(event.phase) })
    assert.deepEqual(outcomes, [phase])
    tape.assertConsumed()
  }
  const failed = new ProviderTape([provider('a', 'request', 1), provider('a', 'success', 2)])
  await assert.rejects(failed.replay('a', fingerprint, async () => { throw new Error('adapter failed') }), /adapter failed/)
  assert.throws(() => failed.assertConsumed(), /failed/)
})

test('a caught unmatched request permanently fails the replay session', async () => {
  for (const [request, hash] of [['unknown', fingerprint], ['a', 'wrong']]) {
    const tape = new ProviderTape([provider('a', 'request', 1), provider('a', 'success', 2)])
    await assert.rejects(tape.replay(request, hash, async () => {}), /matching/)
    let delivered = false
    await assert.rejects(tape.replay('a', fingerprint, async () => { delivered = true }), /failed/)
    assert.equal(delivered, false)
    assert.throws(() => tape.assertConsumed(), /failed/)
  }
})

test('identical audio bytes retain input/output roles without duplicated payload', () => {
  const audio = Buffer.from('synthetic audio')
  const session = capture(workspace.byteLength + audio.byteLength)
  const input = session.addAsset('audio-input', audio)
  assert.equal(session.addAsset('audio-output', audio), input)
  assert.equal(session.addAsset('audio-output', audio), input)
  session.stop()
  const bundle = session.snapshot()
  assert.deepEqual(bundle.assets.find(asset => asset.digest === input)?.roles, ['audio-input', 'audio-output'])
  assert.equal(bundle.assets.length, 2)
  const assets = new Map([[digest(workspace), workspace], [input, audio]])
  bundle.events.push({ kind: 'audio', action: 'captured', role: 'audio-input', asset: input, sequence: 0, elapsedMicros: 1 })
  bundle.events.push({ kind: 'audio', action: 'started', role: 'audio-output', asset: input, sequence: 1, elapsedMicros: 2 })
  validateBundle(bundle, assets)
  bundle.assets.find(asset => asset.digest === input)!.roles = ['audio-input']
  assert.throws(() => validateBundle(bundle, assets), /audio asset role/)
})

test('stopping during provider work marks the bundle incomplete', () => {
  const session = capture()
  session.append(provider('unfinished', 'request', 1), 1)
  assert.throws(() => session.stop(), /terminal/)
  assert.equal(session.snapshot().status, 'incomplete')
})

test('desktop discovery is explicit, loopback-only, bounded, and excludes target content', async () => {
  for (const endpoint of ['https://127.0.0.1:9222', 'http://example.com:9222', 'http://user:secret@127.0.0.1:9222', 'http://127.0.0.1:9222/path']) assert.throws(() => loopbackEndpoint(endpoint))
  const paths: string[] = []
  const fake = (async (input, options) => {
    const url = new URL(String(input)); paths.push(url.pathname)
    assert.equal(options?.redirect, 'error')
    return new Response(JSON.stringify(url.pathname === '/json/version' ? { Browser: 'Synthetic/1' } : [{ type: 'page', title: 'private text', url: 'private URL' }]))
  }) as typeof fetch
  assert.deepEqual(await discoverDesktop('http://127.0.0.1:9222', fake), { browser: 'Synthetic/1', pages: 1 })
  assert.deepEqual(paths, ['/json/version', '/json/list'])
  await assert.rejects(discoverDesktop('http://127.0.0.1:9222', (async () => new Response('x'.repeat(65537))) as typeof fetch), /size limit/)
})
