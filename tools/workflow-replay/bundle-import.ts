import { lstat, open, realpath } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { constants } from 'node:fs'
import { checkLimits, decodeManifest, defaultBundleLimits, type BundleLimits } from './bundle-decoder.ts'
import { validateBundle, type Bundle } from './session.ts'

/** Open only a regular file at the expected canonical path. Stat the opened handle
 * as well, so a path replacement between inspection/open cannot change identity. */
async function readBounded(path: string, maximum: number): Promise<Uint8Array> {
  const before = await lstat(path)
  if (!before.isFile() || before.isSymbolicLink() || await realpath(path) !== path) throw new Error('Recording files must be regular files within the recording directory.')
  if (before.size > maximum) throw new Error('Recording file exceeds import limit.')
  const handle = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0))
  try {
    const opened = await handle.stat()
    if (!opened.isFile() || opened.dev !== before.dev || opened.ino !== before.ino || opened.size !== before.size) throw new Error('Recording changed while opening.')
    const bytes = Buffer.alloc(opened.size + 1)
    let offset = 0
    while (offset < bytes.byteLength) {
      const result = await handle.read(bytes, offset, bytes.byteLength - offset, offset)
      if (!result.bytesRead) break
      offset += result.bytesRead
    }
    if (offset !== opened.size) throw new Error('Recording changed while reading.')
    return bytes.subarray(0, offset)
  } finally { await handle.close() }
}

export type ImportedBundle = { bundle: Bundle; assets: ReadonlyMap<string, Uint8Array> }

/** Import data only: never execute recordings, restore workspaces or contact a
 * provider. Callers own explicit build/settings acceptance before any replay. */
export async function loadBundle(directory: string, limits: BundleLimits = defaultBundleLimits): Promise<ImportedBundle> {
  checkLimits(limits)
  const requested = resolve(directory)
  const metadata = await lstat(requested)
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw new Error('Recording directory must not be a link.')
  const root = await realpath(requested)
  const assetDirectory = join(root, 'assets')
  const assetMetadata = await lstat(assetDirectory)
  if (!assetMetadata.isDirectory() || assetMetadata.isSymbolicLink() || await realpath(assetDirectory) !== assetDirectory) throw new Error('Recording asset directory must not be a link.')
  const bundle = decodeManifest(await readBounded(join(root, 'session.json'), limits.maxManifestBytes), limits)
  const assets = new Map<string, Uint8Array>()
  for (const asset of bundle.assets) {
    // decodeManifest accepts only lowercase 64-character SHA-256 names.
    if (assets.has(asset.digest)) throw new Error('Duplicate recording asset identity.')
    const bytes = await readBounded(join(assetDirectory, asset.digest), Math.min(asset.bytes, limits.maxAssetBytes))
    if (bytes.byteLength !== asset.bytes) throw new Error('Recording asset length mismatch.')
    assets.set(asset.digest, bytes)
  }
  validateBundle(bundle, assets)
  return { bundle, assets }
}
