import type { TreeNode } from '../catalog/skillTree'
import type { SkillLevelEvent, SkillLevelSummary } from '../../../generated/contracts'

/// Skill evidence: records, snapshots and the credit arithmetic that folds an
/// awarded attempt into a learner profile.
///
/// This module is deliberately pure. Everything that crosses IPC — reading the
/// evidence, saving profile choices, subscribing to changes — lives in
/// `./skill-evidence`, so the snapshot and credit logic here stays importable
/// from anywhere without dragging the Tauri boundary along with it.

export interface InputEvidence {
  recording_ids?: string[]
  modality: 'text' | 'speech_transcript'
  suggestion: boolean
  scaffold: boolean
  revision: boolean
}
export const unreportedInput = (): InputEvidence => ({ modality: 'text', suggestion: false, scaffold: false, revision: false })
export type SkillOutcome = import('../../../generated/contracts').Outcome
export interface SkillJudgment {
  spans?: { quote: string; start: number; end: number }[]
  attribution_reason?: string | null
  evidence_kind?: 'quoted' | 'whole_message'
  scores?: { evidence: number; full: number } | null
  answer?: { choice: string; confidence: number; probabilities: Record<string, number> } | null
  skill_id: string
  presence?: 'absent' | 'contextual' | 'direct' | 'unclear'
  outcome?: SkillOutcome
  quotes: string[]
  rationale: string
}
export interface SkillRecord {
  attribution_state?: string | null
  attribution_error?: string | null
  attribution_attempt?: string | null
  assessment_adapter?: 'jev_choice' | 'chat_model'
  decision_policy?: { version: string; minimumPositiveProbability?: number; evidenceThreshold?: number; fullThreshold?: number } | null
  variety?: string | null
  construct_registry_hash: string | null
  mapping_error: string | null
  support_step: import('../../../generated/contracts').CoachMove | null
  attempt_id: string
  session_id: string
  turn_id: number
  message_id: number
  replaces_message_id: number | null
  chat_id: string
  learner_id: string
  target: string
  native: string
  source: string
  input: InputEvidence
  at_secs: number
  model: string
  provider_mode: string
  catalog_version: number
  prompt_version: string
  status: 'pending' | 'complete' | 'failed' | 'superseded'
  assessment: { judgments: SkillJudgment[] } | null
  error: string | null
}
/// A snapshot is written by exactly one rubric. A record from another catalog is
/// a defect to report, never a record to filter out quietly.
export function requireCatalogVersion(snapshot: SkillSnapshot, record: SkillRecord): void {
  if (record.construct_registry_hash !== snapshot.construct_registry_hash) throw new Error('Evidence uses a different construct registry without a native mapping error.')
  if (record.catalog_version !== snapshot.catalog_version) {
    throw new Error(`Evidence uses catalog ${record.catalog_version}, and this snapshot uses ${snapshot.catalog_version}`)
  }
}

export interface SkillSnapshot {
  guide_explanation_language?: string
  guides?: { id: string; name: string; skills: Record<string, string | null> }[]
  construct_registry_hash: string
  catalog: TreeNode[]
  catalog_version: number
  learner_id: string
  target: string
  conversation_count: number
  records: SkillRecord[]
  profile: LearnerProfile
}
export interface ProfileChoices {
  version: number
  revision: number
  learner_id: string
  target: string
  focus: string | null
  excluded_attempts: string[]
}
export interface SkillProgress {
  skill_id: string
  experience: number
  effort: number
  xp: number
  checked: boolean
  star: boolean
}
export interface LearnerProfile {
  credits: { attempt_id: string; skill_id: string; xp: number; experience?: number; effort?: number; event?: import('../../../generated/contracts').RewardEvent }[]
  rules_version: number
  choices: ProfileChoices
  xp: number
  skills: SkillProgress[]
  branches: { skill_id: string; available: boolean }[]
  recommended_focus: string
  active_focus: string
  /** Native Fibonacci level projection over the eligible credits of this scope.
   * Null for UI-derived scopes (one conversation), which have no level. */
  levels: SkillLevelSummary | null
  /** Unclaimed level celebrations; only the full-language scope carries any. */
  pendingLevelEvents: SkillLevelEvent[]
}
export interface PracticeOverview {
  languages: { name: string; endonym: string; snapshot: SkillSnapshot }[]
}

/** Attribute existing awarded credit to its conversation without inventing new XP. */
export function conversationEvidence(snapshot: SkillSnapshot, chatId: string): SkillSnapshot {
  const records = snapshot.records.filter(record => record.chat_id === chatId)
  const ids = new Set(records.map(record => record.attempt_id))
  const credits = snapshot.profile.credits.filter(credit => ids.has(credit.attempt_id))
  const skills = snapshot.profile.skills.map(skill => {
    const own = credits.filter(credit => credit.skill_id === skill.skill_id)
    const experience = own.reduce((sum, credit) => sum + (credit.experience ?? credit.event?.experience ?? 0), 0)
    const effort = own.reduce((sum, credit) => sum + (credit.effort ?? credit.event?.effort ?? 0), 0)
    return { ...skill, xp: own.reduce((sum, credit) => sum + credit.xp, 0), experience, effort, checked: experience > 0, star: false }
  })
  // A conversation has points but no level: language levels and celebrations never travel with it.
  return { ...snapshot, records, conversation_count: 1, profile: { ...snapshot.profile, skills, credits, xp: credits.reduce((sum, credit) => sum + credit.xp, 0), levels: null, pendingLevelEvents: [] } }
}
