import assert from 'node:assert/strict'
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import test from 'node:test'
import { SessionCapture } from './session.ts'
import { captureWorkspace, restoreWorkspace } from './workspace-bundle.ts'
import { loadBundle } from './bundle-import.ts'

async function temporary(run: (root: string) => Promise<void>) {
  const root = await mkdtemp(join(tmpdir(), 'workflow-workspace-'))
  try { await run(root) } finally { await rm(root, { recursive: true, force: true }) }
}
const session = () => new SessionCapture({ contentConsent: true, build: 'synthetic', maxBytes: 100000, maxEvents: 100 })
test('workspace bundle preserves database and audio bytes and refuses an existing destination', async () => temporary(async root => {
  const source = join(root, 'source')
  await mkdir(join(source, 'drill-audio'), { recursive: true })
  const audio = Buffer.from([0, 255, 127, 80])
  await writeFile(join(source, 'skellyspeak.sqlite3'), 'synthetic database bytes')
  await writeFile(join(source, 'drill-audio', 'fixture.wav'), audio)
  const capture = session()
  await captureWorkspace(source, capture)
  capture.stop()
  const exported = join(root, 'bundle')
  await capture.export(exported)
  const { bundle, assets } = await loadBundle(exported)
  const restored = join(root, 'restored')
  await restoreWorkspace(bundle, assets, restored)
  assert.deepEqual(await readFile(join(restored, 'drill-audio', 'fixture.wav')), audio)
  assert.equal(await readFile(join(restored, 'skellyspeak.sqlite3'), 'utf8'), 'synthetic database bytes')
  await assert.rejects(restoreWorkspace(bundle, assets, restored))
}))
test('workspace capture rejects unexpected members and missing database', async () => temporary(async root => {
  await assert.rejects(captureWorkspace(root, session()))
  await writeFile(join(root, 'credentials.json'), 'synthetic')
  await assert.rejects(captureWorkspace(root, session()))
}))
test('workspace restore rejects traversal, duplicate and missing asset roles before creating destination', async () => temporary(async root => {
  const capture = session()
  capture.setWorkspace(Buffer.from('placeholder'))
  capture.stop()
  const bundle = capture.snapshot()
  for (const files of [[{ path: '../escape', digest: 'a' }], [{ path: 'skellyspeak.sqlite3', digest: 'a' }], []]) {
    const assets = new Map([[bundle.workspace, Buffer.from(JSON.stringify({ format: 1, files }))], ['a', Buffer.from('bytes')]])
    await assert.rejects(restoreWorkspace(bundle, assets, join(root, 'destination')))
  }
  const member = { path: 'skellyspeak.sqlite3', digest: bundle.workspace }
  const duplicate = new Map([[bundle.workspace, Buffer.from(JSON.stringify({ format: 1, files: [member, member] }))]])
  await assert.rejects(restoreWorkspace(bundle, duplicate, join(root, 'destination')))
}))
