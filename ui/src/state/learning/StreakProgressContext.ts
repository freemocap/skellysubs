import { createContext, useContext } from 'react'
import type { StreakSnapshot } from '../../generated/contracts'

export const StreakProgressContext = createContext<{
  value: StreakSnapshot | null; error: string | null; effects: boolean
}>({ value: null, error: null, effects: true })
export const useVisibleStreaks = () => useContext(StreakProgressContext)
