// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { SkillLevelsPanel } from './SkillLevelsPanel'
import { skillDemo } from '../../domain/learning/catalog/skillDemo'
import type { ConversationSkillPoints } from '../../domain/learning/statistics/skill-levels'
import { withFixtureLevels } from '../../../tests/fixtures/skill-levels'

function snapshotWithPoints(points: number[]) {
  const snapshot = structuredClone(skillDemo)
  snapshot.catalog.filter(node => node.kind === 'skill').forEach((skill, index) => {
    for (let n = 0; n < points[index]; n++) snapshot.profile.credits.push({ attempt_id: `${skill.id}-${n}`, skill_id: skill.id, xp: 1 })
  })
  return withFixtureLevels(snapshot)
}

/** One conversation's points per skill, catalog order. */
function conversationWith(points: number[]): ConversationSkillPoints {
  const ids = skillDemo.catalog.filter(node => node.kind === 'skill').map(node => node.id)
  return { skills: ids.map((id, index) => ({ id, points: points[index] })), total: points.reduce((sum, value) => sum + value, 0) }
}

const focusHeading = () => document.querySelector('.skill-levels-focus h3')?.textContent

describe('SkillLevelsPanel', () => {
  it('names every arm on the chart and shows a pressed arm right under it', () => {
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    expect(screen.getByText('Spanish skill level 4')).toBeTruthy()
    expect(document.querySelectorAll('.skill-radar-label')).toHaveLength(8)
    expect(screen.queryByText(/Lv 5 goal/)).toBeNull()
    const goal = document.querySelectorAll('.skill-radar-ring-hit')
    fireEvent.pointerMove(goal[goal.length - 1], { clientX: 10, clientY: 10 })
    expect(screen.getByText(/^Lv 5 goal · \d+ pt$/)).toBeTruthy()
    expect(focusHeading()).toBe('Managing conversation')
    fireEvent.click(screen.getByRole('button', { name: /^Possibilities and constraints: skill level 4/ }))
    expect(focusHeading()).toBe('Possibilities and constraints')
    expect(screen.getByText('3 more points for level 5')).toBeTruthy()
  })
  it('lists every skill holding the next level back and every skill already ready', () => {
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    const next = screen.getByRole('heading', { name: 'To reach level 5' }).parentElement!
    expect(within(next).getAllByRole('button')).toHaveLength(4)
    const ready = screen.getByRole('heading', { name: 'Ready for level 5' }).parentElement!
    expect(within(ready).getAllByRole('button')).toHaveLength(4)
    fireEvent.click(within(next).getByRole('button', { name: /Feelings and viewpoints/ }))
    expect(focusHeading()).toBe('Feelings and viewpoints')
  })
  it('opens more about the focused skill when the owner allows it', () => {
    const inspect = vi.fn()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([1, 1, 1, 1, 1, 1, 1, 1])} conversation={null} onInspect={inspect} />)
    const next = screen.getByRole('heading', { name: 'To reach level 2' }).parentElement!
    fireEvent.click(within(next).getByRole('button', { name: /Possibilities and constraints/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Possibilities and constraints in Spanish' }))
    expect(inspect).toHaveBeenCalledWith('possibilities_constraints')
  })
  it('tells a new learner how to reach level 1', () => {
    render(<SkillLevelsPanel languageName="Spanish" snapshot={skillDemo} conversation={null} onInspect={null} />)
    expect(screen.getByText('Get a skill point in each skill to reach level 1.')).toBeTruthy()
  })
  it('keeps one selected skill that hover never changes, marked on the chart, the lists and the card', () => {
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    const start = focusHeading()
    const ready = screen.getByRole('heading', { name: 'Ready for level 5' }).parentElement!
    const chip = within(ready).getAllByRole('button')[0]
    fireEvent.pointerEnter(chip)
    expect(focusHeading()).toBe(start)
    fireEvent.click(chip)
    const chosen = focusHeading()
    expect(chosen).not.toBe(start)
    expect(chip).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: new RegExp(`^${chosen}: skill level`) })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(chip)
    expect(focusHeading()).toBe(chosen)
  })
  it('shows the chart type, scale and zoom controls in a bar above the chart, with no menu to open', () => {
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    expect(screen.queryByRole('button', { name: 'Chart settings' })).toBeNull()
    const bar = screen.getByRole('group', { name: 'Chart controls' })
    expect(within(bar).getByRole('group', { name: 'Chart type' })).toContainElement(screen.getByRole('button', { name: 'Bars' }))
    expect(within(bar).getByRole('group', { name: 'Chart scale' })).toContainElement(screen.getByRole('button', { name: 'To scale' }))
    expect(within(bar).getByRole('group', { name: 'Zoom' })).toContainElement(screen.getByRole('button', { name: 'Zoom in' }))
    // The glyph-only zoom buttons explain themselves on hover.
    expect(screen.getByRole('button', { name: 'Zoom in' })).toHaveAccessibleDescription('Zoom in')
    // The bar reads before the chart it controls.
    const stage = document.querySelector('.skill-chart-stage')!
    expect(bar.compareDocumentPosition(stage) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })
  it('fits chart controls independently of the second scope row, folding only when chart controls cannot fit', () => {
    localStorage.clear()
    let width = 400
    const callbacks = new Set<() => void>()
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback: () => void) { callbacks.add(callback) }
      observe() {} unobserve() {} disconnect() {}
    })
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (this: HTMLElement) { return width })
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      return { width: this.classList.contains('skill-chart-switch') ? 120 : 0 } as DOMRect
    })
    try {
      render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={conversationWith([1, 0, 2, 1, 1, 0, 2, 0])} onInspect={null} />)
      // Three chart switches fit at 400px, even though all four would need 480px.
      const bar = screen.getByRole('group', { name: 'Chart controls' })
      const scope = screen.getByRole('group', { name: 'Points shown' })
      expect(bar).not.toContainElement(scope)
      expect(bar.compareDocumentPosition(scope) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
      expect(within(bar).getByRole('button', { name: 'Bars' })).toBeTruthy()
      expect(screen.queryByRole('button', { name: 'Chart settings' })).toBeNull()
      width = 300
      act(() => { for (const callback of callbacks) callback() })
      expect(scope).toBeVisible()
      expect(screen.queryByRole('button', { name: 'Bars' })).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'Chart settings' }))
      const menu = screen.getByRole('group', { name: 'Chart settings' })
      fireEvent.click(within(menu).getByRole('button', { name: 'Bars' }))
      expect(document.querySelectorAll('.skill-bar')).toHaveLength(8)
      fireEvent.keyDown(document, { key: 'Escape' })
      expect(screen.queryByRole('group', { name: 'Chart settings' })).toBeNull()
      // With room again, the controls return to the row and the button goes.
      width = 1000
      act(() => { for (const callback of callbacks) callback() })
      expect(within(bar).getByRole('button', { name: 'Bars' })).toHaveAttribute('aria-pressed', 'true')
      expect(screen.queryByRole('button', { name: 'Chart settings' })).toBeNull()
    } finally {
      vi.restoreAllMocks()
      vi.unstubAllGlobals()
      localStorage.clear()
    }
  })
  it('shows the language totals, this conversation alone, or both, and remembers the choice', () => {
    localStorage.clear()
    const conversation = conversationWith([1, 0, 2, 1, 1, 0, 2, 0])
    const { unmount } = render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={conversation} onInspect={null} />)
    const shown = screen.getByRole('group', { name: 'Points shown' })
    // Both, until changed: the totals fill the shape and the conversation is an outline over them.
    expect(within(shown).getByRole('button', { name: 'Both' })).toHaveAttribute('aria-pressed', 'true')
    expect(document.querySelector('.skill-radar-conversation')).not.toBeNull()
    expect(screen.getByText('Your points')).toBeTruthy()
    expect(screen.getByText(/^This conversation: 7 skill points/)).toBeTruthy()
    fireEvent.click(within(shown).getByRole('button', { name: 'Language' }))
    expect(document.querySelector('.skill-radar-conversation')).toBeNull()
    expect(screen.queryByText(/^This conversation: 7 skill points/)).toBeNull()
    expect(screen.getByRole('button', { name: /^Information exchange: skill level 5, 11 points/ })).toBeTruthy()
    fireEvent.click(within(shown).getByRole('button', { name: 'Conversation' }))
    // Alone, the conversation is the shape: its points name each arm, no earned rings, busiest skill on the gold ring.
    expect(document.querySelector('.skill-radar-conversation')).toBeNull()
    expect(document.querySelectorAll('.skill-radar-earned')).toHaveLength(0)
    expect(screen.getByRole('button', { name: 'Information exchange: 2 points in this conversation' })).toHaveTextContent('2 pt')
    expect(screen.getByText('This conversation')).toBeTruthy()
    expect(screen.getByText('Busiest skill in this conversation')).toBeTruthy()
    expect(screen.queryByText('Levels reached')).toBeNull()
    expect(screen.getByText(/^This conversation: 7 skill points, scaled so its busiest skill/)).toBeTruthy()
    unmount()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={conversation} onInspect={null} />)
    expect(screen.getByRole('button', { name: 'Conversation' })).toHaveAttribute('aria-pressed', 'true')
    localStorage.clear()
  })
  it('names bars with this conversation’s points when it is shown alone, with only the goal line', () => {
    localStorage.clear()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={conversationWith([1, 0, 2, 1, 1, 0, 2, 0])} onInspect={null} />)
    fireEvent.click(screen.getByRole('button', { name: 'Bars' }))
    fireEvent.click(screen.getByRole('button', { name: 'Conversation' }))
    const bar = screen.getByRole('button', { name: 'Information exchange: 2 points in this conversation' })
    expect(bar).toHaveClass('skill-bar')
    expect(bar).toHaveTextContent('2 pt')
    expect(document.querySelectorAll('.skill-bar-conversation')).toHaveLength(0)
    expect(document.querySelectorAll('.skill-bars-ring')).toHaveLength(1)
    expect(document.querySelector('.skill-bars-ring[data-goal] .skill-bars-tag')).toHaveTextContent('Busiest skill · 2 pt')
    // The busiest skill's bar reaches the goal line; a skill with no points has no bar.
    const width = (index: number) => parseFloat(document.querySelectorAll<HTMLElement>('.skill-bar-fill')[index].style.width)
    expect(width(2)).toBeCloseTo(parseFloat((document.querySelector('.skill-bars-ring[data-goal]') as HTMLElement).style.insetInlineStart))
    expect(width(1)).toBe(0)
    localStorage.clear()
  })
  it('keeps the language view without a conversation, whatever was chosen elsewhere', () => {
    localStorage.setItem('skellyspeak_skill_chart_shown', 'conversation')
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    expect(screen.queryByRole('group', { name: 'Points shown' })).toBeNull()
    expect(screen.getByRole('button', { name: /^Information exchange: skill level 5, 11 points/ })).toBeTruthy()
    expect(screen.getByText('Your points')).toBeTruthy()
    localStorage.clear()
  })
  it('switches between radial and bars, keeps the selection, and remembers the choice', () => {
    localStorage.clear()
    const { unmount } = render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    const start = focusHeading()
    fireEvent.click(screen.getByRole('button', { name: 'Bars' }))
    expect(document.querySelector('.skill-radar')).toBeNull()
    expect(document.querySelectorAll('.skill-bar')).toHaveLength(8)
    expect(screen.getByRole('button', { name: new RegExp(`^${start}: skill level`) })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(screen.getByRole('button', { name: /^Possibilities and constraints: skill level 4/ }))
    expect(focusHeading()).toBe('Possibilities and constraints')
    unmount()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    expect(screen.getByRole('button', { name: 'Bars' })).toHaveAttribute('aria-pressed', 'true')
    expect(document.querySelectorAll('.skill-bar')).toHaveLength(8)
    localStorage.clear()
  })
  it('draws bars to scale when asked, with the goal line at the target points', () => {
    localStorage.clear()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    fireEvent.click(screen.getByRole('button', { name: 'Bars' }))
    const widths = () => [...document.querySelectorAll<HTMLElement>('.skill-bar-fill')].map(bar => parseFloat(bar.style.width))
    const normalized = widths()
    fireEvent.click(screen.getByRole('button', { name: 'To scale' }))
    const scaled = widths()
    expect(scaled).not.toEqual(normalized)
    // To scale, 13 points is the longest bar and 6 points is 6/13 of it.
    expect(scaled[1] / scaled[6]).toBeCloseTo(6 / 13)
    localStorage.clear()
  })
  it('zooms the radar with the buttons and returns to fit', () => {
    localStorage.clear()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    const zoomLabel = () => screen.getByRole('button', { name: 'Fit the whole chart' }).textContent
    expect(zoomLabel()).toBe('1×')
    expect(screen.getByRole('button', { name: 'Zoom out' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }))
    expect(zoomLabel()).toBe('1.5×')
    expect(screen.getByRole('button', { name: 'Zoom out' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Fit the whole chart' }))
    expect(zoomLabel()).toBe('1×')
    localStorage.clear()
  })
  it('zooms bars to scale, magnifying bars and returning to fit', () => {
    localStorage.clear()
    render(<SkillLevelsPanel languageName="Spanish" snapshot={snapshotWithPoints([9, 6, 11, 5, 7, 5, 13, 8])} conversation={null} onInspect={null} />)
    fireEvent.click(screen.getByRole('button', { name: 'Bars' }))
    fireEvent.click(screen.getByRole('button', { name: 'To scale' }))
    const width = (index: number) => parseFloat(document.querySelectorAll<HTMLElement>('.skill-bar-fill')[index].style.width)
    const fitted = width(1)
    fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }))
    expect(width(1)).toBeCloseTo(fitted * 1.5)
    expect(document.querySelectorAll('.skill-bar-fill')[6]).toHaveAttribute('data-over', 'true')
    fireEvent.click(screen.getByRole('button', { name: 'Fit the whole chart' }))
    expect(width(1)).toBeCloseTo(fitted)
    localStorage.clear()
  })
})
