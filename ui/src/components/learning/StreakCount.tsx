import { useEffect, useRef, useState, type CSSProperties } from 'react'
import type { StreakCounter } from '../../generated/contracts'
import { useI18n } from '../localization/i18n'

const DIRECTIONS = [[-1, -1], [-0.4, -1.5], [0.4, -1.4], [1, -0.8], [-0.8, 0.4], [0.8, 0.5]]

/** Counts are always visible. Only a newly claimed day may start a silent effect. */
export function StreakCount({ value, effects = true }: { value?: StreakCounter; effects?: boolean }) {
  const tr = useI18n()
  const count = Math.max(1, value?.count ?? 1)
  const token = value?.celebration ?? null
  const seen = useRef<string | null>(null)
  const timer = useRef<number | undefined>(undefined)
  const [celebrating, setCelebrating] = useState<string | null>(null)
  useEffect(() => () => window.clearTimeout(timer.current), [])
  useEffect(() => {
    if (!effects) { seen.current = token; window.clearTimeout(timer.current); setCelebrating(null); return }
    if (!token || seen.current === token) return
    seen.current = token
    setCelebrating(token)
    window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => setCelebrating(null), 1600)
  }, [token, effects])
  const active = effects && celebrating !== null
  const label = tr('App streak: {value0}', { value0: count })
  return <span className="streak-count" data-celebrating={active || undefined} aria-label={label}>
    <span className="streak-flame" aria-hidden="true">🔥</span>
    <span className="streak-number" aria-hidden="true">
      {active && count > 1 ? <span key={celebrating} className="streak-roll"><span>{tr.number(count - 1)}</span><span>{tr.number(count)}</span></span> : tr.number(count)}
    </span>
    {active && <span key={celebrating} className="streak-confetti" aria-hidden="true">{DIRECTIONS.map(([x, y], index) => <i key={index} style={{ '--burst-x': x, '--burst-y': y } as CSSProperties} />)}</span>}
  </span>
}
