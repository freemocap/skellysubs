import type { ChatMessage, ConversationSnapshot, TurnView } from '../../generated/contracts'

/// The conversation's record, exploded: every message as the chat keeps it, and
/// beside it every field the app has filled on that message, named exactly as
/// the data model names it. Fields that a graph run filled carry the graph
/// result names the turn's surfaces map them to, so the record can point back
/// at the machinery.

/// Message fields that are the message itself, shown on the bubble rather than
/// as fields beside it.
const BUBBLE_FIELDS = new Set<keyof ChatMessage>(['id', 'role', 'text', 'sequence', 'createdAt', 'turnId', 'replacesTurnId', 'replacedBy'])

/// The turn surface each message field is filled from, by the role of the
/// message that owns the field. Surfaces are the keys of `TurnView.nativeSurfaces`.
const SURFACE_OF_FIELD: Record<string, { user?: string; assistant?: string }> = {
  translation: { user: 'learner.translation', assistant: 'reply.translation' },
  wordGloss: { user: 'learner.gloss', assistant: 'reply.gloss' },
  feedback: { user: 'coach_feedback' },
  assessmentState: { user: 'skill_assessment' },
  coachDecision: { user: 'attribution' },
  replyBrief: { assistant: 'reply_brief' },
  replyExplanations: { assistant: 'reply_explanations' },
  replyAssistance: { assistant: 'reply_assistance' },
}

export interface RecordField {
  /// The field's name in the data model.
  name: string
  /// The matching `…State` field's value, when the model declares one.
  state: string | null
  /// The matching `…Error` field's value, when set.
  error: string | null
  /// The field's value as the message carries it.
  value: unknown
  /// The turn surface this field is filled from, and the graph results behind it.
  surface: string | null
  results: string[]
}

export interface RecordMessage {
  id: string
  role: string
  text: string
  sequence: number
  createdAt: string
  turnId: string
  turn: TurnView | null
  fields: RecordField[]
}

/// The base name of a state or error field (`glossState` → `gloss`), or null.
function companionBase(name: string, suffix: 'State' | 'Error'): string | null {
  return name.endsWith(suffix) && name.length > suffix.length ? name.slice(0, -suffix.length) : null
}

/// Fields whose value is a state or error for another field: `translationState`
/// belongs to `translation`, `briefState` to `replyBrief`, `glossState` to `wordGloss`.
function ownerOf(base: string, keys: string[]): string | null {
  if (keys.includes(base)) return base
  const match = keys.find(key => key.toLowerCase().endsWith(base.toLowerCase()) && !key.endsWith('State') && !key.endsWith('Error'))
  return match ?? null
}

export function recordFields(message: ChatMessage, turn: TurnView | null): RecordField[] {
  const record = message as unknown as Record<string, unknown>
  const keys = Object.keys(record).filter(key => !BUBBLE_FIELDS.has(key as keyof ChatMessage) && record[key] !== undefined)
  const states = new Map<string, string>(), errors = new Map<string, string>()
  const owned = new Set<string>()
  for (const key of keys) {
    const stateBase = companionBase(key, 'State'), errorBase = companionBase(key, 'Error')
    const base = stateBase ?? errorBase
    if (!base) continue
    const owner = ownerOf(base, keys)
    if (!owner) continue
    const value = record[key]
    if (value === null) { owned.add(key); continue }
    if (typeof value !== 'string') throw new Error(`Message field ${key} is not a string.`)
    if (stateBase) states.set(owner, value); else errors.set(owner, value)
    owned.add(key)
  }
  return keys.filter(key => !owned.has(key) && (record[key] !== null || states.has(key) || errors.has(key))).map(name => {
    const surface = SURFACE_OF_FIELD[name]?.[message.role as 'user' | 'assistant'] ?? null
    const results = surface && turn?.nativeSurfaces?.[surface] ? turn.nativeSurfaces[surface] : []
    const value = record[name] ?? null
    return { name, state: states.get(name) ?? null, error: errors.get(name) ?? null, value, surface, results }
  })
}

/// The turn's speech request belongs to the message it reads aloud.
const SPEECH_SURFACE = 'audio'

function speechField(message: ChatMessage, turn: TurnView | null): RecordField[] {
  const speech = turn?.speech
  if (!speech || speech.sourceMessageId !== message.id) return []
  const results = turn.nativeSurfaces?.[SPEECH_SURFACE] ?? []
  return [{ name: 'speech', state: speech.state, error: null, value: speech, surface: SPEECH_SURFACE, results }]
}

export function recordMessages(snapshot: ConversationSnapshot): RecordMessage[] {
  return [...snapshot.messages].sort((a, b) => a.sequence - b.sequence).map(message => {
    const turn = snapshot.turns.find(candidate => candidate.id === message.turnId) ?? null
    return { id: message.id, role: message.role, text: message.text, sequence: message.sequence, createdAt: message.createdAt, turnId: message.turnId, turn, fields: [...recordFields(message, turn), ...speechField(message, turn)] }
  })
}

/// What a run step made, as the conversation shows it: the message field a
/// graph result of the step fills, with the field's value. The text result is
/// the partner's message itself; the audio result is the turn's speech request.
export interface StepProduct {
  result: string
  surface: string
  /// Whose message carries the field, and which field.
  role: string
  field: string
  value: unknown
}

const TEXT_SURFACE = 'text'

export function stepProducts(view: { results: Array<{ port: string; value: { kind: string; producer?: { node: string } } }> }, turn: TurnView, messages: ChatMessage[], node: string): StepProduct[] {
  const produced = new Set(view.results.filter(result => result.value.kind === 'wired' && result.value.producer!.node === node).map(result => result.port))
  if (!produced.size || !turn.nativeSurfaces) return []
  const products: StepProduct[] = []
  for (const [surface, results] of Object.entries(turn.nativeSurfaces)) {
    for (const result of results) {
      if (!produced.has(result)) continue
      if (surface === TEXT_SURFACE) {
        const reply = messages.find(message => message.turnId === turn.id && message.role === 'assistant')
        if (reply) products.push({ result, surface, role: reply.role, field: 'text', value: reply.text })
        continue
      }
      if (surface === SPEECH_SURFACE) {
        if (turn.speech) products.push({ result, surface, role: 'assistant', field: 'speech', value: turn.speech })
        continue
      }
      for (const [field, roles] of Object.entries(SURFACE_OF_FIELD)) {
        for (const [role, owned] of Object.entries(roles)) {
          if (owned !== surface) continue
          const message = messages.find(candidate => candidate.turnId === turn.id && candidate.role === role)
          const value = message ? (message as unknown as Record<string, unknown>)[field] : undefined
          if (value !== undefined && value !== null) products.push({ result, surface, role, field, value })
        }
      }
    }
  }
  return products
}
