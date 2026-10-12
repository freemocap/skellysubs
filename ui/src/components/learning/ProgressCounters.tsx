import { ToolbarIcon, type ToolbarIconName } from '../controls/ToolbarIcon'
import { useEffect, useRef, useState } from 'react'
import type { EffortProgress, StreakSnapshot } from '../../generated/contracts'
import { StreakCount } from './StreakCount'
import { useI18n } from '../localization/i18n'

const GAIN_MS = 1600

/** Shared XP face for the profile button and conversation badge. The shell adds
 * streaks below XP; effort units remain in the progress card. */
export function ProgressCounters({ xp, xpLabel = 'XP', scope, effort, effects = true, error, icon = 'star', code, global, streaks }: {
  xp: number | null; xpLabel?: string; scope?: string; effort: EffortProgress | null; effects?: boolean; error?: string | null
  /** Marks the XP number; replaced by `code` when a language code is given. */
  icon?: ToolbarIconName; code?: string
  /** All languages' XP, shown after the language's own number when given. */
  global?: number | null
  streaks?: StreakSnapshot | null
}) {
  const tr = useI18n()
  const identity = `${scope ?? ''}|${effort?.target ?? ''}`
  const xpGain = useGain(xp, identity, effects)
  const formatted = xp === null ? '—' : compact(tr, xp)
  const full = xp === null ? tr('Loading…') : tr.number(xp)
  const globalFull = global == null ? tr('Loading…') : tr.number(global)
  return <span className="progress-counters" data-streaks={streaks !== undefined || undefined}>
    <span className="progress-counter progress-counter-xp" data-gaining={xpGain > 0 || undefined} title={`${tr(xpLabel)}: ${full}`} aria-label={`${tr(xpLabel)}: ${full}`}>
      {code ? <span className="language-code" aria-hidden="true">{code}</span> : <span className="progress-counter-mark" data-icon={icon} aria-hidden="true"><ToolbarIcon name={icon} size={14} /></span>}
      <strong key={`${identity}:${xp}`} className="progress-counter-number" aria-hidden="true">{formatted}</strong>
      {' '}<span className="progress-counter-icon" aria-hidden="true">XP</span>
      {xpGain > 0 && <span key={xp} className="progress-counter-gain" aria-hidden="true">+{compact(tr, xpGain)}</span>}
    </span>
    {global !== undefined && <span className="progress-counter progress-counter-global" title={`${tr('Total XP')}: ${globalFull}`} aria-label={`${tr('Total XP')}: ${globalFull}`}>
      <span className="progress-counter-mark" aria-hidden="true"><ToolbarIcon name="globe" size={14} /></span>
      <strong className="progress-counter-number" aria-hidden="true">{global === null ? '—' : compact(tr, global)}</strong>
    </span>}
    {streaks !== undefined && <StreakCount key={streaks?.sessionId ?? 'loading-app'} value={streaks?.app} effects={effects} />}
    {error && <span className="progress-counter-error" role="status" title={error} aria-label={error}>!</span>}
  </span>
}

/** The increase since the last value for the same identity, shown briefly. Initial
 * reads and language or conversation switches never count as gains. */
function useGain(value: number | null, identity: string, effects: boolean) {
  const previous = useRef({ value, identity })
  const [gain, setGain] = useState(0)
  useEffect(() => {
    const before = previous.current
    previous.current = { value, identity }
    setGain(effects && before.identity === identity && before.value !== null && value !== null ? Math.max(0, value - before.value) : 0)
    const timeout = setTimeout(() => setGain(0), GAIN_MS)
    return () => clearTimeout(timeout)
  }, [value, identity, effects])
  return gain
}

function compact(tr: ReturnType<typeof useI18n>, value: number) {
  return tr.number(value, value >= 10000 ? { notation: 'compact', maximumFractionDigits: 1 } : undefined)
}
