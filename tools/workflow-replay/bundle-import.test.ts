import assert from 'node:assert/strict'
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { SessionCapture, type Bundle, type TimedEvent } from './session.ts'
import { decodeManifest, defaultBundleLimits } from './bundle-decoder.ts'
import { loadBundle } from './bundle-import.ts'
import { compareLatency, latencyReport } from './latency-report.ts'

function capture(): SessionCapture {
  const session = new SessionCapture({ contentConsent: true, build: 'fixture', maxBytes: 10000, maxEvents: 100 })
  session.setWorkspace(Buffer.from('synthetic workspace'))
  session.append({ kind: 'action', target: 'composer', action: 'input', value: 'é مرحبا 你好' }, 1)
  session.stop()
  return session
}
const encode = (value: unknown): Buffer => Buffer.from(JSON.stringify(value))
async function temporary(run: (directory: string) => Promise<void>) {
  const directory = await mkdtemp(join(tmpdir(), 'skelly-bundle-test-'))
  try { await run(directory) } finally { await rm(directory, { recursive: true, force: true }) }
}

test('strict decoder preserves source content and rejects unknown shape, unsafe integers and invalid UTF-8', () => {
  const bundle = capture().snapshot()
  assert.deepEqual(decodeManifest(encode(bundle)), JSON.parse(encode(bundle).toString()))
  const changes = [
    { ...bundle, credentials: 'excluded' }, { ...bundle, format: 2 }, { ...bundle, status: 'incomplete' },
    { ...bundle, events: [{ ...bundle.events[0], action: 'execute-script' }] },
    { ...bundle, events: [{ ...bundle.events[0], sequence: Number.MAX_SAFE_INTEGER + 1 }] },
    { ...bundle, assets: [{ ...bundle.assets[0], digest: '../outside' }] },
    { ...bundle, assets: [{ ...bundle.assets[0], roles: ['workspace', 'workspace'] }] },
    { ...bundle, events: [{ ...bundle.events[0], value: null }] },
  ]
  for (const value of changes) assert.throws(() => decodeManifest(encode(value)))
  assert.throws(() => decodeManifest(Buffer.from([0xff])), /UTF-8/)
  assert.throws(() => decodeManifest(encode(bundle), { ...defaultBundleLimits, maxManifestBytes: 1 }), /limits/)
  assert.throws(() => decodeManifest(encode(bundle), { ...defaultBundleLimits, maxTotalAssetBytes: 1 }), /limits/)
})

test('imports an exported bundle and rejects corrupted or missing assets', async () => temporary(async root => {
  const path = join(root, 'session')
  await capture().export(path)
  const loaded = await loadBundle(path)
  assert.equal(loaded.bundle.events[0].kind, 'action')
  assert.equal(Buffer.from(loaded.assets.get(loaded.bundle.workspace)!).toString(), 'synthetic workspace')
  const assetPath = join(path, 'assets', loaded.bundle.workspace)
  await writeFile(assetPath, 'synthetic workspacE')
  await assert.rejects(loadBundle(path), /damaged/)
  await rm(assetPath)
  await assert.rejects(loadBundle(path), /ENOENT/)
}))

test('provider tape reference requires the exact asset role and remains distinct from event-only capture', async () => temporary(async root => {
  const session = new SessionCapture({ contentConsent: true, build: 'fixture', maxBytes: 10000, maxEvents: 10 })
  session.setWorkspace(Buffer.from('synthetic workspace'))
  session.setProviderTape(Buffer.from('{"format":1,"records":[]}'))
  session.stop()
  const path = join(root, 'session')
  await session.export(path)
  const loaded = await loadBundle(path)
  assert.ok(loaded.bundle.providerTape)
  assert.ok(loaded.assets.has(loaded.bundle.providerTape))
  const bundle = loaded.bundle
  bundle.assets.find(asset => asset.digest === bundle.providerTape)!.roles = ['workspace']
  await writeFile(join(path, 'session.json'), encode(bundle))
  await assert.rejects(loadBundle(path), /provider tape asset role/)
}))

test('rejects directory junctions and manifest path traversal before asset reads', async () => temporary(async root => {
  const path = join(root, 'session')
  await capture().export(path)
  const outside = join(root, 'outside')
  await mkdir(outside)
  const assets = join(path, 'assets')
  await rm(assets, { recursive: true })
  await symlink(outside, assets, process.platform === 'win32' ? 'junction' : 'dir')
  await assert.rejects(loadBundle(path), /link/)
  await rm(assets)
  await mkdir(assets)
  const bundle = JSON.parse(await readFile(join(path, 'session.json'), 'utf8')) as Bundle
  bundle.assets[0].digest = '../../outside'
  await writeFile(join(path, 'session.json'), encode(bundle))
  await assert.rejects(loadBundle(path), /shape/)
}))

function timings(values: { trace: string; clock?: 'ui' | 'native'; start?: number; end?: number }[]): Bundle {
  const bundle = capture().snapshot()
  for (const value of values) {
    for (const phase of ['start', 'end'] as const) {
      if (value[phase] === undefined) continue
      bundle.events.push({ kind: 'timing', trace: value.trace, span: 'open-help', clock: value.clock ?? 'ui', phase, micros: value[phase], sequence: bundle.events.length, elapsedMicros: bundle.events.length + 1 } as TimedEvent)
    }
  }
  return bundle
}

test('latency pairs only same-clock spans and exposes incomplete measurements', () => {
  const report = latencyReport(timings([{ trace: 'one', start: 100, end: 200 }, { trace: 'two', start: 300 }, { trace: 'two', clock: 'native', end: 2 }]))
  assert.equal(report.samples[0].durationMicros, 100)
  assert.deepEqual(report.missing.map(value => [value.clock, value.missing]), [['ui', 'end'], ['native', 'start']])
  assert.throws(() => latencyReport(timings([{ trace: 'one', start: 5, end: 4 }])), /before/)
  assert.throws(() => latencyReport(timings([{ trace: 'one', start: 5 }, { trace: 'one', start: 6 }])), /Duplicate/)
})

test('comparison reports paired distributions, missing observations and mismatched workloads', () => {
  const baseline = timings([{ trace: 'a', start: 0, end: 100 }, { trace: 'b', start: 10, end: 310 }, { trace: 'c', start: 0, end: 500 }])
  const candidate = timings([{ trace: 'a', start: 1000, end: 1050 }, { trace: 'b', start: 20, end: 220 }, { trace: 'd', start: 0, end: 8 }])
  candidate.build = 'new-build'
  const report = compareLatency(baseline, candidate)
  assert.deepEqual(report.pairs.map(pair => pair.deltaMicros), [-50, -100])
  assert.deepEqual(report.summaries[0].baseline, { count: 2, p50Micros: 100, p95Micros: 300, maxMicros: 300 })
  assert.deepEqual(report.unmatched.map(item => item.side), ['baseline', 'candidate'])
  candidate.workspace = 'f'.repeat(64)
  assert.throws(() => compareLatency(baseline, candidate), /workloads/)
})
