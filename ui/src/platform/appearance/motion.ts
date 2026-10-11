import { useSyncExternalStore } from 'react'
import type { MotionPreference } from '../../generated/contracts'

const query = '(prefers-reduced-motion: reduce)'
const changeEvent = 'skellyspeak-motion-change'

export function reducedMotion() {
  const resolved = document.documentElement.dataset.motion
  return resolved ? resolved === 'reduced' : window.matchMedia(query).matches
}

export function applyMotion(preference: MotionPreference) {
  const media = window.matchMedia(query)
  const update = () => {
    document.documentElement.dataset.motion = preference === 'system' ? (media.matches ? 'reduced' : 'full') : preference
    window.dispatchEvent(new Event(changeEvent))
  }
  update()
  media.addEventListener('change', update)
  return () => {
    media.removeEventListener('change', update)
    delete document.documentElement.dataset.motion
  }
}

function subscribe(listener: () => void) {
  const media = window.matchMedia(query)
  media.addEventListener('change', listener)
  window.addEventListener(changeEvent, listener)
  return () => {
    media.removeEventListener('change', listener)
    window.removeEventListener(changeEvent, listener)
  }
}

export function useReducedMotion() {
  return useSyncExternalStore(subscribe, reducedMotion)
}
