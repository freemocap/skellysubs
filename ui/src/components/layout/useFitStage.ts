import { useLayoutEffect, useRef, useState, type RefObject } from 'react'

/// The fullest layout a one-row bar can show in the room it has.
///
/// A bar lists its layouts from fullest to most compact. Each is a value of
/// the bar's `data-fit` attribute, which the bar's stylesheet reads to drop or
/// shorten parts. The hook tries them in that order and keeps the first under
/// which `overflows` finds nothing cut off; when none fits, the last. Every try
/// happens synchronously before paint. Initial fitting happens during mounting;
/// subsequent notifications share one animation-frame pass.
///
/// The room is the bar's own, not the window's: a side panel opening or
/// closing changes it as much as a resize does. The bar fits again when it
/// changes size, when its content changes (a counter gains a digit, a name
/// changes, the interface language changes), when the document's appearance
/// changes its spacing or type, and when a font arrives.
export function useFitStage<Stage extends string>(bar: RefObject<HTMLElement | null>, stages: readonly [Stage, ...Stage[]], overflows: (bar: HTMLElement) => boolean): Stage {
  const [stage, setStage] = useState<Stage>(stages[0])
  const latest = useRef({ stages, overflows })
  latest.current = { stages, overflows }
  useLayoutEffect(() => {
    const element = bar.current
    if (!element) return
    const fit = () => {
      const { stages, overflows } = latest.current
      let chosen = stages[stages.length - 1]
      for (const candidate of stages) {
        if (element.dataset.fit !== candidate) element.dataset.fit = candidate
        if (!overflows(element)) { chosen = candidate; break }
      }
      if (element.dataset.fit !== chosen) element.dataset.fit = chosen
      setStage(chosen)
    }
    fit()
    let frame: number | null = null
    const schedule = () => {
      frame ??= requestAnimationFrame(() => { frame = null; fit() })
    }
    // Trying a layout only changes the bar's data-fit attribute, which neither
    // observer watches, so fitting never triggers itself.
    const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule)
    resize?.observe(element)
    const content = new MutationObserver(schedule)
    content.observe(element, { subtree: true, childList: true, characterData: true })
    content.observe(document.documentElement, { attributes: true })
    document.fonts?.addEventListener('loadingdone', schedule)
    return () => {
      resize?.disconnect()
      content.disconnect()
      if (frame !== null) cancelAnimationFrame(frame)
      document.fonts?.removeEventListener('loadingdone', schedule)
    }
  }, [bar])
  return stage
}

/// Whether an element's content is wider than its box: a truncated label, or a
/// row whose parts no longer fit side by side. One pixel absorbs rounding.
export function cutOff(element: Element | null): boolean {
  return !!element && element.scrollWidth > element.clientWidth + 1
}
