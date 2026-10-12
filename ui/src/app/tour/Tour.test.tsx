// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { Tour } from './Tour'
import { useOnboardingStore } from '../../state/settings/onboarding'
import type { Preferences } from '../../generated/contracts'

vi.mock('../../domain/input/back', () => ({ openOverlay: () => () => {} }))
const demoLanguage = (code: string, direction: 'ltr' | 'rtl', varietyId: string) => ({
  code, name: code, endonym: code, base: code, defaultVariety: varietyId, fontScale: 1, direction, languageTag: code.slice(0, 2), romanization: null, transcriptionLanguage: null,
  greeting: { text: '', romanized: null }, partner: { name: '', romanizedName: null, vibe: [] },
  varieties: [{ id: varietyId, label: varietyId, direction, fontScale: 1, romanization: null, transcriptionLanguage: null }],
})
const demoLanguages = [demoLanguage('spanish', 'ltr', 'spanish-spain'), demoLanguage('arabic', 'rtl', 'arabic-levantine'), demoLanguage('english', 'ltr', 'english-united-states')]
vi.mock('../../platform/ipc/tauri', () => ({
  // The top bar reads language totals; the tour's assertions do not depend on them.
  invoke: async (command: string) => { if (command === 'get_language_totals') return []; throw new Error(`Tour test has no IPC fixture for ${command}`) },
  languages: () => demoLanguages,
  languageFor: (code: string, varietyId?: string) => {
    const language = demoLanguages.find(item => item.code === code)
    if (!language) return null
    const variety = varietyId ? language.varieties.find(item => item.id === varietyId) : undefined
    return variety ? { ...language, direction: variety.direction, fontScale: variety.fontScale } : language
  },
}))

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function (): void { this.open = true }
  HTMLDialogElement.prototype.close = function (): void { this.open = false }
  // jsdom computes no layout: every element reports a zero rect. The tour
  // treats a zero rect as "not really there", so tests give every element a
  // plausible size to work with, the same way tourTarget.test.ts does per element.
  Element.prototype.getBoundingClientRect = () => ({ x: 0, y: 0, width: 40, height: 20, top: 0, left: 0, right: 40, bottom: 20, toJSON: () => ({}) })
  useOnboardingStore.setState({ busy: false, preferences: { onboardingHelp: true } as Preferences,
    showHelp: vi.fn(async show => { useOnboardingStore.setState({ preferences: { onboardingHelp: show } as Preferences }) }),
  })
})

it('opens on the map, with every view and its stop count pinned over its own demo', async () => {
  render(<Tour />)
  expect(screen.getByRole('heading', { name: 'Chat' })).toBeInTheDocument()
  for (const view of ['Chat', 'Practice', 'Progress', 'AI panel']) {
    const section = screen.getByRole('group', { name: view })
    await waitFor(() => expect(within(section).getAllByRole('button', { name: /^Stop \d/ })).toHaveLength(view === 'Chat' ? 9 : view === 'Practice' ? 6 : 4))
  }
  // Each view's own demo content is really mounted, not a stand-in graphic.
  expect(within(screen.getByRole('group', { name: 'Chat' })).getByLabelText('Record audio')).toBeInTheDocument()
})

it('a map number jumps straight to that stop, over the same demo', async () => {
  render(<Tour />)
  const pin = await within(screen.getByRole('group', { name: 'Practice' })).findByRole('button', { name: 'Stop 3: Record an attempt' })
  fireEvent.click(pin)
  expect(screen.getByRole('heading', { name: 'Record an attempt' })).toBeInTheDocument()
  expect(screen.getByText('Practice · 3 of 6')).toBeInTheDocument()
  expect(screen.getByText('#1')).toBeInTheDocument()
})

it('Continue on the map starts Chat at its first stop', () => {
  render(<Tour />)
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  expect(screen.getByRole('heading', { name: 'Tap a word' })).toBeInTheDocument()
  expect(screen.getByLabelText('Record audio')).toBeInTheDocument()
})

it('steps through a view with Continue and back to the map on the first stop', () => {
  render(<Tour />)
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  expect(screen.getByRole('heading', { name: 'Play aloud' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Back' }))
  expect(screen.getByRole('heading', { name: 'Tap a word' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Map' }))
  expect(screen.getByText('Select a number to go to that part, or Continue to go through in order.')).toBeInTheDocument()
})

it('each view opens on its own demo page when picked from the tabs', () => {
  render(<Tour />)
  fireEvent.click(screen.getByRole('tab', { name: 'Progress' }))
  expect(screen.getByRole('heading', { name: 'Skill level' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('tab', { name: 'AI panel' }))
  expect(screen.getByRole('heading', { name: 'Activity' })).toBeInTheDocument()
  expect(screen.getByLabelText('Everything recorded about Reply')).toBeInTheDocument()
})

it('the last stop of a view offers the next view and finishing', async () => {
  render(<Tour />)
  fireEvent.click(screen.getByRole('tab', { name: 'AI panel' }))
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }))
  expect(screen.getByRole('heading', { name: 'Details' })).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Next: Chat' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Finish' }))
  await waitFor(() => expect(useOnboardingStore.getState().showHelp).toHaveBeenCalledWith(false))
})

it('closing records dismissal and reports a failure without closing', async () => {
  render(<Tour />)
  fireEvent.click(screen.getByRole('button', { name: 'Close tour' }))
  await waitFor(() => expect(useOnboardingStore.getState().showHelp).toHaveBeenCalledWith(false))

  useOnboardingStore.setState({ showHelp: vi.fn().mockRejectedValue(new Error('Revision conflict')) })
  fireEvent.click(screen.getByRole('button', { name: 'Close tour' }))
  expect(await screen.findByRole('alert')).toHaveTextContent('Revision conflict')
})
