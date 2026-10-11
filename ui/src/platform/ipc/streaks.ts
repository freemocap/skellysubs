import type { StreakSnapshot } from '../../generated/contracts'
import { invoke } from './native'

export const refreshStreaks = (sessionId: string, active: boolean) =>
  invoke<StreakSnapshot>('refresh_streaks', { sessionId, active })
