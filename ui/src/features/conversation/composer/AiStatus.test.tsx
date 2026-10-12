// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { Disposition, InspectionSnapshot } from '../../../generated/graph-contracts'
import definition from '../../../generated/coach-graph.json'
import type { TurnView } from '../../../generated/contracts'
import { useConnectionHealth } from '../../../state/session/connection-health'
import { useNavigationStore } from '../../../state/navigation/navigation'
import { useSessionStore } from '../../../state/session/session'
import { useAiWindowStore } from '../../../state/navigation/ai-window'
import { AiStatus } from './AiStatus'

const windowApi = vi.hoisted(() => ({ openAiWindow: vi.fn() }))
vi.mock('../../../platform/ipc/window', () => ({ ...windowApi, aiWindowState: async () => ({ supported: true, open: false }) }))

function connection(configured: boolean) {
  useSessionStore.setState({ connection: {
    route: 'hosted', signedIn: true, email: '', revision: 1,
    configured, assessmentAdapter: 'jev_choice' as const, standardModel: 'standard', fastModel: 'fast', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false,
  } })
  if (configured) useConnectionHealth.setState({ routes: { hosted: { revision: 1, status: 'connected', checkedAt: 1, error: null } } })
}

function turn(id: string, nodes: Record<string, Disposition>): TurnView {
  return { id, state: 'assisting', paused: false, hold: null, route: 'hosted', replacesTurnId: null, replacedBy: null,
      nativeGraph: { ...definition, artifact: definition.artifact as InspectionSnapshot['artifact'],
      engine: 'engine', revision: '1', run: id, nodes, activation: {}, paused: false, active: true,
      stepping: null, step_available: false, attempts: {}, reasons: {} } }
}

const idle = { transcribing: false, scheduling: false, turns: [], synthesizing: false, beside: null }
const glossing = turn('t1', { reply_gloss: 'Running' })
const translating = turn('t1', { reply_translation: 'Running' })

beforeEach(() => {
  useConnectionHealth.setState({ routes: {} })
  useNavigationStore.setState(useNavigationStore.getInitialState())
  useSessionStore.setState(useSessionStore.getInitialState())
  useAiWindowStore.setState({ supported: true, open: false })
})
afterEach(() => { vi.useRealTimers() })

it('rests grey, with no line, while connected and idle', () => {
  connection(true)
  const view = render(<AiStatus {...idle} />)
  const pill = screen.getByRole('button', { name: 'AI Connected' })
  expect(pill).toHaveTextContent('AI')
  // The connected mark is the dot's colour; it keys on this attribute.
  expect(pill).toHaveAttribute('data-configured', 'true')
  expect(view.container.querySelector('.ai-status')).not.toHaveAttribute('data-busy')
  expect(view.container.querySelector('.ai-status-text')).toBeNull()
})

it.each(['hosted', 'custom'] as const)('says AI is not connected and opens AI access from the %s setup status', route => {
  useSessionStore.setState({ connection: {
    route, signedIn: false, email: '', revision: 1,
    configured: false, assessmentAdapter: 'jev_choice' as const, standardModel: 'standard', fastModel: 'fast', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false,
  } })
  const view = render(<AiStatus {...idle} />)
  expect(view.container.querySelector('.ai-status')).toHaveAttribute('data-tone', 'offline')
  expect(view.container.querySelector('.ai-status-text')).toHaveTextContent('AI not connected')
  fireEvent.click(screen.getByRole('button', { name: 'AI Not Connected' }))
  expect(useNavigationStore.getState().overlay).toBe('settings')
})

it('counts as connected only after a successful check at the current revision', () => {
  useSessionStore.setState({ connection: {
    route: 'custom', signedIn: false, email: '', revision: 9,
    configured: true, assessmentAdapter: 'jev_choice' as const, standardModel: 'standard', fastModel: 'fast', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false,
  } })
  const view = render(<AiStatus {...idle} />)
  expect(screen.getByRole('button', { name: 'AI Not Connected' })).toBeInTheDocument()
  act(() => useConnectionHealth.getState().record('custom', 9))
  // Connected, the pill opens the AI View; AI access stays under Settings.
  fireEvent.click(screen.getByRole('button', { name: 'AI Connected' }))
  expect(useNavigationStore.getState().overlay).toBe('activity')
  act(() => useSessionStore.setState(state => ({ connection: { ...state.connection!, revision: 10 } })))
  view.rerender(<AiStatus {...idle} />)
  expect(screen.getByRole('button', { name: 'AI Not Connected' })).toBeInTheDocument()
})

it('keeps a known connection through a re-check, so the first click after returning to the app opens the AI View', () => {
  connection(true)
  render(<AiStatus {...idle} />)
  // Focusing the window re-checks the same configuration; the result it replaces was connected.
  act(() => useConnectionHealth.getState().begin('hosted', 1))
  const pill = screen.getByRole('button', { name: 'AI Connected' })
  expect(pill).toHaveAttribute('data-configured', 'true')
  fireEvent.click(pill)
  expect(useNavigationStore.getState().overlay).toBe('activity')
})

it('opens the AI View, not AI access, while the first check is still running', () => {
  connection(true)
  useConnectionHealth.setState({ routes: { hosted: { revision: 1, status: 'checking', checkedAt: null, error: null } } })
  render(<AiStatus {...idle} />)
  fireEvent.click(screen.getByRole('button', { name: 'Checking AI connection…' }))
  expect(useNavigationStore.getState().overlay).toBe('activity')
})

it('opens and closes the AI panel, and marks it open', () => {
  connection(true)
  render(<AiStatus {...idle} />)
  const pill = screen.getByRole('button', { name: 'AI Connected' })
  fireEvent.click(pill)
  expect(useNavigationStore.getState().overlay).toBe('activity')
  expect(pill).toHaveAttribute('aria-expanded', 'true')
  fireEvent.click(pill)
  expect(useNavigationStore.getState().overlay).toBeNull()
})

it('focuses the popped-out AI window instead of opening a second view', () => {
  connection(true)
  windowApi.openAiWindow.mockResolvedValue(undefined)
  useAiWindowStore.setState({ supported: true, open: true })
  render(<AiStatus {...idle} />)
  fireEvent.click(screen.getByRole('button', { name: 'AI Connected' }))
  expect(windowApi.openAiWindow).toHaveBeenCalledOnce()
  expect(useNavigationStore.getState().overlay).toBeNull()
})

it('moves while a graph node runs and exposes its exact identity', () => {
  connection(true)
  const view = render(<AiStatus {...idle} turns={[glossing]} />)
  expect(view.container.querySelector('.ai-status')).toHaveAttribute('data-busy', 'true')
  const line = view.container.querySelector('.ai-status-line')
  expect(line).toHaveTextContent('reply gloss')
  expect(line).toHaveAttribute('title', 'reply_gloss')
  // Background work is shown, not announced.
  expect(view.container.querySelector('.ai-status-announce')).toBeEmptyDOMElement()
})

it('announces the steps a learner starts', () => {
  connection(true)
  const view = render(<AiStatus {...idle} transcribing />)
  expect(view.container.querySelector('.ai-status-announce')).toHaveTextContent('Transcribing recorded audio…')
})

it('keeps each line up long enough to read before the next replaces it', () => {
  vi.useFakeTimers()
  connection(true)
  const view = render(<AiStatus {...idle} turns={[glossing]} />)
  const text = () => view.container.querySelector('.ai-status-text')?.textContent
  expect(text()).toBe('reply gloss…')
  view.rerender(<AiStatus {...idle} turns={[translating]} />)
  expect(text()).toBe('reply gloss…')
  act(() => { vi.advanceTimersByTime(1000) })
  expect(text()).toBe('reply translation…')
})

it('shows the longest wording that fits the line, and re-chooses as the line resizes', () => {
  connection(true)
  // Rendered widths of each wording, and the room the line has, as layout would report them.
  const widths: Record<string, number> = { 'Transcribing recorded audio…': 135, 'Transcribing audio…': 102, 'Transcribing…': 62 }
  let room = 77
  let resized: (() => void) | undefined
  vi.stubGlobal('ResizeObserver', class { constructor(callback: () => void) { resized = callback } observe() {} disconnect() {} })
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    return { width: widths[this.textContent ?? ''] ?? 0 } as DOMRect
  })
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(function (this: Element) {
    return this.classList.contains('ai-status-line') ? room : 0
  })
  try {
    const view = render(<AiStatus {...idle} transcribing />)
    const shown = () => view.container.querySelector('.ai-status-text')?.textContent ?? null
    expect(shown()).toBe('Transcribing…')
    for (const [next, expected] of [[120, 'Transcribing audio…'], [300, 'Transcribing recorded audio…'], [40, null]] as const) {
      room = next
      act(() => resized?.())
      expect(shown()).toBe(expected)
    }
  } finally {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  }
})

it('links the latest exchange’s failed follow-on work to its AI activity', () => {
  connection(true)
  const onInspectLatest = vi.fn()
  const failed = turn('t1', { reply: 'Adopted', gloss: 'Failed' })
  render(<AiStatus {...idle} turns={[failed]} latest={failed} onInspectLatest={onInspectLatest} />)
  fireEvent.click(screen.getByRole('button', { name: '1 failed' }))
  expect(onInspectLatest).toHaveBeenCalledOnce()
})
