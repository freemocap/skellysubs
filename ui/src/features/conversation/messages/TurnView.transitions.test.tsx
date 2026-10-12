// @vitest-environment jsdom
import { act, fireEvent, render } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { ReadingPreferencesContext } from '../../../components/reading/ReadingPreferences'
import { PendingTurn } from './PendingTurn'
import { TurnView, type TurnShape } from './TurnView'
import type { PendingMessage } from '../session/usePendingMessage'

const preferences = { autoTranslate: true, alwaysRomanize: true, alwaysPronunciation: true, supportsRomanization: true }
const pending: PendingMessage = { key: 'send', text: 'البيوت cafe\u0301 中文', phase: 'sending', editing: null, failure: null, retry: null, known: new Set() }
const turn = (state: string): TurnShape => ({
  id: 1, user: pending.text, pendingText: '', userGlossState: state, userTranslationState: state,
  assistant: { reply: '你好', tokens: [], user_tokens: [], translation: '', user_translation: '', mechanics: [], errors: [],
    scaffolds: { replies: [], frames: [], starters: [] }, glossState: state, translationState: state },
})
const renderTurn = (state: string) => <ReadingPreferencesContext value={preferences}>
  <TurnView turn={turn(state)} editing={false} focused={false} reviewing ttsReady={false} speaking={false} rtl
    onBubbleTap={vi.fn()} onAskCoach={vi.fn()} />
</ReadingPreferencesContext>

it('wires preparation into the message control without cancelling it when inspection opens', () => {
  const toggle = vi.fn(), onSpeak = vi.fn()
  const speech = { retained: null, time: 0, playing: false, preparing: true, enabled: true, rate: 1, volume: 1, seek: vi.fn(), stop: vi.fn(), toggle }
  const content = (playing: boolean) => <TurnView turn={turn('ready')} editing={false} focused={false} reviewing={false} ttsReady speaking={playing} rtl={false}
    partnerSpeech={{ ...speech, playing, preparing: !playing }} onSpeak={onSpeak} onBubbleTap={vi.fn()} onAskCoach={vi.fn()} />
  const view = render(content(false))
  const control = view.getByRole('button', { name: 'Cancel speech preparation' })
  fireEvent.click(view.getByRole('button', { name: 'Inspect recording' }))
  expect(toggle).not.toHaveBeenCalled()
  expect(view.queryByText('Recording audio unavailable.')).toBeNull()
  fireEvent.click(control)
  expect(onSpeak).toHaveBeenCalledOnce()
  view.rerender(content(true))
  expect(view.getByRole('button', { name: 'Stop playback' })).toBe(control)
})

it('keeps requested aid rows and partner reaction space through admission, queue and execution', () => {
  const view = render(<ReadingPreferencesContext value={preferences}><PendingTurn message={pending} rtl onDismiss={vi.fn()} /></ReadingPreferencesContext>)
  for (const side of ['me', 'bot']) {
    const bubble = view.container.querySelector(`.msg.${side}`) as HTMLElement
    expect(bubble).toHaveClass('aids-reserved', 'rtl')
    expect(bubble.style.getPropertyValue('--reading-aid-rows')).toBe('2')
    expect(bubble.querySelector('.trans.hydrating-slot')).not.toBeNull()
  }
  expect(view.container.querySelector('.partner-turn > .partner-reaction-slot')).not.toBeNull()
  expect(view.container.querySelector('.message-feedback-line')).toHaveAttribute('aria-hidden', 'true')
  expect(view.container.querySelector('.msg.me .reply-received')?.textContent).toBe(pending.text)
  for (const state of ['ready', 'waiting_dependencies', 'running']) {
    view.rerender(renderTurn(state))
    expect(view.container.querySelectorAll('.msg.aids-reserved')).toHaveLength(2)
    expect(view.container.querySelector('.msg.bot .trans.hydrating-slot')).not.toBeNull()
    expect(view.container.querySelector('.partner-turn > .partner-reaction-slot')).not.toBeNull()
  }
  view.rerender(renderTurn('failed'))
  expect(view.container.querySelectorAll('.msg.aids-reserved')).toHaveLength(0)
})

it.each([
  { autoTranslate: false, alwaysRomanize: false, alwaysPronunciation: false, supportsRomanization: true, rows: 0 },
  { autoTranslate: true, alwaysRomanize: false, alwaysPronunciation: false, supportsRomanization: true, rows: 1 },
  { autoTranslate: false, alwaysRomanize: true, alwaysPronunciation: false, supportsRomanization: true, rows: 1 },
  { autoTranslate: false, alwaysRomanize: true, alwaysPronunciation: false, supportsRomanization: false, rows: 0 },
])('reserves $rows aid rows for declared reading preferences', settings => {
  const view = render(<ReadingPreferencesContext value={settings}><PendingTurn message={pending} rtl={false} onDismiss={vi.fn()} /></ReadingPreferencesContext>)
  const bubble = view.container.querySelector('.msg.me') as HTMLElement
  expect(bubble.style.getPropertyValue('--reading-aid-rows')).toBe(String(settings.rows))
  expect(bubble.classList.contains('aids-reserved')).toBe(settings.rows > 0)
})

it('continues the live reveal when a saved reply takes over, without replaying its prefix', () => {
  vi.useFakeTimers()
  const text = 'Hello one two three four five six seven eight'
  const execution: NonNullable<TurnShape['execution']> = {
    id: 'turn', state: 'pending', paused: false, route: 'hosted', hold: null, replacesTurnId: null, replacedBy: null,
    nativeExecutionAvailable: true,
    nativePreview: { attempt: '1', execution: '1', capture: { session: 'session', sequence: '1', text, failure: null }, retained_sequence: '1', uncommitted: false, live: true, complete: false },
  }
  const content = (value: TurnShape) => <TurnView turn={value} editing={false} focused={false} reviewing={false} ttsReady={false} speaking={false} rtl={false} onBubbleTap={vi.fn()} onAskCoach={vi.fn()} />
  const pendingTurn: TurnShape = { id: 1, turnId: 'turn', user: 'Hi', pendingText: '', assistant: null, execution, replyState: { state: 'pending', error: null, control: null } }
  const view = render(content({ ...pendingTurn, execution: { ...execution, nativePreview: undefined } }))
  try {
    view.rerender(content(pendingTurn))
    act(() => vi.advanceTimersByTime(80))
    const remaining = view.container.querySelector('.reply-unrevealed')?.textContent
    expect(remaining).toBeTruthy()
    view.rerender(content({ ...pendingTurn, assistant: { ...turn('ready').assistant!, reply: text }, execution: {
      ...execution, state: 'succeeded',
    } }))
    expect(view.container.querySelector('.reply-unrevealed')?.textContent).toBe(remaining)
    act(() => vi.advanceTimersByTime(800))
    expect(view.container.querySelector('.reply-unrevealed')).toBeNull()
    expect(view.container.querySelector('.msg.bot')).toHaveTextContent(text)
  } finally {
    view.unmount()
    vi.useRealTimers()
  }
})
