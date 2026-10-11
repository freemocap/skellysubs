import { invoke } from './tauri'
import { batchRead } from './batch-read'
import type { PracticeOverview, ProfileChoices, SkillSnapshot } from '../../domain/learning/evidence/skills'

/// The IPC half of skill evidence. Every function here crosses the Tauri
/// boundary; the snapshot types and credit arithmetic they speak in live in
/// `domain/learning/evidence/skills`.
///
/// Nothing here reads settings: the caller passes the target it wants, so this
/// layer stays a boundary rather than a second reader of another store.

export function getSkillEvidence(target: string): Promise<SkillSnapshot> {
  return invoke('get_skill_evidence', { target })
}

export function saveSkillProfile(choices: ProfileChoices): Promise<SkillSnapshot> {
  return invoke('save_skill_profile', { target: choices.target, expectedRevision: choices.revision, choices })
}

export function getPracticeOverview(): Promise<PracticeOverview> { return invoke('get_practice_overview') }

export function getSkillGuide(language: string, variety: string, skill: string, explanation: string, retry = false): Promise<import('../../generated/contracts').SkillGuideResult> {
  return invoke('get_skill_guide', { language, variety, skill, explanation, retry })
}

/** XP, conversations and lifetime effort per catalog language, without full snapshots. */
export const getLanguageTotals = batchRead(() => invoke<import('../../generated/contracts').LanguageTotals[]>('get_language_totals'))
