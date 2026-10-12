import { SKILL_CATALOG_VERSION } from '../../../generated/contracts'
import { skillDemo } from '../../../domain/learning/catalog/skillDemo'
import type { AudioInspection, ConversationFeedback, DrillAttemptView, InspectionSpectrogram, WordComparison, WordOutcome } from '../../../generated/contracts'
import type { SkillSnapshot } from '../../../domain/learning/evidence/skills'
import type { GuidedTurnResult } from '../../../types'

/// Fixed content for the tour's demo pages: the same production components the
/// real surfaces use, fed with data that never depends on what the learner has
/// actually done. Nothing here is fetched, recorded or sent.

// ── Chat ─────────────────────────────────────────────────────────────────
export const CHAT_QUESTION: GuidedTurnResult = {
  reply: '¿Qué hiciste el fin de semana?',
  tokens: [
    { text: '¿Qué', gloss: 'What', pos: null, notable: false, romanization: null, pronunciation: null },
    { text: 'hiciste', gloss: 'did you do', pos: null, notable: false, romanization: null, pronunciation: null },
    { text: 'el', gloss: 'the', pos: null, notable: false, romanization: null, pronunciation: null },
    { text: 'fin de semana', gloss: 'weekend', pos: null, notable: false, romanization: null, pronunciation: null },
  ],
  user_tokens: [], translation: 'What did you do at the weekend?', user_translation: null,
  mechanics: [], scaffolds: { replies: [], frames: [], starters: [] }, errors: [],
}
export const CHAT_REPLY: GuidedTurnResult = {
  reply: '¡Qué bien! ¿Qué compraste?',
  tokens: [
    { text: '¡Qué', gloss: 'How', pos: null, notable: false, romanization: null, pronunciation: null },
    { text: 'bien!', gloss: 'nice!', pos: null, notable: false, romanization: null, pronunciation: null },
    { text: '¿Qué', gloss: 'What', pos: null, notable: false, romanization: null, pronunciation: null },
    { text: 'compraste?', gloss: 'did you buy?', pos: null, notable: false, romanization: null, pronunciation: null },
  ],
  user_tokens: [], translation: 'How nice! What did you buy?', user_translation: 'Yesterday go to the market.',
  mechanics: [], scaffolds: { replies: [], frames: [], starters: [] }, errors: [],
}
export const CHAT_FEEDBACK: ConversationFeedback = { grammar: 3, conversation: 5, answers: {} }
export const CHAT_READING_SCOPE = { language: 'spanish', variety: 'spanish-spain', explanation: 'english', explanationVariety: 'english-united-states' }

// ── Drill ────────────────────────────────────────────────────────────────
// A procedural stand-in spectrogram: shaped like formant bands moving over
// time, not derived from any recording, so the tour never ships anyone's voice.
function syntheticSpectrogram(duration: number, seed: number): InspectionSpectrogram {
  const frameSeconds = 0.02, bandCount = 48, frameCount = Math.round(duration / frameSeconds)
  const bands = Array.from({ length: bandCount }, (_, band) => {
    const lowHz = 80 * 1.08 ** band, highHz = 80 * 1.08 ** (band + 1)
    return { lowHz, centerHz: (lowHz + highHz) / 2, highHz }
  })
  const formants = [4 + seed, 14 + seed, 26 + seed]
  const bins = Array.from({ length: frameCount }, (_, frame) => {
    const t = frame / frameCount
    const envelope = Math.sin(Math.PI * Math.min(1, t * 6)) * Math.sin(Math.PI * Math.min(1, (1 - t) * 6))
    return Array.from({ length: bandCount }, (_, band) => {
      const closeness = formants.reduce((best, center) => Math.max(best, Math.exp(-((band - center - 2 * Math.sin(t * 9 + center)) ** 2) / 10)), 0)
      return -70 + 55 * Math.max(0, envelope) * closeness
    })
  })
  return {
    frameSeconds, frameStartSeconds: Array.from({ length: frameCount }, (_, index) => index * frameSeconds),
    windowSeconds: frameSeconds * 2, fftSize: 1024, bands, minFrequencyHz: 80, maxFrequencyHz: 8000, measuredMaxFrequencyHz: 7800,
    melScale: 'htk', normalization: 'peak', dbReference: 'full_scale', dbMin: -70, dbMax: 0, bins,
  }
}
function syntheticInspection(id: string, duration: number, seed: number): AudioInspection {
  return {
    recordingId: id, owner: { kind: 'drillItem', id: 'demo' }, duration, sampleRate: 16000,
    waveform: { binSeconds: 0.02, min: Array.from({ length: Math.round(duration / 0.02) }, () => -0.2), max: Array.from({ length: Math.round(duration / 0.02) }, () => 0.2) },
    spectrogram: syntheticSpectrogram(duration, seed),
    activity: { algorithm: 'demo', noiseFloorDbfs: -55, thresholdDbfs: -40, regions: [{ start: 0.1, end: duration - 0.1 }], pauses: [], limitations: [] },
    wordTiming: { status: 'available', reason: null, unsupported: [], words: [] },
  }
}
const DRILL_TARGET_WORDS = ['أنا', 'بفهم', 'الخرايط', 'القديمة', 'شوية']
function timedAudio(inspection: AudioInspection): AudioInspection {
  return { ...inspection, wordTiming: { status: 'available', reason: null, unsupported: [],
    words: DRILL_TARGET_WORDS.map((word, index) => {
      const width = inspection.duration / DRILL_TARGET_WORDS.length
      return { index, word, providerStart: index * width, providerEnd: (index + 0.9) * width, start: index * width, end: (index + 0.9) * width, clipped: false }
    }) } }
}
const DRILL_REFERENCE = syntheticInspection('demo-reference', 1.9, 0)
const DRILL_SPOKEN = syntheticInspection('demo-spoken', 2.1, 1)
function drillWord(text: string, kind: WordOutcome): WordComparison {
  return kind === 'same' ? { kind, target: text, transcript: text, similarity: null }
    : kind === 'missing' ? { kind, target: text, transcript: null, similarity: null }
    : { kind, target: text, transcript: `${text.slice(0, -1)}ة`, similarity: 0.7 }
}
const DRILL_OUTCOMES: WordOutcome[][] = [
  ['same', 'same', 'substituted', 'missing', 'same'],
  ['same', 'same', 'same', 'same', 'same'],
  ['same', 'same', 'substituted', 'same', 'missing'],
  ['same', 'same', 'substituted', 'same', 'same'],
]
const DRILL_RATIOS = [0.48, 0.93, 0.45, 0.88]
export const DRILL_ATTEMPTS: DrillAttemptView[] = DRILL_OUTCOMES.map((kinds, index) => ({
  id: `demo-attempt-${index + 1}`, sequence: BigInt(index + 1), visitId: null, audioBytes: 400000n, audioPrunedAt: null,
  transcriptionAttemptId: null, countedAsPractice: false, createdAt: new Date(Date.UTC(2026, 8, 23, 14, 27, index * 9)).toISOString(),
  transcript: kinds.map((kind, n) => kind === 'missing' ? '' : drillWord(DRILL_TARGET_WORDS[n], kind).transcript).join(' '),
  comparison: {
    policy: 'drill-comparison-v1', target: DRILL_TARGET_WORDS.join(' '), transcript: '', normalizations: ['strip_punctuation'],
    normalizedTarget: DRILL_TARGET_WORDS.join(' '), normalizedTranscript: '', edits: kinds.filter(kind => kind !== 'same').length,
    referenceGraphemes: 26, characterErrorRate: 1 - DRILL_RATIOS[index], matchRatio: DRILL_RATIOS[index], scriptNote: 'matches',
    words: kinds.map((kind, n) => drillWord(DRILL_TARGET_WORDS[n], kind)),
  },
}) as unknown as DrillAttemptView).reverse()
export const DRILL_REFERENCE_AUDIO = timedAudio(DRILL_REFERENCE)
export const DRILL_ATTEMPT_AUDIO = timedAudio(DRILL_SPOKEN)
export const DRILL_PHRASE = { id: 'demo', text: DRILL_TARGET_WORDS.join(' ') }
export const DRILL_SECOND_PHRASE = { id: 'demo-2', text: '¿Qué compraste allí?' }

// ── Progress ─────────────────────────────────────────────────────────────
function progressSnapshot(): SkillSnapshot {
  const snapshot = structuredClone(skillDemo)
  const picked = snapshot.profile.skills.filter((_, index) => index % 3 === 0).slice(0, 10)
  const counts = [4, 3, 3, 2, 2, 2, 1, 1, 1, 1]
  let total = 0
  snapshot.profile.credits = []
  snapshot.records = []
  picked.forEach((skill, index) => {
    const count = counts[index]
    for (let take = 0; take < count; take++) {
      const attemptId = `demo-${index}-${take}`
      snapshot.profile.credits.push({ attempt_id: attemptId, skill_id: skill.skill_id, xp: 10 })
      snapshot.records.push({
        attempt_id: attemptId, session_id: 'demo', turn_id: take, message_id: index * 10 + take, replaces_message_id: null,
        construct_registry_hash: 'fixture-registry', mapping_error: null, support_step: null, chat_id: 'demo-chat', learner_id: 'demo',
        target: 'spanish-spain', native: 'english', source: 'Esa taza.', input: { modality: 'text', suggestion: false, scaffold: false, revision: false },
        at_secs: 100 + index * 3600 + take * 60, model: 'demo', provider_mode: 'custom', catalog_version: SKILL_CATALOG_VERSION, prompt_version: 'v1',
        status: 'complete', assessment: { judgments: [{ skill_id: skill.skill_id, outcome: 'demonstrated', quotes: ['taza'], rationale: 'Identifies the cup.' }] }, error: null,
      })
    }
    Object.assign(skill, { xp: count * 10, successes: count, checked: true, star: count >= 3 })
    total += count * 10
  })
  snapshot.profile.xp = total
  snapshot.conversation_count = 4
  return snapshot
}
export const PROGRESS_SNAPSHOT = progressSnapshot()
