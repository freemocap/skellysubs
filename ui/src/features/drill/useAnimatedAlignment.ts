import { reducedMotion } from '../../platform/appearance/motion'
import { useEffect, useMemo, useRef, useState } from 'react'

/** Interpolate normalized positions so changing durations never causes a jump. */
export function useAnimatedAlignment(alignment: ((seconds: number) => number) | null, takeDuration: number, referenceDuration: number, recordingId: string | undefined) {
  const target = useMemo(() => alignment
    ? (fraction: number) => alignment(fraction * takeDuration) / referenceDuration
    : (fraction: number) => fraction, [alignment, takeDuration, referenceDuration])
  const shown = useRef(target)
  const identity = useRef(recordingId)
  const [mapping, setMapping] = useState(() => target)
  useEffect(() => {
    const from = identity.current === recordingId ? shown.current : (fraction: number) => fraction
    identity.current = recordingId
    if (typeof requestAnimationFrame !== 'function' || reducedMotion()) {
      shown.current = target; setMapping(() => target); return
    }
    let frame = 0
    let start: number | null = null
    const tick = (now: number) => {
      start ??= now
      const progress = Math.min(1, Math.max(0, (now - start) / 350))
      const eased = 1 - (1 - progress) ** 3
      const next = progress === 1 ? target : (fraction: number) => from(fraction) * (1 - eased) + target(fraction) * eased
      shown.current = next; setMapping(() => next)
      if (progress < 1) frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
  }, [target, recordingId])
  return useMemo(() => (seconds: number) => mapping(seconds / takeDuration) * (alignment ? referenceDuration : takeDuration),
    [mapping, takeDuration, referenceDuration, alignment])
}
