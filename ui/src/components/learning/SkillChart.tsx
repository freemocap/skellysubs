import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import { ToolbarIcon } from '../controls/ToolbarIcon'
import { useI18n } from '../localization/i18n'
import { useStoredChoice } from '../persistence/useStoredChoice'
import { skillColors, skillShortLabel } from '../../domain/learning/catalog/skill-domains'
import type { LanguageSkillLevels } from '../../domain/learning/statistics/skill-levels'
import { SkillRadar } from './SkillRadar'
import { goalTag, markDetail, markName } from './skill-chart-marks'
import { CHART_LIMIT, CHART_SHOWN, MAX_ZOOM, MIN_ZOOM, skillChartExtent, zoomExtent, type SkillChartExtent, type SkillChartMarks, type SkillChartScale, type SkillChartShown, type SkillChartType } from './skill-chart-scale'

const CHART_TYPES: readonly SkillChartType[] = ['radial', 'bars']
const CHART_SCALES: readonly SkillChartScale[] = ['normalized', 'scale']
/** Each zoom button press multiplies or divides the zoom by this. */
const ZOOM_STEP = 1.5

const clampZoom = (zoom: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom))

export interface SkillChartView {
  type: SkillChartType
  scale: SkillChartScale
  /** Whose points fill a chart that has a conversation; see `shownOn`. */
  shown: SkillChartShown
  setType: (type: SkillChartType) => void
  setScale: (scale: SkillChartScale) => void
  setShown: (shown: SkillChartShown) => void
}

/** The learner's chart type, scale and points shown, shared by every skill chart
 * in this browser profile. Radial, normalized and both until changed. */
export function useSkillChartView(): SkillChartView {
  const [type, setType] = useStoredChoice('skellyspeak_skill_chart_type', CHART_TYPES)
  const [scale, setScale] = useStoredChoice('skellyspeak_skill_chart_scale', CHART_SCALES)
  const [shown, setShown] = useStoredChoice('skellyspeak_skill_chart_shown', CHART_SHOWN)
  return { type: type ?? 'radial', scale: scale ?? 'normalized', shown: shown ?? 'both', setType, setScale, setShown }
}

/** Whose points a chart draws: the view's choice where it has a conversation, else the language. */
export const shownOn = (view: SkillChartView, conversation: number[] | null): SkillChartShown => conversation ? view.shown : 'language'

/** The language's skills as a radar or as bars in one framed card: a row of
 * controls across the top, the chart in a fixed square under it, and
 * `children` (the key and notes) along the bottom. The first row holds the
 * chart type, scale and zoom. A second row chooses whose points are shown
 * (only with a conversation: language, conversation or both). When the
 * card is too narrow for them, type, scale and zoom fold into a settings
 * button and its menu (closed by a press outside or Escape), and the row
 * unfolds again when the card grows; without a conversation the button
 * floats over the chart's corner instead of keeping a row. While zoomed, the
 * button shows the zoom. Switching type never moves the rest of the page; the
 * new chart fades in. Switching scale or zoom keeps the chart and glides every
 * mark to its new length; switching scale returns to fit. Zoom magnifies short
 * arms in either scale and chart type; marks past the edge are clipped there.
 * `conversation` is one conversation's points per skill, or null where the
 * chart has none. */
export function SkillChart({ levels, conversation, view, selected, onSelect, children }: {
  levels: LanguageSkillLevels
  conversation: number[] | null
  view: SkillChartView
  selected: string; onSelect: (id: string) => void
  children: ReactNode
}) {
  const tr = useI18n()
  const [zoom, setZoom] = useState(MIN_ZOOM)
  useEffect(() => setZoom(MIN_ZOOM), [view.scale])
  const shown = shownOn(view, conversation)
  const extent = zoomExtent(skillChartExtent(levels, conversation, view.scale, shown), zoom)
  const marks: SkillChartMarks = shown === 'conversation' && conversation
    ? { of: 'conversation', points: conversation, busiest: view.scale === 'normalized' ? Math.max(...conversation) : null }
    : { of: 'language' }
  const zoomText = tr('{value0}×', { value0: tr.number(zoom, { maximumFractionDigits: 1 }) })
  const card = useRef<HTMLElement>(null)
  const row = useRef<HTMLDivElement>(null)
  const folded = useRowFit(card, row, [tr, conversation !== null])
  const [menuOpen, setMenuOpen] = useState(false)
  const settings = useRef<HTMLDivElement>(null)
  const menuId = useId()
  useEffect(() => { if (!folded) setMenuOpen(false) }, [folded])
  useEffect(() => {
    if (!menuOpen) return
    const outside = (event: PointerEvent) => { if (!settings.current?.contains(event.target as Node)) setMenuOpen(false) }
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') setMenuOpen(false) }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
    return () => { document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', escape) }
  }, [menuOpen])
  const typeSwitch = <Switch label={tr('Chart type')} value={view.type} onChange={view.setType}
    options={[{ value: 'radial', label: tr('Radial') }, { value: 'bars', label: tr('Bars') }]} />
  const scaleSwitch = <Switch label={tr('Chart scale')} value={view.scale} onChange={view.setScale}
    options={[{ value: 'normalized', label: tr('Normalized') }, { value: 'scale', label: tr('To scale') }]} />
  const zoomGroup = <div className="skill-chart-switch skill-chart-zoom" role="group" aria-label={tr('Zoom')}>
    <button type="button" aria-label={tr('Zoom out')} title={tr('Zoom out')} disabled={zoom <= MIN_ZOOM} onClick={() => setZoom(current => clampZoom(current / ZOOM_STEP))}>−</button>
    <button type="button" className="skill-chart-zoom-value" aria-label={tr('Fit the whole chart')} title={tr('Fit the whole chart')} disabled={zoom === MIN_ZOOM} onClick={() => setZoom(MIN_ZOOM)}>{zoomText}</button>
    <button type="button" aria-label={tr('Zoom in')} title={tr('Zoom in')} disabled={zoom >= MAX_ZOOM} onClick={() => setZoom(current => clampZoom(current * ZOOM_STEP))}>+</button>
  </div>
  return <figure className="skill-chart" data-scale={view.scale} ref={card}>
    <div className="skill-chart-bar" role="group" aria-label={tr('Chart controls')} ref={row} data-floating={folded && !conversation ? 'true' : undefined}>
      {folded
        ? <div className="skill-chart-settings" ref={settings}>
          <button type="button" className="skill-chart-menu-button" aria-label={tr('Chart settings')} title={tr('Chart settings')}
            aria-expanded={menuOpen} aria-controls={menuOpen ? menuId : undefined} onClick={() => setMenuOpen(open => !open)}>
            <ToolbarIcon name="settings" size={14} />{zoom > MIN_ZOOM && <span>{zoomText}</span>}
          </button>
          {menuOpen && <div className="skill-chart-menu" id={menuId} role="group" aria-label={tr('Chart settings')}>
            <span>{tr('Chart type')}</span>{typeSwitch}
            <span>{tr('Chart scale')}</span>{scaleSwitch}
            <span>{tr('Zoom')}</span>{zoomGroup}
          </div>}
        </div>
        : <>{typeSwitch}{scaleSwitch}{zoomGroup}</>}
    </div>
    {conversation && <div className="skill-chart-scope">
      <Switch label={tr('Points shown')} value={view.shown} onChange={view.setShown}
        options={[{ value: 'language', label: tr('Language') }, { value: 'conversation', label: tr('Conversation') }, { value: 'both', label: tr('Both') }]} />
    </div>}
    {/* Keyed by chart type, so a change of type replays the chart's entrance. */}
    <div className="skill-chart-stage" key={view.type}>
      {view.type === 'radial'
        ? <SkillRadar levels={levels} extent={extent} marks={marks} selected={selected} onSelect={onSelect} />
        : <SkillBars levels={levels} extent={extent} marks={marks} selected={selected} onSelect={onSelect} />}
    </div>
    <figcaption className="skill-chart-notes">{children}</figcaption>
  </figure>
}

/** Whether the control row must fold: true while its children, side by side
 * with the row's gaps and padding, would be wider than the card. The width
 * they need is measured while the row is unfolded and remembered, so the row
 * unfolds again once the card is wide enough for it. Re-measured when the card
 * resizes, when fonts arrive and when `deps` change. */
function useRowFit(card: RefObject<HTMLElement | null>, row: RefObject<HTMLDivElement | null>, deps: unknown[]): boolean {
  const [folded, setFolded] = useState(false)
  const isFolded = useRef(false)
  const needed = useRef<number | null>(null)
  useLayoutEffect(() => {
    const frame = card.current, line = row.current
    if (!frame || !line) return
    const update = () => {
      const available = frame.clientWidth
      if (!isFolded.current) {
        const style = getComputedStyle(line)
        const gap = Number.parseFloat(style.columnGap) || 0
        const children = [...line.children] as HTMLElement[]
        const content = children.reduce((sum, child) => sum + child.getBoundingClientRect().width, 0) + gap * Math.max(0, children.length - 1)
        needed.current = content + (Number.parseFloat(style.paddingInlineStart) || 0) + (Number.parseFloat(style.paddingInlineEnd) || 0)
        if (needed.current > available) { isFolded.current = true; setFolded(true) }
      } else if (needed.current !== null && needed.current <= available) { isFolded.current = false; setFolded(false) }
    }
    update()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(update)
    observer.observe(frame)
    let mounted = true
    void document.fonts?.ready.then(() => { if (mounted) update() })
    return () => { mounted = false; observer.disconnect() }
  // The callers' deps name what changes the row's content.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
  return folded
}

/** A small segmented switch: one pressed option at a time. */
function Switch<T extends string>({ label, value, options, onChange }: {
  label: string; value: T; options: { value: T; label: string }[]; onChange: (value: T) => void
}) {
  return <div className="skill-chart-switch" role="group" aria-label={label}>
    {options.map(option => <button type="button" key={option.value} aria-pressed={value === option.value}
      onClick={() => onChange(option.value)}>{option.label}</button>)}
  </div>
}

/** One bar per skill in catalog order, each in its own colour, measured
 * against the earned-level lines and the gold goal line. Hovering a line shows
 * its level and points (the goal's tag above the bars, the others below). A bar longer than the track
 * stops at its end, squared off; lines past the end are not drawn. Pressing a bar selects its skill. Bars and lines glide to new lengths when the extent changes.
 * `marks` says whose points the bars hold, which names them and tags the goal line. */
export function SkillBars({ levels, extent, marks = { of: 'language' }, selected, onSelect }: {
  levels: LanguageSkillLevels
  extent: SkillChartExtent
  marks?: SkillChartMarks
  selected: string; onSelect: (id: string) => void
}) {
  const tr = useI18n()
  const along = (length: number) => `${length / CHART_LIMIT * 100}%`
  return <div className="skill-bars" style={{ gridTemplateRows: `var(--space-9) repeat(${levels.skills.length}, minmax(0, 1fr)) var(--space-9)` }}>
    {levels.skills.map((skill, index) => {
      const colours = skillColors(skill.id)
      const conversation = extent.conversation?.[index] ?? null
      return <button type="button" key={skill.id} className="skill-bar" data-reward-skill={skill.id} aria-pressed={selected === skill.id}
        aria-label={markName(tr, marks, skill, index)}
        style={{ gridRow: index + 2, color: colours.mark }} onClick={() => onSelect(skill.id)}>
        <span className="skill-bar-name">
          <strong style={{ color: colours.ink }}>{tr(skillShortLabel(skill.id))}</strong>
          <span>{markDetail(tr, marks, skill, index)}</span>
        </span>
        <span className="skill-bar-track" aria-hidden="true">
          <span className="skill-bar-fill" data-over={extent.skills[index] > CHART_LIMIT ? 'true' : undefined} style={{ width: along(Math.min(extent.skills[index], CHART_LIMIT)) }} />
          {conversation !== null && conversation > 0 && <span className="skill-bar-conversation" style={{ width: along(Math.min(conversation, CHART_LIMIT)) }} />}
        </span>
      </button>
    })}
    <div className="skill-bars-lines" aria-hidden="true">
      {extent.rings.map((length, index) => length <= CHART_LIMIT && <span key={index} className="skill-bars-ring" data-current={index === extent.rings.length - 1 ? 'true' : undefined} style={{ insetInlineStart: along(length) }}>
        <span className="skill-bars-tag">{tr('Lv {value0} · {value1} pt', { value0: index + 1, value1: tr.number(levels.bands[index]) })}</span>
      </span>)}
      {extent.goal <= CHART_LIMIT && <span className="skill-bars-ring" data-goal="true" style={{ insetInlineStart: along(extent.goal) }}>
        <span className="skill-bars-tag">{goalTag(tr, marks, levels)}</span>
      </span>}
    </div>
  </div>
}
