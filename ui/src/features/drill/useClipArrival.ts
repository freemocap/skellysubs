import { reducedMotion } from '../../platform/appearance/motion'
import { useEffect, useRef } from 'react'

/** Move a copy of the clipped plot; the destination remains painted throughout. */
export function useClipArrival(recordingId?: string) {
  const target = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const destination = target.current
    if (!recordingId || !destination || !destination.animate || reducedMotion()) return
    const source = [...document.querySelectorAll<HTMLElement>('[data-clip-id]')].find(element => element.dataset.clipId === recordingId)
    if (!source) return
    const from = source.getBoundingClientRect(), to = destination.getBoundingClientRect()
    if (!from.width || !to.width || !to.height) return
    const canvas = destination.querySelector('canvas')
    if (!canvas) return
    const copy = canvas.cloneNode() as HTMLCanvasElement
    copy.getContext('2d')?.drawImage(canvas, 0, 0)
    copy.setAttribute('aria-hidden', 'true')
    copy.className = 'drill-clip-flight'
    Object.assign(copy.style, { left: `${to.left}px`, top: `${to.top}px`, width: `${to.width}px`, height: `${to.height}px` })
    document.body.append(copy)
    destination.dataset.clipFlightUntil = String(performance.now() + 1000)
    const animation = copy.animate([
      { transform: `translate(${from.left - to.left}px, ${from.top - to.top}px) scale(${from.width / to.width}, ${from.height / to.height})`, opacity: 0.8 },
      { transform: 'none', opacity: 0.8, offset: 0.85 },
      { transform: 'none', opacity: 0 },
    ], { duration: 1000, easing: 'ease-in-out' })
    animation.onfinish = () => copy.remove()
    return () => { animation.cancel(); copy.remove(); delete destination.dataset.clipFlightUntil }
  }, [recordingId])
  return target
}
