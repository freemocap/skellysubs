// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { TopBar } from './TopBar'
import { useConnectionHealth } from '../../state/session/connection-health'
import { useNavigationStore } from '../../state/navigation/navigation'
import { useSettingsStore } from '../../state/settings/settings'
import { useSessionStore } from '../../state/session/session'
import type { Settings } from '../../types'
import type { EffortAward } from '../../generated/contracts'
import type { SkillSnapshot } from '../../domain/learning/evidence/skills'
import { skillDemo } from '../../domain/learning/catalog/skillDemo'
import { EffortProgressContext } from '../../state/learning/EffortProgressContext'
import { playRewardSound } from '../../platform/audio/reward-sounds'

const evidence = vi.hoisted(() => ({ snapshot: null as SkillSnapshot | null }))
vi.mock('../../state/learning/useSkillEvidence', () => ({ useSkillEvidence: () => evidence }))
vi.mock('../../platform/audio/reward-sounds', () => ({ playRewardSound: vi.fn() }))
vi.mock('../../platform/ipc/skill-evidence', () => ({ getLanguageTotals: vi.fn(async () => [
  { target: 'spanish', name: 'Spanish', nativeName: 'Español', languageTag: 'es', xp: 12, conversations: 2, partnerUnderstood: 1, explorations: 0, bot: 0, revisionsSent: 0, practiceAttempts: 3 },
  { target: 'french', name: 'French', nativeName: 'Français', languageTag: 'fr', xp: 30, conversations: 1, partnerUnderstood: 0, explorations: 0, bot: 0, revisionsSent: 0, practiceAttempts: 0 },
]) }))
vi.mock('../../platform/ipc/tauri', () => ({ isTauri: true, languages: () => [
  {code:'spanish',base:'spanish',name:'Spanish',endonym:'Español',defaultVariety:'spanish-mexico',varieties:[{id:'spanish-mexico',label:'Mexico'}]}, {code:'french',base:'french',name:'French',endonym:'Français',defaultVariety:'french-france',varieties:[{id:'french-france',label:'France'}]}
] }))
beforeEach(() => {
  evidence.snapshot = null
  useConnectionHealth.setState({ routes: {} })
  useNavigationStore.setState(useNavigationStore.getInitialState())
  useSessionStore.setState(useSessionStore.getInitialState())
  useSettingsStore.setState({...useSettingsStore.getInitialState(), settings: {my_languages:['spanish','french'], target_varieties:{}, target_language:'spanish'} as Settings})
})
it('leaves the AI status to the chat composer', () => {
  render(<TopBar />)
  expect(screen.queryByRole('button', { name: /^AI (Not )?Connected$/ })).toBeNull()
  expect(screen.getByRole('button', { name: 'More' })).toBeInTheDocument()
})
it('keeps the target language reachable, with Practice and Progress as destinations among the bar\'s controls', () => {
  const setLanguage = vi.fn().mockResolvedValue(undefined)
  useSettingsStore.setState({selectLanguageVariety:setLanguage})
  useNavigationStore.getState().openProgress('skills')
  render(<TopBar />)
  fireEvent.click(screen.getByRole('button', {name:'Target language'}))
  fireEvent.click(screen.getByRole('button', {name:'Français (French)'}))
  expect(setLanguage).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', {name:'France'}))
  expect(setLanguage).toHaveBeenCalledExactlyOnceWith('french','french-france')
  // The conversation list opens from the chat header; the theme is in Settings.
  expect(screen.queryByRole('button', {name:'Conversations'})).toBeNull()
  expect(screen.queryByRole('button', {name:/Switch to (dark|light) mode/})).toBeNull()
  // The conversation is home and has no button; Practice and Progress follow the
  // language, with the bar's other controls, in the one row the bar has at
  // every width.
  const destinations = screen.getByRole('navigation', {name:'Main navigation'})
  expect(within(destinations).getAllByRole('button').map(button => button.textContent)).toEqual(['Practice', 'Progress'])
  const following = (first: Element, second: Element) => Boolean(first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING)
  expect(following(screen.getByRole('button', {name:/SkellySpeak home/}), destinations)).toBe(true)
  expect(following(screen.getByRole('button', {name:'Target language'}), destinations)).toBe(true)
  expect(following(destinations, screen.getByRole('button', {name:'More'}))).toBe(true)
  expect(within(destinations).getByRole('button', {name:'Progress'})).toHaveAttribute('aria-current', 'page')
})
it('disables language switching during a save or settings edit', () => {
  useSettingsStore.setState({savingLanguage:true})
  const view = render(<TopBar />)
  expect(screen.getByRole('button', {name:'Target language'})).toBeDisabled()
  useSettingsStore.setState({savingLanguage:false})
  useNavigationStore.getState().showOverlay('settings')
  view.rerender(<TopBar />)
  expect(screen.getByRole('button', {name:'Target language'})).toBeDisabled()
})
it('moves between the destinations, returns home from the wordmark and preserves secondary navigation', () => {
  useNavigationStore.getState().setMode('review')
  render(<TopBar />)
  const destinations = screen.getByRole('navigation', {name:'Main navigation'})
  expect(within(destinations).getByRole('button', {name:'Practice'})).not.toHaveAttribute('aria-current')
  fireEvent.click(within(destinations).getByRole('button', {name:'Practice'}))
  expect(useNavigationStore.getState()).toMatchObject({mode:'practice',page:'guided',practiceView:'drill',drillOpened:true})
  expect(within(destinations).getByRole('button', {name:'Practice'})).toHaveAttribute('aria-current', 'page')
  expect(within(destinations).getByRole('button', {name:'Progress'})).not.toHaveAttribute('aria-current')
  fireEvent.click(screen.getByRole('button', {name:/SkellySpeak home/}))
  expect(useNavigationStore.getState()).toMatchObject({page:'guided',practiceView:'chat',mobileSurface:'chat'})
  expect(within(destinations).queryByRole('button', {current:'page'})).toBeNull()
  fireEvent.click(screen.getByRole('button', {name:'More'}))
  expect(useNavigationStore.getState().overlay).toBe('more')
})

it('opens the language browser from the compact selector', () => {
  render(<TopBar />)
  fireEvent.click(screen.getByRole('button', { name: 'Target language' }))
  fireEvent.click(screen.getByRole('button', { name: 'Add language…' }))
  expect(useNavigationStore.getState().overlay).toBe('languages')
  expect(screen.getByRole('button', { name: 'Target language' })).toBeInTheDocument()
})

it('shows the all-languages card on the first press and opens the XP tab on the second', () => {
  render(<TopBar />)
  const xp = screen.getByRole('button', { name: 'Language XP' })
  expect(xp).toHaveAccessibleDescription('App streak: 1')
  fireEvent.click(xp)
  expect(screen.getByRole('dialog', { name: 'All languages' })).toBeVisible()
  expect(useNavigationStore.getState().page).toBe('guided')
  fireEvent.click(xp)
  expect(screen.queryByRole('dialog', { name: 'All languages' })).toBeNull()
  expect(useNavigationStore.getState()).toMatchObject({ page: 'skills', progressTab: 'xp' })
})
it('shows the card on mouse hover, where a click then opens the XP tab', () => {
  vi.useFakeTimers()
  render(<TopBar />)
  const xp = screen.getByRole('button', { name: 'Language XP' })
  fireEvent.pointerEnter(xp, { pointerType: 'mouse' })
  expect(screen.getByRole('dialog', { name: 'All languages' })).toBeVisible()
  fireEvent.pointerLeave(xp, { pointerType: 'mouse' })
  act(() => { vi.advanceTimersByTime(300) })
  expect(screen.queryByRole('dialog', { name: 'All languages' })).toBeNull()
  fireEvent.pointerEnter(xp, { pointerType: 'mouse' })
  fireEvent.click(xp)
  expect(useNavigationStore.getState().progressTab).toBe('xp')
  vi.useRealTimers()
})
it('shows the total across languages behind a globe, and every language with its effort in the card', async () => {
  render(<TopBar />)
  expect(await screen.findByLabelText('Total XP: 42')).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Language XP' }))
  const card = screen.getByRole('dialog', { name: 'All languages' })
  expect(within(card).getAllByRole('row').slice(1).map(row => within(row).getByRole('rowheader').textContent)).toEqual(['FRFrench', 'ESSpanish', 'Total'])
  expect(within(card).getByRole('button', { name: 'Practice' })).toBeVisible()
})

/// jsdom has no layout. In this model the language's own name needs 60px; its
/// box has `room` pixels while the XP number shows, and 40px more once the pill
/// folds to the skill level. A folded XP button has no boxes, as under display: none.
function measureBar(room: () => number) {
  const resized: Array<() => void> = []
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { resized.push(callback) }
    observe() {} unobserve() {} disconnect() {}
  })
  const folded = (element: Element) => element.closest('.topbar')?.getAttribute('data-fit') === 'compact-progress'
  const spies = [
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      return { width: this.matches('.learning-picker-endonym') ? 60 : 0, height: 0, top: 0, left: 0, right: 0, bottom: 0, x: 0, y: 0, toJSON() {} } as DOMRect
    }),
    vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(function (this: Element) {
      return this.matches('.learning-picker-identity > span:first-child') ? room() + (folded(this) ? 40 : 0) : 0
    }),
    vi.spyOn(Element.prototype, 'getClientRects').mockImplementation(function (this: Element) {
      return (this.matches('.profile-trigger') && folded(this) ? [] : [{}]) as unknown as DOMRectList
    }),
  ]
  return {
    resize: () => act(() => resized.forEach(callback => callback())),
    restore: () => { spies.forEach(spy => spy.mockRestore()); vi.unstubAllGlobals() },
  }
}

it('folds the progress pill to the skill level while the language’s own name would be cut off, and unfolds it when the bar has room', async () => {
  evidence.snapshot = { ...skillDemo, target: 'spanish' }
  let room = 40
  const bar = measureBar(() => room)
  try {
    render(<TopBar />)
    const row = document.querySelector('.topbar')!
    expect(row).toHaveAttribute('data-fit', 'compact-progress')
    expect(screen.getByRole('button', { name: 'Skill level 0' })).toBeInTheDocument()
    room = 80
    bar.resize()
    await waitFor(() => expect(row).toHaveAttribute('data-fit', 'full'))
  } finally {
    bar.restore()
  }
})

it('plays the reward pop on the pill while its XP number is folded away, and on the XP number otherwise', () => {
  evidence.snapshot = { ...skillDemo, target: 'spanish' }
  const award: EffortAward = { id: 'award', dimension: 'revisions_sent', sourceId: 'source', language: 'spanish', variety: '', conversationId: null, policy: 'policy', createdAt: '2026-10-07', claimed: false }
  const effort = { target: 'spanish', value: { target: 'spanish', partnerUnderstood: 0, revisionsSent: 1, practiceAttempts: 0, explorations: 0, bot: 0, recent: [award] }, error: null, arrived: ['award'], effects: true }
  for (const [room, target] of [[40, '.progress-anchor'], [80, '.profile-trigger']] as const) {
    vi.mocked(playRewardSound).mockClear()
    const bar = measureBar(() => room)
    try {
      const view = render(<EffortProgressContext value={effort}><TopBar /></EffortProgressContext>)
      expect(playRewardSound).toHaveBeenCalledExactlyOnceWith({ kind: 'pop' }, document.querySelector(`.topbar ${target}`))
      view.unmount()
    } finally {
      bar.restore()
    }
  }
})
