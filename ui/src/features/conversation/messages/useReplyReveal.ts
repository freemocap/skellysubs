import { useEffect, useRef, useState } from 'react'

import { useReducedMotion } from '../../../platform/appearance/motion'

const TICK_MS = 40
const MAX_TICKS = 20
const words = new Intl.Segmenter(undefined, { granularity: 'word' })

/** Presentation only: preserve exact source text and reveal at word boundaries.
 * Small deltas appear immediately; bursts catch up within 800 ms. Never delay
 * publication, speech, controls or diagnostics. Existing history starts complete. */
export function useReplyReveal(text: string, animate: boolean) {
  const reduced = useReducedMotion()
  const [shown, setShown] = useState(text)
  const previous = useRef(text)
  const target = useRef(text)
  target.current = text
  // Replacements and terminal errors must never retain another source's prefix.
  const compatible = text.startsWith(shown)
  let visible = !animate || reduced || !compatible ? text : shown
  // A delta can extend a previously displayed word or combining sequence.
  // Keep that whole segment together rather than splitting its shaping spans.
  if (visible.length && visible.length < text.length) {
    for (const part of words.segment(text)) {
      const end = part.index + part.segment.length
      if (part.index < visible.length && end > visible.length) { visible = text.slice(0, end); break }
      if (part.index >= visible.length) break
    }
  }
  useEffect(() => {
    if (!animate || reduced || !text.startsWith(previous.current)) {
      previous.current = text
      setShown(text)
      return
    }
    previous.current = text
    if (text === shown) return
    // One timer owns the backlog, including deltas received while catching up.
    let ticks = 0
    const reveal = () => setShown(current => {
      const next = target.current
      if (!next.startsWith(current)) return next
      const remaining = [...words.segment(next.slice(current.length))]
      let count = Math.max(1, Math.ceil(remaining.filter(part => part.isWordLike).length / Math.max(1, MAX_TICKS - ticks)))
      let end = current.length
      for (const part of remaining) {
        end += part.segment.length
        if (part.isWordLike && --count <= 0) break
      }
      return next.slice(0, end)
    })
    reveal()
    const timer = setInterval(() => {
      ticks++
      reveal()
      if (ticks >= MAX_TICKS) clearInterval(timer)
    }, TICK_MS)
    return () => clearInterval(timer)
  }, [text, animate, reduced])
  return { text: visible, revealing: visible.length < text.length }
}
