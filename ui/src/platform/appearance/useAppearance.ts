import { useEffect, useLayoutEffect } from 'react'
import { DEFAULT_APPEARANCE } from '../../generated/contracts'
import { applyMotion } from './motion'
import type { Settings } from '../../types'

/** Apply learner-owned appearance at the app boundary, including portaled overlays. */
export function useAppearance(settings: Settings | null) {
  const appearance = settings?.appearance ?? DEFAULT_APPEARANCE
  const theme = settings?.theme ?? 'light'
  useLayoutEffect(() => applyMotion(appearance.motion), [appearance.motion])
  useEffect(() => {
    const root = document.documentElement
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const update = () => { root.dataset.theme = theme === 'system' ? (media.matches ? 'dark' : 'light') : theme }
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [theme])
  useEffect(() => {
    document.documentElement.dataset.palette = appearance.palette
  }, [appearance.palette])
}
