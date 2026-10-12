import type { Shape } from '../../generated/graph-contracts'

/// Presentation identities derived from authored graph schema. Nothing here
/// names an operation or a type: every value is computed from the identifier
/// or the shape itself, so a new operation or contract needs no viewer change.

/// Number of data hues available to type identities. Matches the
/// `--graph-type-0` … `--graph-type-7` token set.
export const TYPE_HUE_COUNT = 8

/// A deterministic hue slot for a type contract. Distinct contracts may share a
/// slot; the type name is always shown beside its color, so a shared hue never
/// carries meaning on its own.
export function typeHue(contract: string): number {
  if (!contract) throw new Error('A type contract must be a non-empty string.')
  let hash = 21
  for (let index = 0; index < contract.length; index++) hash = (hash * 31 + contract.charCodeAt(index)) % 2147483647
  return hash % TYPE_HUE_COUNT
}

/// The words of a contract's last dotted segment, without its version prefix
/// (`1:conversation.validated-context` → `['validated', 'context']`).
function contractWords(contract: string): string[] {
  if (!contract) throw new Error('A contract must be a non-empty string.')
  const unversioned = contract.replace(/^\d+:/, '')
  const segment = unversioned.slice(unversioned.lastIndexOf('.') + 1)
  const words = segment.split(/[-_]/).filter(Boolean)
  if (!words.length) throw new Error(`Contract has no displayable name: ${contract}`)
  return words
}

/// A type contract as plain words (`1:conversation.generated-text` →
/// `generated text`). The full contract remains the identity.
export function typeName(contract: string): string {
  return contractWords(contract).join(' ')
}

/// Whether a contract belongs to the `ai` namespace: credentials, install ids,
/// model targets and sampling settings that let a model call happen. They are
/// counted on a node, never shown as rows.
export function isAccessContract(contract: string): boolean {
  if (!contract) throw new Error('A contract must be a non-empty string.')
  return contract.replace(/^\d+:/, '').startsWith('ai.')
}

/// An operation contract as the verb phrase its author gave it
/// (`1:conversation.generate-prose` → `Generate prose`).
export function operationVerb(contract: string): string {
  const words = contractWords(contract)
  return words.map((word, index) => index === 0 ? word.charAt(0).toUpperCase() + word.slice(1) : word).join(' ')
}

/// A readable label for one authored identifier segment (`reply_source` →
/// `Reply source`). Callers pass a node's local name; qualified composition
/// members are shown inside their group by their own segment.
export function humanizeId(name: string): string {
  if (!name) throw new Error('An identifier must be a non-empty string.')
  const spaced = name.replace(/_/g, ' ')
  return spaced.charAt(0).toUpperCase() + spaced.slice(1)
}

/// A field name as words (`targetLanguage` → `target language`).
export function fieldWords(field: string): string {
  if (!field) throw new Error('A field name must be a non-empty string.')
  return field.replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/[_-]/g, ' ').toLowerCase()
}

/// The shape of a value as words, one level deep: a record lists its fields, a
/// list names what it holds, scalars name themselves.
export function shapeWords(shape: Shape): string {
  if (typeof shape === 'string') return shape.toLowerCase()
  if ('Nullable' in shape) return `${shapeWords(shape.Nullable)} or nothing`
  if ('List' in shape) return `list of ${shapeWords(shape.List)}`
  if ('Map' in shape) return `map of ${shapeWords(shape.Map)}`
  const fields = Object.keys(shape.Record)
  if (!fields.length) throw new Error('A record shape must declare at least one field.')
  return fields.map(fieldWords).join(', ')
}

/// The top-level field names of a record shape, or none for other shapes.
export function shapeFields(shape: Shape): string[] {
  return typeof shape !== 'string' && 'Record' in shape ? Object.keys(shape.Record) : []
}
