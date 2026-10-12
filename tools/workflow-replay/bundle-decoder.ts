import type { Asset, AssetKind, Bundle, TimedEvent } from './session.ts'

export type BundleLimits = {
  maxManifestBytes: number
  maxAssetBytes: number
  maxTotalAssetBytes: number
  maxEvents: number
  maxAssets: number
}
export const defaultBundleLimits: Readonly<BundleLimits> = Object.freeze({
  maxManifestBytes: 16 * 1024 * 1024,
  maxAssetBytes: 128 * 1024 * 1024,
  maxTotalAssetBytes: 512 * 1024 * 1024,
  maxEvents: 100_000,
  maxAssets: 10_000,
})
type ObjectValue = Record<string, unknown>
const hashes = /^[a-f0-9]{64}$/
const invalid = (): never => { throw new Error('Invalid recording manifest shape.') }
const bounded = (): never => { throw new Error('Recording exceeds import limits.') }
export function checkLimits(limits: BundleLimits): void {
  for (const value of Object.values(limits)) {
    if (!Number.isSafeInteger(value) || value <= 0) throw new Error('Import limits must be positive safe integers.')
  }
}
function object(value: unknown): ObjectValue {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return invalid()
  return value as ObjectValue
}
function fields(value: ObjectValue, required: readonly string[], optional: readonly string[] = []): void {
  if (required.some(key => !Object.hasOwn(value, key)) || Object.keys(value).some(key => !required.includes(key) && !optional.includes(key))) invalid()
}
function text(value: unknown, empty = false): string {
  if (typeof value !== 'string' || (!empty && !value.length)) return invalid()
  return value
}
function integer(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) return invalid()
  return value
}
function hash(value: unknown): string {
  const result = text(value)
  if (!hashes.test(result)) invalid()
  return result
}
function enumeration<const T extends readonly string[]>(value: unknown, choices: T): T[number] {
  const result = text(value)
  if (!choices.includes(result)) invalid()
  return result as T[number]
}
function array(value: unknown, max: number): unknown[] {
  if (!Array.isArray(value)) return invalid()
  if (value.length > max) bounded()
  return value
}
function asset(value: unknown, limits: BundleLimits): Asset {
  const item = object(value)
  fields(item, ['digest', 'bytes', 'roles'])
  const bytes = integer(item.bytes)
  if (bytes > limits.maxAssetBytes) bounded()
  const roles: AssetKind[] = array(item.roles, 5).map(role => enumeration(role, ['workspace', 'audio-input', 'audio-output', 'screenshot', 'provider-tape']))
  if (!roles.length || new Set(roles).size !== roles.length) invalid()
  return { digest: hash(item.digest), bytes, roles }
}
function event(value: unknown): TimedEvent {
  const item = object(value)
  const common = ['kind', 'sequence', 'elapsedMicros']
  const base = { sequence: integer(item.sequence), elapsedMicros: integer(item.elapsedMicros) }
  switch (item.kind) {
    case 'action':
      fields(item, [...common, 'target', 'action', 'value'])
      return { ...base, kind: 'action', target: text(item.target), action: enumeration(item.action, ['click', 'input', 'composition', 'key', 'pointer', 'wheel', 'scroll', 'focus']), value: text(item.value, true) }
    case 'display':
      fields(item, [...common, 'target', 'text'])
      return { ...base, kind: 'display', target: text(item.target), text: text(item.text, true) }
    case 'audio':
      fields(item, [...common, 'action', 'role', 'asset'])
      return { ...base, kind: 'audio', action: enumeration(item.action, ['captured', 'requested', 'started', 'ended', 'cancelled']), role: enumeration(item.role, ['audio-input', 'audio-output']), asset: hash(item.asset) }
    case 'timing':
      fields(item, [...common, 'trace', 'span', 'phase', 'clock', 'micros'])
      return { ...base, kind: 'timing', trace: text(item.trace), span: text(item.span), phase: enumeration(item.phase, ['start', 'end']), clock: enumeration(item.clock, ['ui', 'native', 'runner']), micros: integer(item.micros) }
    case 'provider':
      fields(item, [...common, 'request', 'fingerprint', 'phase', 'body'])
      return { ...base, kind: 'provider', request: text(item.request), fingerprint: hash(item.fingerprint), phase: enumeration(item.phase, ['request', 'chunk', 'success', 'failure', 'cancelled']), body: text(item.body, true) }
    default: return invalid()
  }
}

/** Decode untrusted UTF-8 bytes before reading any manifest-selected file paths.
 * Unknown fields/variants fail; content strings are preserved, not normalized. */
export function decodeManifest(bytes: Uint8Array, limits: BundleLimits = defaultBundleLimits): Bundle {
  checkLimits(limits)
  if (bytes.byteLength > limits.maxManifestBytes) bounded()
  let parsed: unknown
  try { parsed = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) as unknown }
  catch { throw new Error('Recording manifest is not valid UTF-8 JSON.') }
  const item = object(parsed)
  fields(item, ['format', 'build', 'workspace', 'status', 'assets', 'events'], ['reason', 'providerTape'])
  if (item.format !== 1 || item.status !== 'complete' || Object.hasOwn(item, 'reason')) throw new Error('Recording is incomplete or unsupported.')
  const assets = array(item.assets, limits.maxAssets).map(value => asset(value, limits))
  let total = 0
  for (const value of assets) {
    total += value.bytes
    if (!Number.isSafeInteger(total) || total > limits.maxTotalAssetBytes) bounded()
  }
  return {
    format: 1, build: text(item.build), workspace: hash(item.workspace), status: 'complete',
    ...(Object.hasOwn(item, 'providerTape') ? { providerTape: hash(item.providerTape) } : {}),
    assets, events: array(item.events, limits.maxEvents).map(event),
  }
}
