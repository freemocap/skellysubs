import { createHash } from 'node:crypto'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { ProviderTape } from './provider-tape.ts'

// This tooling protocol records observations; graph semantics remain in native code.
export type AssetKind = 'workspace' | 'audio-input' | 'audio-output' | 'screenshot' | 'provider-tape'
export type Asset = { digest: string; bytes: number; roles: AssetKind[] }
export type Event =
  | { kind: 'action'; target: string; action: 'click' | 'input' | 'composition' | 'key' | 'pointer' | 'wheel' | 'scroll' | 'focus'; value: string }
  | { kind: 'display'; target: string; text: string }
  | { kind: 'audio'; action: 'captured' | 'requested' | 'started' | 'ended' | 'cancelled'; role: 'audio-input' | 'audio-output'; asset: string }
  | { kind: 'timing'; trace: string; span: string; phase: 'start' | 'end'; clock: 'ui' | 'native' | 'runner'; micros: number }
  | { kind: 'provider'; request: string; fingerprint: string; phase: 'request' | 'chunk' | 'success' | 'failure' | 'cancelled'; body: string }
export type TimedEvent = Event & { sequence: number; elapsedMicros: number }
export type Bundle = {
  format: 1
  build: string
  workspace: string
  providerTape?: string
  status: 'complete' | 'incomplete'
  reason?: string
  assets: Asset[]
  events: TimedEvent[]
}
export const digest = (bytes: Uint8Array): string => createHash('sha256').update(bytes).digest('hex')
const hashPattern = /^[a-f0-9]{64}$/
const nonnegative = (value: number): boolean => Number.isSafeInteger(value) && value >= 0

/** Capture content only after an explicit user opt-in. Never supply credentials,
 * transport headers, access settings, or arbitrary native diagnostic objects.
 * Text and media are intentionally sensitive; this is not a redaction service.
 */
export class SessionCapture {
  private readonly bundle: Bundle
  private readonly blobs = new Map<string, Uint8Array>()
  private usedBytes = 0
  private stopped = false
  private lastTime = 0
  private readonly maxBytes: number
  private readonly maxEvents: number

  constructor(options: { contentConsent: boolean; build: string; maxBytes: number; maxEvents: number }) {
    if (!options.contentConsent) throw new Error('Explicit content recording consent is required.')
    if (!options.build || !nonnegative(options.maxBytes) || !nonnegative(options.maxEvents)) throw new Error('Invalid capture settings.')
    this.maxBytes = options.maxBytes
    this.maxEvents = options.maxEvents
    this.bundle = { format: 1, build: options.build, workspace: '', status: 'incomplete', assets: [], events: [] }
  }

  private requireOpen(): void {
    if (this.stopped) throw new Error('Capture is stopped.')
  }

  private reserve(bytes: number): void {
    if (this.usedBytes + bytes > this.maxBytes) {
      this.stop('Capture byte limit exceeded.')
      throw new Error(this.bundle.reason)
    }
    this.usedBytes += bytes
  }

  addAsset(kind: AssetKind, bytes: Uint8Array): string {
    this.requireOpen()
    const key = digest(bytes)
    const existing = this.bundle.assets.find(asset => asset.digest === key)
    if (existing) {
      if (!existing.roles.includes(kind)) existing.roles.push(kind)
    } else {
      this.reserve(bytes.byteLength)
      this.blobs.set(key, Uint8Array.from(bytes))
      this.bundle.assets.push({ digest: key, bytes: bytes.byteLength, roles: [kind] })
    }
    return key
  }

  /** Bytes must come from a consistent native workspace export, with credentials
   * excluded. Copying a live SQLite file is not a supported capture mechanism. */
  setWorkspace(bytes: Uint8Array): void {
    this.requireOpen()
    if (this.bundle.workspace) throw new Error('Starting workspace already supplied.')
    this.bundle.workspace = this.addAsset('workspace', bytes)
  }

  setProviderTape(bytes: Uint8Array): void {
    this.requireOpen()
    if (this.bundle.providerTape) throw new Error('Provider tape already supplied.')
    this.bundle.providerTape = this.addAsset('provider-tape', bytes)
  }

  append(event: Event, elapsedMicros: number): void {
    this.requireOpen()
    if (!nonnegative(elapsedMicros) || elapsedMicros < this.lastTime) throw new Error('Runner time must be monotonic.')
    if (this.bundle.events.length >= this.maxEvents) {
      this.stop('Capture event limit exceeded.')
      throw new Error(this.bundle.reason)
    }
    const item = { ...event, sequence: this.bundle.events.length, elapsedMicros }
    // Clone here: changing a caller-owned event must not change retained evidence.
    const encoded = JSON.stringify(item)
    this.reserve(Buffer.byteLength(encoded))
    this.bundle.events.push(JSON.parse(encoded) as TimedEvent)
    this.lastTime = elapsedMicros
  }

  stop(reason?: string): void {
    if (this.stopped) return
    this.stopped = true
    this.bundle.status = reason ? 'incomplete' : 'complete'
    this.bundle.reason = reason
    if (!reason) {
      try { validateBundle(this.bundle, this.blobs) }
      catch (error) {
        this.bundle.status = 'incomplete'
        this.bundle.reason = error instanceof Error ? error.message : 'Invalid recording.'
        throw error
      }
    }
  }

  snapshot(): Bundle { return structuredClone(this.bundle) }

  /** Explicit export only. A new directory is required; an interrupted write has
   * no complete manifest. Native integration must stream larger recordings. */
  async export(directory: string, incompleteReason?: string): Promise<void> {
    if (!this.stopped) throw new Error('Stop capture before export.')
    await mkdir(directory, { recursive: false, mode: 0o700 })
    await mkdir(join(directory, 'assets'), { mode: 0o700 })
    for (const [key, bytes] of this.blobs) await writeFile(join(directory, 'assets', key), bytes, { flag: 'wx', mode: 0o600 })
    const manifest = incompleteReason ? { ...this.bundle, status: 'incomplete', reason: incompleteReason } : this.bundle
    await writeFile(join(directory, 'session.json'), JSON.stringify(manifest), { flag: 'wx', mode: 0o600 })
  }
}

/** Validate a typed in-memory bundle before replay. A disk decoder must separately
 * validate unknown JSON; callers must not cast arbitrary JSON into this type. */
export function validateBundle(bundle: Bundle, assets: ReadonlyMap<string, Uint8Array>): void {
  if (bundle.format !== 1 || !bundle.build || bundle.status !== 'complete') throw new Error('Recording is incomplete or unsupported.')
  const indexed = new Map<string, Asset>()
  for (const asset of bundle.assets) {
    if (!hashPattern.test(asset.digest) || indexed.has(asset.digest)) throw new Error('Invalid or duplicate asset identity.')
    if (!asset.roles.length || new Set(asset.roles).size !== asset.roles.length || asset.roles.some(role => !['workspace', 'audio-input', 'audio-output', 'screenshot', 'provider-tape'].includes(role))) throw new Error('Invalid asset roles.')
    const bytes = assets.get(asset.digest)
    if (!bytes || bytes.byteLength !== asset.bytes || digest(bytes) !== asset.digest) throw new Error('Missing or damaged asset.')
    indexed.set(asset.digest, asset)
  }
  if (!indexed.get(bundle.workspace)?.roles.includes('workspace')) throw new Error('Consistent starting workspace is required.')
  if (bundle.providerTape && !indexed.get(bundle.providerTape)?.roles.includes('provider-tape')) throw new Error('Missing provider tape asset role.')
  let time = 0
  for (const [sequence, event] of bundle.events.entries()) {
    if (event.sequence !== sequence || !nonnegative(event.elapsedMicros) || event.elapsedMicros < time) throw new Error('Invalid event ordering.')
    time = event.elapsedMicros
    if (event.kind === 'audio' && !indexed.get(event.asset)?.roles.includes(event.role)) throw new Error('Missing audio asset role.')
    if (event.kind === 'timing' && !nonnegative(event.micros)) throw new Error('Invalid clock sample.')
  }
  // A stopped recorder must not label an unfinished provider stream complete.
  new ProviderTape(bundle.events)
}
