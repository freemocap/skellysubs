// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { useEffect, useState } from 'react'
import App from './App'
import { useSessionStore } from '../state/session/session'
import { useNavigationStore } from '../state/navigation/navigation'
import { GuideActions, type RunGuideAction } from '../components/learning/GuideActions'

const startSkill = vi.hoisted(() => vi.fn(async (_language: string, _variety: string, _skill: string) => {}))
const askGuide = vi.hoisted(() => vi.fn<RunGuideAction>(async () => {}))

vi.mock('../components/layout/useIsMobile', () => ({ useIsMobile: () => true }))
vi.mock('../state/learning/useSkillEvidence', async importOriginal => ({ ...await importOriginal<typeof import('../state/learning/useSkillEvidence')>(), useSkillEvidence: () => ({ snapshot: null, error: null }) }))
const { native, state } = vi.hoisted(() => {
  const state = { connection: { route: 'hosted', signedIn: true, email: '', revision: 1, configured: true, standardModel: '', fastModel: '', paused: false } }
  const native = vi.fn(async (command: string) => {
    if (command === 'get_connection') return state.connection
    if (command === 'select_route') {
      state.connection = { ...state.connection, route: 'hosted', revision: state.connection.revision + 1 }
      return state.connection
    }
    if (command === 'hosted_sign_in') {
      state.connection = { ...state.connection, signedIn: true, configured: true }
      return { email: 'learner@example.test', remaining: 1 }
    }
    throw new Error(`Unexpected command: ${command}`)
  })
  return { native, state }
})
vi.mock('../platform/ipc/tauri', () => ({ isTauri: true, getSettings: async () => ({ native_language: 'english', native_variety: 'english-united-states', interface_locale: 'english', target_language: 'spanish', provider_mode: 'custom' }), invoke: native, languageFor: () => null, languages: () => [] }))
vi.mock('./shell/GraphHoverOverlay', () => ({ GraphHoverOverlay: () => null }))
vi.mock('./shell/UpdateBanner', () => ({ UpdateBanner: () => null }))
vi.mock('../features/settings/SettingsModal', () => ({ SettingsModal: () => null }))
vi.mock('../features/skills/SkillsPage', () => ({ default: ({ onPractice }: { onPractice: (language: string, variety: string, skill: string) => Promise<void> }) => <><button onClick={() => void onPractice('spanish', 'spanish-spain', 'time_events')}>Practice this skill</button><GuideActions guide={{ language: 'spanish', variety: 'spanish-spain', skillId: 'time_events', editionLanguage: 'english', fingerprint: 'source' }} subskill="sequence" /></> }))
vi.mock('../features/drill/DrillPage', () => ({ default: () => <p>Drill surface</p> }))
// The page is replaced, but it reads access the way the real one does — from the
// session store — rather than through props the shell no longer threads down.
vi.mock('../features/conversation/ConversationPage', () => ({ default: ({ mobileSurface, onSkillStartReady, onGuideActionReady }: { mobileSurface: string; onSkillStartReady: (action: typeof startSkill | null) => void; onGuideActionReady: (action: RunGuideAction | null) => void }) => {
  useEffect(() => { onSkillStartReady(startSkill); return () => onSkillStartReady(null) }, [onSkillStartReady])
  useEffect(() => { onGuideActionReady(askGuide); return () => onGuideActionReady(null) }, [onGuideActionReady])
  const [draft, setDraft] = useState('')
  const connection = useSessionStore((state) => state.connection)
  const startHostedSignIn = useSessionStore((state) => state.startHostedSignIn)
  return <><p>Practice surface: {mobileSurface}</p><input aria-label="Draft" value={draft} onChange={event => setDraft(event.target.value)} />{connection?.configured === false && <button type="button" onClick={() => void startHostedSignIn()}>Sign in with Google</button>}</>
} }))

it('keeps navigation reachable and preserves the mounted page stub across destinations', async () => {
  HTMLDialogElement.prototype.showModal = function () { this.open = true }
  HTMLDialogElement.prototype.close = function () { this.open = false }
  render(<App />)
  const nav = screen.getByRole('navigation', { name: 'Main navigation' })
  expect(within(nav).getAllByRole('button').map(button => button.textContent)).toEqual(['Practice', 'Progress'])
  expect(screen.queryByText('Guided conversation')).not.toBeInTheDocument()
  fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'Keep my words' } })
  // Progress and back keeps the conversation mounted; the return strip leads back.
  fireEvent.click(within(nav).getByRole('button', { name: 'Progress' }))
  await screen.findByRole('button', { name: 'Practice this skill' })
  expect(within(nav).getByRole('button', { name: 'Progress' })).toHaveAttribute('aria-current', 'page')
  fireEvent.click(screen.getByRole('button', { name: 'Back to conversation' }))
  expect(within(nav).queryByRole('button', { current: 'page' })).toBeNull()
  expect(screen.getByLabelText('Draft')).toHaveValue('Keep my words')
  expect(screen.getByText('Practice surface: chat')).toBeInTheDocument()
  // The coach opens from the conversation; the wordmark brings the conversation back.
  act(() => useNavigationStore.getState().openConversation('panel'))
  expect(screen.getByText('Practice surface: panel')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'SkellySpeak home — Chat' }))
  expect(screen.getByText('Practice surface: chat')).toBeInTheDocument()
  expect(screen.getByLabelText('Draft')).toHaveValue('Keep my words')
  fireEvent.click(screen.getByRole('button', { name: 'More' }))
  fireEvent.click(screen.getByRole('button', { name: 'AI activity' }))
  expect(screen.getByText('Live operations')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Close AI activity' }))
  expect(screen.queryByText('Live operations')).not.toBeInTheDocument()
  expect(screen.getByLabelText('Draft')).toHaveValue('Keep my words')
})

// The phone sheet's close control lives in the view's own header actions.
vi.mock('../features/activity/AiView', () => ({ AiView: ({ actions }: { actions: React.ReactNode }) => <><p role="status">Live operations</p>{actions}</> }))

it('opens Practice over the mounted conversation, saves the destination and returns from its strip', async () => {
  render(<App />)
  const switcher = screen.getByRole('navigation', { name: 'Main navigation' })
  fireEvent.change(screen.getByLabelText('Draft'), { target: { value: 'Still here' } })
  act(() => useNavigationStore.getState().openConversation('panel'))
  fireEvent.click(within(switcher).getByRole('button', { name: 'Practice' }))
  const holder = (element: HTMLElement) => element.closest('.page-holder')!
  expect(holder(await screen.findByText('Drill surface'))).not.toHaveAttribute('aria-hidden', 'true')
  // The conversation stays mounted underneath, hidden rather than destroyed.
  expect(holder(screen.getByLabelText('Draft'))).toHaveAttribute('aria-hidden', 'true')
  expect(localStorage.getItem('skellyspeak.practice-view')).toBe('drill')

  fireEvent.click(screen.getByRole('button', { name: 'Back to conversation' }))
  expect(holder(screen.getByLabelText('Draft'))).not.toHaveAttribute('aria-hidden', 'true')
  expect(screen.getByLabelText('Draft')).toHaveValue('Still here')
  expect(screen.getByText('Practice surface: panel')).toBeInTheDocument()
  expect(holder(screen.getByText('Drill surface'))).toHaveAttribute('aria-hidden', 'true')
  expect(localStorage.getItem('skellyspeak.practice-view')).toBe('chat')
  localStorage.removeItem('skellyspeak.practice-view')
})

it('reaches the conversation from a skill action even when Practice was the previous destination', async () => {
  render(<App />)
  const navigation = screen.getByRole('navigation', { name: 'Main navigation' })
  fireEvent.click(within(navigation).getByRole('button', { name: 'Practice' }))
  await screen.findByText('Drill surface')
  fireEvent.click(within(navigation).getByRole('button', { name: 'Progress' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Practice this skill' }))
  await waitFor(() => expect(screen.getByLabelText('Draft').closest('.page-holder')).toHaveAttribute('aria-hidden', 'false'))
  expect(startSkill).toHaveBeenCalledWith('spanish', 'spanish-spain', 'time_events')
  expect(useNavigationStore.getState()).toMatchObject({ page: 'guided', practiceView: 'chat', mobileSurface: 'chat' })
})

it('hands a selected skill to coaching and opens the coach surface', async () => {
  render(<App />)
  fireEvent.click(within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name: 'Progress' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Ask coach' }))
  await waitFor(() => expect(screen.getByLabelText('Draft').closest('.page-holder')).toHaveAttribute('aria-hidden', 'false'))
  expect(askGuide).toHaveBeenCalledWith({ kind: 'coach', text: 'Explain this selection.', guide: { language: 'spanish', variety: 'spanish-spain', skillId: 'time_events', editionLanguage: 'english', fingerprint: 'source' }, focus: { kind: 'subskill', subskillId: 'sequence' } })
  expect(useNavigationStore.getState()).toMatchObject({ page: 'guided', mobileSurface: 'panel' })
})

it('connects the page stub to hosted sign-in through the session store', async () => {
  state.connection = { route: 'custom', signedIn: false, email: '', revision: 7, configured: false, standardModel: '', fastModel: '', paused: false }
  native.mockClear()
  // Startup loads the session store; a test that asserts on what it holds seeds
  // it the same way rather than relying on the shell to fetch.
  await act(async () => { await useSessionStore.getState().refresh() })
  render(<App />)
  fireEvent.click(await screen.findByRole('button', { name: 'Sign in with Google' }))
  await waitFor(() => expect(native).toHaveBeenCalledWith('select_route', { expectedRevision: 7, route: 'hosted' }))
  await waitFor(() => expect(native).toHaveBeenCalledWith('hosted_sign_in'))
})
