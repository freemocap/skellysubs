import { ToolbarIcon, type ToolbarIconName } from '../controls/ToolbarIcon'
import { ErrorNotice } from '../feedback/ErrorNotice'
import { effortDimensions, type EffortField } from './effort-dimensions'
import type { EffortProgress } from '../../generated/contracts'
import { useI18n } from '../localization/i18n'
import type { ReactNode } from 'react'

/** The card a progress button shows first: XP plus the owner's effort units, each
 * in the colour of the place it comes from, optional owner content, and the way
 * on to the full view. The owner positions it and supplies the data. */
export function ProgressCard({ title, xp, effort, units, error, expandLabel, onExpand, children, headerAccessory, icon = 'star', tone = 'progress' }: {
  title: string; xp: number | null; effort: EffortProgress | null; units?: readonly EffortField[]; error?: string | null
  expandLabel: string; onExpand: () => void; children?: ReactNode; headerAccessory?: ReactNode
  /** The XP mark and colour family: progress gold for the learner overall, coach green for a conversation. */
  icon?: ToolbarIconName; tone?: 'progress' | 'coach'
}) {
  const tr = useI18n()
  const shown = effortDimensions.filter(({ field }) => !units || units.includes(field))
  return <section className="progress-card" data-tone={tone} role="dialog" aria-label={title}>
    <header className="progress-card-heading">
      <strong className="progress-card-xp" data-icon={icon}><ToolbarIcon name={icon} size={16} />{xp === null ? '—' : tr.number(xp)}<span>{tr(' XP')}</span></strong>
      {headerAccessory}
      <span className="progress-card-title">{title}</span>
    </header>
    {shown.length > 0 && <ul className="progress-card-units">{shown.map(({ field, icon, label }) => {
      const value = effort ? tr.number(effort[field]) : '—'
      return <li key={field} data-unit={field} aria-label={`${tr(label)}: ${value}`} title={tr(label)}>
        <ToolbarIcon name={icon} size={15} />
        <strong aria-hidden="true">{value}</strong>
        <span aria-hidden="true">{tr(label)}</span>
      </li>
    })}</ul>}
    {children}
    {error && <ErrorNotice error={error}>{error}</ErrorNotice>}
    <button type="button" className="progress-card-expand" onClick={onExpand}>{tr(expandLabel)}<ToolbarIcon name="chevron" size={14} /></button>
  </section>
}
