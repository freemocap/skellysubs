import { useLayoutEffect, useRef } from 'react'

/** Expanding above a toolbar keeps its toggle under the pointer. Stop anchoring
 * as soon as the learner scrolls; never fight intentional navigation. */
export function useMessageToolAnchor(state: string) {
  const anchor = useRef<{ button: HTMLElement; top: number } | null>(null)
  useLayoutEffect(() => {
    const saved = anchor.current
    if (!saved) return
    const bubble = saved.button.closest('.msg')
    if (!bubble) return
    let scroller = bubble.parentElement
    while (scroller && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) scroller = scroller.parentElement
    if (!scroller) return
    const scroll = scroller
    let active = true
    const adjust = () => {
      if (!active || !saved.button.isConnected) return
      const delta = saved.button.getBoundingClientRect().top - saved.top
      if (Math.abs(delta) > 0.5) scroll.scrollTop += delta
    }
    const stop = () => { active = false; anchor.current = null }
    adjust()
    const observer = new ResizeObserver(adjust)
    observer.observe(bubble)
    scroll.addEventListener('pointerdown', stop, { passive: true })
    scroll.addEventListener('wheel', stop, { passive: true })
    scroll.addEventListener('touchstart', stop, { passive: true })
    scroll.addEventListener('keydown', stop)
    return () => {
      observer.disconnect()
      scroll.removeEventListener('pointerdown', stop)
      scroll.removeEventListener('wheel', stop)
      scroll.removeEventListener('touchstart', stop)
      scroll.removeEventListener('keydown', stop)
    }
  }, [state])
  return (button: HTMLElement) => { anchor.current = { button, top: button.getBoundingClientRect().top } }
}
