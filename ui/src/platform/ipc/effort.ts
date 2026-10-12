import { publishEffortAwards } from './reward-origin'
import { invoke } from '@tauri-apps/api/core'
import type { EffortProgress } from '../../generated/contracts'
import { effortPublished } from './effort-events'

export async function recordBotInspection(selection: Pick<import('../../generated/contracts').NativeAttemptInspection, 'engine' | 'run' | 'revision' | 'node' | 'attempt'>): Promise<void> {
  const { engine, run, revision, node, attempt } = selection
  const award = await invoke<import('../../generated/contracts').EffortAward | null>('record_bot_inspection', { engine, run, revision, node, attempt })
  if (award) {
    publishEffortAwards([award])
    effortPublished()
  }
}

/** Language totals, or one conversation's when `conversation` is given. */
export const getEffortProgress = (target: string, conversation: string | null = null): Promise<EffortProgress> => invoke<EffortProgress>('get_effort_progress', { target, conversation }).then(value => { publishEffortAwards(value.recent); return value })
export const claimEffortAwards = (target: string, ids: string[]): Promise<string[]> => invoke('claim_effort_awards', { target, ids })

export const getEffortReport = (target: string, dimension: import('../../generated/contracts').EffortDimension | null, before: string | null): Promise<import('../../generated/contracts').EffortReport> => invoke('get_effort_report', { target, dimension, before })
