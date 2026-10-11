import { reducedMotion } from '../../platform/appearance/motion'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'

/** Publication waits for the playback words to settle, then flies into history. */
export function useWordArrival(id: string | null, ready: boolean, scope: string | null) {
  const [released, setReleased] = useState<string | null>(null)
  const seen = useRef(new Set<string>())
  const [flight, setFlight] = useState<string | null>(null)
  useEffect(() => { seen.current.clear(); setReleased(null); setFlight(null) }, [scope])
  useEffect(() => {
    if (!id || !ready || seen.current.has(id)) return
    const reduce = reducedMotion()
    const clipEnd = Number(document.querySelector<HTMLElement>('[data-attempt-spectrum]')?.dataset.clipFlightUntil ?? 0)
    const delay = Math.max(450, clipEnd - performance.now() + 450)
    const timer = setTimeout(() => {
      // Publish and start both word flights in the same React commit.
      seen.current.add(id)
      setReleased(id)
      if (!reduce) setFlight(id)
    }, reduce ? 0 : delay)
    return () => clearTimeout(timer)
  }, [id, ready, scope])
  useLayoutEffect(() => {
    if (!flight) return
    const copies: HTMLElement[] = []
    const animations: Animation[] = []
    {
      const source = document.querySelector('[data-attempt-spectrum]')
      const destination = [...document.querySelectorAll<HTMLElement>('[data-history-id]')]
        .find(element => element.dataset.historyId === flight && element.getBoundingClientRect().width > 0)
        ?? document.querySelector<HTMLElement>('.drill-history-entry[data-expanded]')
        ?? document.querySelector<HTMLElement>('.drill-history-preview-rows')
      if (!source || !destination) return
      const to = (destination.querySelector('.drill-word-pairs') ?? destination).getBoundingClientRect()
      const words = [...source.querySelectorAll<HTMLElement>('.timed-word bdi')]
      words.forEach((word, index) => {
        const from = word.getBoundingClientRect()
        if (!from.width || !word.animate) return
        const copy = document.createElement('span')
        copy.textContent = word.textContent
        copy.className = 'drill-word-flight'
        copy.setAttribute('aria-hidden', 'true')
        Object.assign(copy.style, { left: `${from.left}px`, top: `${from.top}px`, color: getComputedStyle(word).color })
        document.body.append(copy); copies.push(copy)
        const x = to.left + (index + 0.5) / Math.max(1, words.length) * to.width - from.left
        const animation = copy.animate([
          { transform: 'none', opacity: 1 },
          { transform: `translate(${x}px, ${to.top - from.top}px)`, opacity: 1, offset: 0.85 },
          { transform: `translate(${x}px, ${to.top - from.top}px)`, opacity: 0 },
        ], { duration: 800, easing: 'ease-in-out' })
        if (document.timeline?.currentTime != null) animation.startTime = document.timeline.currentTime
        animation.onfinish = () => copy.remove()
        animations.push(animation)
      })
    }
    return () => { animations.forEach(animation => animation.cancel()); copies.forEach(copy => copy.remove()) }
  }, [flight])
  return id && released !== id && !seen.current.has(id) ? id : null
}
