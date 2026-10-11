import { lstat, mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { SessionCapture, type Bundle } from './session.ts'

type Entry = { path: string; digest: string }
const database = /^skellyspeak\.sqlite3(?:-wal|-shm|-journal)?$/
const audio = /^drill-audio\/[a-zA-Z0-9_.-]{1,200}$/
const allowed = (path: string) => database.test(path) || (audio.test(path) && !path.endsWith('/.') && !path.endsWith('/..'))

/** Consume only the native ownership-bound export, never a live database path. */
export async function captureWorkspace(directory: string, capture: SessionCapture): Promise<void> {
  const files: Entry[] = []
  const take = async (path: string) => {
    if (!allowed(path)) throw new Error('Unsupported workspace export member.')
    const source = join(directory, ...path.split('/'))
    const metadata = await lstat(source)
    if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size > 128 * 1024 * 1024) throw new Error('Workspace export member is invalid or too large.')
    files.push({ path, digest: capture.addAsset('workspace', await readFile(source)) })
  }
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isFile() && database.test(entry.name)) await take(entry.name)
    else if (entry.isDirectory() && entry.name === 'drill-audio') {
      for (const audio of await readdir(join(directory, entry.name))) await take(`drill-audio/${audio}`)
    } else throw new Error('Unexpected workspace export member.')
  }
  if (!files.some(file => file.path === 'skellyspeak.sqlite3')) throw new Error('Starting database is missing.')
  files.sort((a, b) => a.path.localeCompare(b.path))
  capture.setWorkspace(Buffer.from(JSON.stringify({ format: 1, files })))
}

/** Restores a validated bundle into a new directory only. Native startup still
 * validates workspace identity, integrity, migrations and product state. */
export async function restoreWorkspace(bundle: Bundle, assets: ReadonlyMap<string, Uint8Array>, directory: string): Promise<void> {
  const bytes = assets.get(bundle.workspace)
  if (!bytes || bytes.byteLength > 1024 * 1024) throw new Error('Missing or oversized workspace manifest.')
  const manifest: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes))
  if (!manifest || typeof manifest !== 'object' || !('format' in manifest) || manifest.format !== 1 || !('files' in manifest) || !Array.isArray(manifest.files) || manifest.files.length > 10000) throw new Error('Invalid workspace manifest.')
  const seen = new Set<string>()
  const files: { path: string; bytes: Uint8Array }[] = []
  for (const value of manifest.files as unknown[]) {
    if (!value || typeof value !== 'object' || !('path' in value) || typeof value.path !== 'string' || !allowed(value.path) || seen.has(value.path) || !('digest' in value) || typeof value.digest !== 'string') throw new Error('Invalid workspace member.')
    const bytes = assets.get(value.digest)
    if (!bytes || !bundle.assets.some(asset => asset.digest === value.digest && asset.roles.includes('workspace'))) throw new Error('Missing workspace member.')
    seen.add(value.path); files.push({ path: value.path, bytes })
  }
  if (!seen.has('skellyspeak.sqlite3')) throw new Error('Starting database is missing.')
  await mkdir(directory, { recursive: false, mode: 0o700 })
  if (files.some(file => file.path.startsWith('drill-audio/'))) await mkdir(join(directory, 'drill-audio'), { mode: 0o700 })
  for (const file of files) await writeFile(join(directory, ...file.path.split('/')), file.bytes, { flag: 'wx', mode: 0o600 })
}
