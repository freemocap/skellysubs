import { useEffect, useRef, useState } from 'react'
import type { StreakSnapshot } from '../../generated/contracts'
import { refreshStreaks } from '../../platform/ipc/streaks'
import { errorMessage } from '../../platform/diagnostics/error-details'

/** One main-shell owner records foreground use and consumes presentation claims.
 * Polls only read; language selection has no bearing on app activity. */
export function useStreakProgress(sessionId: string, effects: boolean) {
  const enabled = useRef(effects)
  enabled.current = effects
  const [state, setState] = useState<{ value: StreakSnapshot | null; error: string | null; key: string }>({ value: null, error: null, key: '' })
  useEffect(() => {
    if (!sessionId) return
    let alive = true
    let pending = false
    let queued = false
    let activity = false
    let lastInteraction = ''
    const foreground = () => document.visibilityState === 'visible' && document.hasFocus()
    async function refresh(active: boolean) {
      activity ||= active
      queued = true
      if (pending) return
      pending = true
      while (alive && queued) {
        queued = false
        const record = activity
        activity = false
        try {
          const value = await refreshStreaks(sessionId, record)
          if (!alive) break
          if (value.sessionId !== sessionId) throw new Error('Streak response scope mismatch.')
          const app = { ...value.app, celebration: enabled.current ? value.app.celebration : null }
          setState({ key: sessionId, value: { ...value, app }, error: null })
        } catch (error) {
          if (record) lastInteraction = ''
          if (alive) setState(previous => ({ key: sessionId, value: previous.key === sessionId ? previous.value : null, error: errorMessage(error) }))
        }
      }
      pending = false
    }
    const focus = () => { if (foreground()) void refresh(true) }
    const interaction = () => {
      if (!foreground()) return
      const date = new Date().toDateString()
      if (lastInteraction === date) return
      lastInteraction = date
      void refresh(true)
    }
    window.addEventListener('focus', focus)
    document.addEventListener('visibilitychange', focus)
    document.addEventListener('pointerdown', interaction)
    document.addEventListener('keydown', interaction)
    const timer = window.setInterval(() => { if (foreground()) void refresh(false) }, 30000)
    void refresh(foreground())
    return () => {
      alive = false
      window.clearInterval(timer)
      window.removeEventListener('focus', focus)
      document.removeEventListener('visibilitychange', focus)
      document.removeEventListener('pointerdown', interaction)
      document.removeEventListener('keydown', interaction)
    }
  }, [sessionId])
  return { ...(state.key === sessionId ? state : { value: null, error: null }), effects }
}
