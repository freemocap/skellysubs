import { DEFAULT_EXECUTION, DEFAULT_APPEARANCE } from '../../generated/contracts'
// @vitest-environment jsdom
import { StrictMode } from 'react'
import userEvent from '@testing-library/user-event'
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Command, ConversationSnapshot, Receipt, Snapshot, TranscriptionInspectionResult } from '../../generated/contracts'
import type { Settings } from '../../types'
import { createSpectrumFeed, type SpectrumFeed } from '../../domain/audio/spectrum-feed'
import spectra from '../../../tools/spectrogram-fixture.json'

vi.mock('../../platform/audio/microphones', () => ({ listMicrophones: vi.fn().mockResolvedValue({ source: 'native', devices: [] }) }))

const ipc = vi.hoisted(() => ({ invoke: vi.fn(), fault: vi.fn() }))
const microphone = vi.hoisted(() => ({ transcribe: (_text: string) => {}, recording: false, spectrum: null as SpectrumFeed | null, lastTranscription: null as TranscriptionInspectionResult | null,
  pendingRecordings: [] as { recordingId: string; state: 'processing' | 'completed' | 'failed'; failure: unknown }[], retry: vi.fn() }))
const chrome = vi.hoisted(() => ({ getSettings: vi.fn(), saveSettings: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc.invoke }))
vi.mock('../../platform/diagnostics/faults', () => ({ reportFault: ipc.fault }))
vi.mock('../../platform/diagnostics/log', () => ({ logDiagnostic: vi.fn(), logInfo: vi.fn(), logWarn: vi.fn(), logError: vi.fn() }))
// Only peripheral presentation/hardware is replaced. The page, controller,
// workspace command builder and native snapshot projection run unchanged.
vi.mock('../../platform/ipc/tauri', () => ({
  isTauri: true, getSettings: chrome.getSettings, saveSettings: chrome.saveSettings,
  languages: () => [{ code: 'spanish', base: 'spanish', name: 'Spanish', endonym: 'Español' }, { code: 'english', base: 'english', name: 'English', endonym: 'English' }],
  languageFor: (code: string) => code === 'english' ? { code: 'english', name: 'English', endonym: 'English' } : { code: 'spanish', name: 'Spanish', endonym: 'Español' },
}))
vi.mock('../../platform/audio/reward-sounds', () => ({ configureRewardSounds: vi.fn(), stopRewardSounds: vi.fn() }))
vi.mock('../../platform/audio/useMicRecorder', () => ({ useMicRecorder: ({ onTranscribe }: { onTranscribe: (text: string, result: TranscriptionInspectionResult) => void }) => { microphone.transcribe = text => onTranscribe(text, microphone.lastTranscription ?? { inspection: { recordingId: 'recording' } } as TranscriptionInspectionResult); return { lastTranscription: microphone.lastTranscription, recording: microphone.recording, transcribing: false, waveSource: null, spectrum: microphone.spectrum, pendingRecordings: microphone.pendingRecordings, retry: microphone.retry, toggleMic: vi.fn(), cancel: vi.fn() } } }))
vi.mock('./coaching/CoachAnalysisPanel', () => ({ CoachAnalysisPanel: ({ coachingContent, tab }: { coachingContent: React.ReactNode; tab: string }) => tab === 'coaching' ? coachingContent : null }))
vi.mock('./progress/RewardPresentation', () => ({ RewardPresentationProvider: ({ children }: { children: React.ReactNode }) => children }))
vi.mock('./progress/XpChip', () => ({ XpChip: () => null }))
vi.mock('./progress/SkillRewards', () => ({ SkillRewards: () => null }))

import ConversationPage from './ConversationPage'
import { ReadingHelp } from '../../components/reading/ReadingHelp'
import { useConversation } from './session/useConversation'
import { useSettingsStore } from '../../state/settings/settings'
import { useSessionStore } from '../../state/session/session'
import { useNavigationStore } from '../../state/navigation/navigation'
import { useAiTrayStore } from '../../state/navigation/ai-tray'

/** Opens every message's ⋯ menu, where Words, Analysis and Pronunciation live. */
const openMenus = () => screen.queryAllByRole('button', { name: 'More actions' }).forEach(button => { if (button.getAttribute('aria-expanded') !== 'true') fireEvent.click(button) })


const SETTINGS: Settings = {
  my_languages: [], target_varieties: {},
  provider_mode: 'hosted',

  hosted_email: 'me@example.com',


  custom_base_url: '',

  custom_model: '',

  standard_model: 'google/gemini-2.5-flash',
  observer_model: null,
  target_language: 'spanish',
  target_variety: '',
  native_language: 'english', native_variety: 'english-united-states', interface_locale: 'english',
  microphone_device_id: null,
  auto_speak: false,
  auto_send: false,
  always_romanize: false,
  auto_translate: false,
  always_pronunciation: false,
  text_size: 100,
  text_spacing: 2,
  fast_mode: true, reward_sounds: 'follow_tts',
  master_volume: 100, voice_volume: 100, effects_volume: 100,
  tts_rate: 1,
  shortcuts: { mic: 'ctrl+m', speak: 'ctrl+l', panel: 'ctrl+b', settings: 'ctrl+,' },
}


function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
function directory(): Snapshot {
  return {
    savedTopics: [], sessionId: 'native-session', revision: 10,
    learner: { id: 'learner', name: '', revision: 1, preferences: { execution: { ...DEFAULT_EXECUTION }, appearance: { ...DEFAULT_APPEARANCE }, explanationVarietyId: 'english-united-states', interfaceLocale: 'english', myLanguages: [], targetVarieties: {}, theme: 'dark', explanationLanguage: 'english', textSize: 100, textSpacing: 2, highContrast: false, onboarding: 'completed', onboardingRequired: false, onboardingLanguage: null, onboardingHelp: false } },
    languages: [], languageProfiles: [], personas: [], contacts: [],
    conversations: ['a', 'b'].map((id, index) => ({
      id, contactId: 'contact', languageId: 'spanish', title: id, archived: false,
      revision: 7 + index, settingsRevision: 1, createdAt: '2026-09-10', lastUsed: 2 - index,
      settings: { direction: { topic: null, timeReference: 'any', usePersonaDetails: true }, difficulty: 'beginner', explanationLanguage: 'english', varietyId: '', explanationVarietyId: 'english-united-states', composingHelp: 'balanced', coachProactivity: 'on_request', translation: true, pronunciation: false, romanization: false, autoSend: true, readAloud: true, speechVoice: 'alloy' },
    })),
  }
}
function snapshot(id = 'a', revision = 1, text?: string): ConversationSnapshot {
  return {
    opening: null, topicChoices: [], starterGreeting: { text: 'hola', romanized: null }, revisionSuffixCounts: [], conversationId: id, contentRevision: String(revision), sessionId: 'native-session', revision, hasOlder: false,
    messages: text === undefined ? [] : [{ coachDecision: null, wordGloss: null, glossState: null, glossError: null, glossOperationId: null, turnId: `${id}-turn`, replacesTurnId: null, replacedBy: null, id: `${id}-source`, sequence: 1, role: 'user', text, createdAt: '2026-09-10', translation: null, translationState: null }],
    turns: [], coachMessages: [], transcriptionAttempts: [],
    connection: { route: 'hosted', signedIn: true, email: '', revision: 1, configured: true, assessmentAdapter: 'jev_choice' as const, standardModel: 'google/gemini-2.5-flash', fastModel: '', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false },
  }
}
let workspace: Snapshot
let watches: Array<{ conversationId: string; afterRevision: number } & ReturnType<typeof deferred<ConversationSnapshot>>>
let submit: (command: Command) => Promise<Receipt>
afterEach(async () => {
  cleanup()
  // Native long polls eventually settle. Release their test doubles too, so
  // shared in-flight reads cannot leak a never-settling request into another test.
  await act(async () => {
    for (const watch of watches) watch.resolve(snapshot(watch.conversationId))
  })
})
const resetView = vi.fn()
const setHistoryOpen = vi.fn()
function useSubject(settings = SETTINGS) { return useConversation({ settings, resetView, setHistoryOpen }) }
function commands(): Command[] {
  return ipc.invoke.mock.calls.filter(([name]) => name === 'execute_command').map(([, args]) => args.command as Command)
}
beforeEach(async () => {
  HTMLDialogElement.prototype.showModal = function () { this.open = true }
  HTMLDialogElement.prototype.close = function () { this.open = false }
  vi.clearAllMocks()
  microphone.lastTranscription = null
  microphone.pendingRecordings = []
  useSessionStore.setState({ ...useSessionStore.getInitialState(), connection: snapshot().connection })
  localStorage.clear()
  workspace = directory()
  watches = []
  submit = async command => ({ actionId: command.actionId, entityId: 'accepted', revision: 11 })
  chrome.getSettings.mockResolvedValue(SETTINGS)
  chrome.saveSettings.mockResolvedValue(undefined)
  // The page reads settings from the store, so seed it the way startup does.
  // Test setup resets every store between tests, so this is the first load.
  await useSettingsStore.getState().load()
  ipc.invoke.mockImplementation((name: string, args?: { conversationId: string; afterRevision: number; command: Command }) => {
    if (name === 'get_snapshot') return Promise.resolve(workspace)
    if (name === 'watch_conversation') {
      const pending = { conversationId: args!.conversationId, afterRevision: args!.afterRevision, ...deferred<ConversationSnapshot>() }
      watches.push(pending)
      return pending.promise
    }
    if (name === 'execute_command') return submit(args!.command)
    throw new Error(`Unexpected native command: ${name}`)
  })
})

describe('native conversation ownership', () => {
  it('starts a phrase conversation with the selected partner once, then opens the returned conversation', async () => {
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    const accepted = deferred<Receipt>()
    submit = command => command.action.kind === 'startPhraseConversation'
      ? accepted.promise : Promise.resolve({ actionId: command.actionId, entityId: 'new', revision: 12 })
    let started!: Promise<void>
    await act(async () => {
      started = result.current.startFromPhrase('source', 'cafe\u0301')
      void result.current.startFromPhrase('source', 'cafe\u0301')
    })
    expect(commands()).toHaveLength(1)
    expect(commands()[0].action).toEqual({ kind: 'startPhraseConversation', sourceMessageId: 'source', phrase: 'cafe\u0301', contactId: 'contact', expectedRevision: 10 })
    workspace = { ...workspace, conversations: [...workspace.conversations, { ...workspace.conversations[0], id: 'new' }] }
    await act(async () => { accepted.resolve({ actionId: commands()[0].actionId, entityId: 'new', revision: 11 }); await started })
    expect(commands()[1].action).toEqual({ kind: 'openConversation', conversationId: 'new' })
    expect(result.current.currentChatId).toBe('new')
  })

  it('routes skill starts through the current conversation and attaches guide questions to its coach', async () => {
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    const guide = { language: 'spanish', variety: 'spanish-spain', skillId: 'time_events', editionLanguage: 'english', fingerprint: 'source' }
    await act(async () => { await result.current.runGuideAction({ kind: 'coach', guide, text: 'Explain this skill.' }) })
    expect(commands()[0].action).toEqual({ kind: 'askGuideCoach', conversationId: 'a', text: 'Explain this skill.', guide, expectedRevision: workspace.conversations[0].revision })
    expect(result.current.currentChatId).toBe('a')
    submit = async command => {
      if (command.action.kind === 'startSkillConversation') workspace = { ...workspace, conversations: [...workspace.conversations, { ...workspace.conversations[0], id: 'skill-chat' }] }
      return { actionId: command.actionId, entityId: 'skill-chat', revision: 12 }
    }
    await act(async () => { await result.current.startFromSkill('spanish', 'spanish-spain', 'time_events') })
    expect(commands()[1].action).toEqual({ kind: 'startSkillConversation', sourceConversationId: 'a', language: 'spanish', variety: 'spanish-spain', skillId: 'time_events', subskillId: null, expectedRevision: 10 })
    expect(result.current.currentChatId).toBe('skill-chat')
  })

  it('mount, StrictMode replay, preferences and remount observe without greeting or saving', async () => {
    const view = renderHook(({ settings }) => useSubject(settings), { initialProps: { settings: SETTINGS }, wrapper: StrictMode })
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    await waitFor(() => expect(watches).toHaveLength(2))
    view.rerender({ settings: { ...SETTINGS, auto_translate: true, text_size: 140, always_pronunciation: true } })
    await act(async () => {})
    expect(watches).toHaveLength(2)
    view.unmount()
    renderHook(() => useSubject(), { wrapper: StrictMode })
    await waitFor(() => expect(watches).toHaveLength(3))
    expect(commands()).toEqual([])
    expect(new Set(ipc.invoke.mock.calls.map(([name]) => name))).toEqual(new Set(['get_snapshot', 'watch_conversation']))
  })

  it('Send uses fresh native session/revision once and waits for the durable snapshot', async () => {
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    workspace = { ...workspace, sessionId: 'current-session', conversations: workspace.conversations.map(c => ({ ...c, revision: 42 })) }
    await act(async () => result.current.sendMessage('  Sí, 你好 👩🏽‍💻\n'))
    expect(commands()).toHaveLength(1)
    expect(commands()[0]).toEqual({ sessionId: 'current-session', actionId: expect.any(String), action: { kind: 'sendMessage', input: { modality: 'text', suggestion: false, scaffold: false, revision: false }, conversationId: 'a', expectedRevision: 42, text: '  Sí, 你好 👩🏽‍💻\n' } })
    expect(commands()[0].actionId).not.toBe('')
    expect(result.current.turns).toEqual([])
    await act(async () => watches[0].resolve(snapshot('a', 12, 'Durably accepted')))
    expect(result.current.turns[0].user).toBe('Durably accepted')
    expect(commands()).toHaveLength(1)
  })

  it('ignores an old watcher after explicit conversation switch', async () => {
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    const old = watches[0]
    await act(async () => result.current.openChat('b'))
    await waitFor(() => expect(watches).toHaveLength(2))
    expect(commands().map(c => c.action)).toEqual([{ kind: 'openConversation', conversationId: 'b' }])
    await act(async () => watches[1].resolve(snapshot('b', 5, 'Current conversation')))
    await waitFor(() => expect(watches).toHaveLength(3))
    await act(async () => old.resolve(snapshot('a', 999, 'Stale result')))
    expect(result.current.currentChatId).toBe('b')
    expect(result.current.turns.map(t => t.user)).toEqual(['Current conversation'])
    expect(watches.filter(w => w.conversationId === 'a')).toHaveLength(1)
    expect(watches[2].afterRevision).toBe(5)
  })

  it.each(['resolve', 'reject'] as const)('ignores watcher %s after unmount without restarting it', async outcome => {
    const view = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    view.unmount()
    await act(async () => {
      if (outcome === 'resolve') watches[0].resolve(snapshot('a', 99, 'Late'))
      else watches[0].reject(new Error('Late watch failure'))
    })
    expect(watches).toHaveLength(1)
    expect(commands()).toEqual([])
    expect(ipc.fault).not.toHaveBeenCalled()
  })

  it('rejects wrong-scope snapshots without publication or another watch', async () => {
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot('b', 7, 'Wrong owner')))
    expect(result.current.turns).toEqual([])
    expect(result.current.readError).toContain('Conversation snapshot scope mismatch.')
    expect(watches).toHaveLength(1)
    expect(commands()).toEqual([])
  })

  it('propagates Send failure without optimistic messages, saves or automatic retries', async () => {
    submit = async () => { throw new Error('Admission refused') }
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => { await expect(result.current.sendMessage('Keep my draft')).rejects.toThrow('Admission refused') })
    expect(commands()).toHaveLength(1)
    expect(result.current.turns).toEqual([])
  })

  it('assisting does not block the next reply, but pending does', async () => {
    const { result } = renderHook(() => useSubject())
    await waitFor(() => expect(watches).toHaveLength(1))
    const assisting = snapshot()
    assisting.turns = [{ id: 'turn', replacesTurnId: null, replacedBy: null, route: 'hosted', state: 'assisting', paused: false, hold: null,   }]
    await act(async () => watches[0].resolve(assisting))
    expect(result.current.pendingReply).toBe(false)
    await act(async () => watches[1].resolve({ ...assisting, revision: 2, contentRevision: '2', turns: [{ ...assisting.turns[0], state: 'pending' }] }))
    expect(result.current.pendingReply).toBe(true)
    expect(commands()).toEqual([])
  })
})

/** The draft opens from Type, or by itself once there is text in it. */
async function draftField() {
  const existing = screen.queryByPlaceholderText(/Write in/)
  if (existing) return existing
  fireEvent.click(await screen.findByRole('button', { name: 'Type' }))
  return screen.getByPlaceholderText(/Write in/)
}
function page() {
  return <ConversationPage nativePicker={null} mobileSurface="chat" active />
}
it('retains the real draft, reading position and phone coach selection while the conversation is hidden', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: true, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  try {
    useNavigationStore.getState().openConversation('panel')
    const view = render(<ConversationPage nativePicker={null} mobileSurface="panel" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot('a', 1, 'Hola')))
    const draft = await draftField()
    fireEvent.change(draft, { target: { value: 'Keep this draft' } })
    const stream = document.querySelector<HTMLDivElement>('.stream')!
    Object.defineProperty(stream, 'scrollHeight', { configurable: true, value: 1600 })
    Object.defineProperty(stream, 'clientHeight', { configurable: true, value: 400 })
    stream.scrollTop = 120
    fireEvent.scroll(stream)

    view.rerender(<ConversationPage nativePicker={null} mobileSurface="panel" active={false} />)
    // Hidden layout and scroll events must not overwrite the saved reading position.
    Object.defineProperty(stream, 'scrollHeight', { configurable: true, value: 0 })
    Object.defineProperty(stream, 'clientHeight', { configurable: true, value: 0 })
    stream.scrollTop = 0
    fireEvent.scroll(stream)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(useNavigationStore.getState().mobileSurface).toBe('panel')
    Object.defineProperty(stream, 'scrollHeight', { configurable: true, value: 1600 })
    Object.defineProperty(stream, 'clientHeight', { configurable: true, value: 400 })
    view.rerender(<ConversationPage nativePicker={null} mobileSurface="panel" active />)
    expect(document.querySelector('.stream')).toBe(stream)
    expect(stream.scrollTop).toBe(120)
    expect(draft).toHaveValue('Keep this draft')
    expect(document.querySelector('.split.mobile-coach')).not.toBeNull()
    view.unmount()
  } finally { media.mockRestore() }
})
it('opens the conversation list from the chat header and lets the coach cover the chat on phones', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: true, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  const onHistory = vi.fn()
  useNavigationStore.getState().openConversation('panel')
  const view = render(<ConversationPage nativePicker={null} mobileSurface="panel" active onHistoryOpenChange={onHistory} />)
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot()))
  fireEvent.click(screen.getByRole('button', { name: 'Conversations' }))
  expect(onHistory).toHaveBeenCalledWith(true)
  // Narrow: the coach opens from the row just above the answer, not the header, and
  // covers the conversation, which stays mounted beneath it.
  const coach = screen.getByRole('button', { name: 'Coach' })
  expect(coach).toHaveAttribute('aria-expanded', 'true')
  expect(coach.closest('.composer-assist')).not.toBeNull()
  expect(document.querySelector('.chat-head .chat-coach, .chat-coach-edge')).toBeNull()
  expect(document.querySelector('.split.mobile-coach .coach-scrim')).not.toBeNull()
  expect((await draftField())).toBeInTheDocument()
  fireEvent.keyDown(window, { key: 'Escape' })
  expect(useNavigationStore.getState().mobileSurface).toBe('chat')
  view.unmount()
  media.mockRestore()
})
it('keeps difficulty in the one-row header while it fits there at any window width, and moves it to the settings sheet only while the row has no room', async () => {
  // A phone-width window whose header still has room: the select stays beside the partner.
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: query === '(max-width: 860px)', media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  try {
    const roomy = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    const head = document.querySelector<HTMLElement>('.chat-head')!
    expect(head).toHaveAttribute('data-fit', 'full')
    expect(within(head).getByRole('combobox', { name: 'Difficulty' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Conversation settings' }))
    expect(within(screen.getByRole('dialog', { name: 'Conversation settings' })).queryByRole('region', { name: 'Difficulty' })).toBeNull()
    roomy.unmount()
  } finally {
    media.mockRestore()
  }
  // A wide window whose header is too short for the partner and the select side
  // by side (the coach panel takes the rest): the select leads the settings sheet.
  const width = vi.spyOn(Element.prototype, 'scrollWidth', 'get').mockImplementation(function (this: Element) {
    return this.matches('.conversation-identity') && !this.closest('.chat-head')?.getAttribute('data-fit')?.includes('difficulty-in-settings') ? 400 : 0
  })
  try {
    const tight = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(2))
    await act(async () => watches[1].resolve(snapshot()))
    expect(document.querySelector('.chat-head')).toHaveAttribute('data-fit', 'difficulty-in-settings')
    fireEvent.click(screen.getByRole('button', { name: 'Conversation settings' }))
    const group = within(screen.getByRole('dialog', { name: 'Conversation settings' })).getByRole('region', { name: 'Difficulty' })
    expect(within(group).getByRole('combobox', { name: 'Difficulty' })).toBeVisible()
    expect(group.parentElement!.firstElementChild).toBe(group)
    tight.unmount()
  } finally {
    width.mockRestore()
  }
})
it('puts the recording panel’s divider on the panel’s own edge, with the row above it outside the panel', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: true, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  const view = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot()))
  const divider = screen.getByRole('separator', { name: 'Resize the recording panel' })
  // The divider is the recording panel's top edge: the panel follows it directly…
  expect(divider.nextElementSibling).toHaveClass('composer-voice')
  // …and the row of reply help, status and coach stays above it, with the conversation.
  const assist = document.querySelector('.composer-assist')!
  expect(assist.compareDocumentPosition(divider) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  view.unmount()
  media.mockRestore()
})
it('opens the coach on phones as a full-screen modal, over the recorder, without moving focus into it', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: true, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  try {
    useNavigationStore.getState().openConversation('chat')
    const view = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    const coach = screen.getByRole('button', { name: 'Coach' })
    coach.focus()
    fireEvent.click(coach)
    view.rerender(<ConversationPage nativePicker={null} mobileSurface="panel" active />)
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 50)) })
    const modal = document.querySelector<HTMLElement>('.split.mobile-coach > .break')!
    // A modal of its own, with its own chat box: nothing to resize, and not a panel inside the conversation.
    expect(modal).not.toBeNull()
    expect(modal.closest('section.chat')).toBeNull()
    expect(modal.querySelector('[role=separator][aria-label="Resize the coach panel"]')).toBeNull()
    expect(document.querySelector('.split.mobile-coach > .coach-scrim')).not.toBeNull()
    // Opening moves no focus into it: nothing scrolls to reveal a field, and no keyboard opens.
    expect(modal.contains(document.activeElement)).toBe(false)
    view.unmount()
  } finally {
    media.mockRestore()
  }
})
it('gives the AI tray its place on phones, right above the AI pill, and takes it back under the coach', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: true, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  try {
    useNavigationStore.getState().openConversation('chat')
    const view = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    const slot = useAiTrayStore.getState().slot!
    expect(slot).not.toBeNull()
    expect(slot.parentElement).toHaveClass('composer-assist')
    expect(slot.nextElementSibling).toHaveClass('composer-activity')
    // The full-screen coach covers the recording panel; a tray there would be hidden.
    view.rerender(<ConversationPage nativePicker={null} mobileSurface="panel" active />)
    expect(useAiTrayStore.getState().slot).toBeNull()
    view.rerender(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    expect(useAiTrayStore.getState().slot).toBe(slot)
    // A conversation kept behind another page does not hold it.
    view.rerender(<ConversationPage nativePicker={null} mobileSurface="chat" active={false} />)
    expect(useAiTrayStore.getState().slot).toBeNull()
    view.unmount()
  } finally {
    media.mockRestore()
  }
})
it('lets opened reply help on phones be dragged to a height it keeps', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: true, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  try {
    localStorage.removeItem('skellyspeak_pane_reply-help')
    useNavigationStore.getState().openConversation('chat')
    const view = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(exchangeSnapshot()))
    const assist = document.querySelector<HTMLElement>('.composer-assist')!
    fireEvent.click(await within(assist).findByRole('button', { name: 'Help with this reply' }))
    const tray = assist.querySelector<HTMLElement>('.reply-help')!
    vi.spyOn(tray, 'getBoundingClientRect').mockReturnValue({ height: 200 } as DOMRect)
    fireEvent.keyDown(within(assist).getByRole('separator', { name: 'Resize the reply help panel' }), { key: 'ArrowUp' })
    expect(assist).toHaveAttribute('data-help-sized')
    expect(assist.style.getPropertyValue('--reply-help-height')).toBe('216px')
    expect(localStorage.getItem('skellyspeak_pane_reply-help')).toBe('216')
    view.unmount()
  } finally {
    media.mockRestore()
    localStorage.removeItem('skellyspeak_pane_reply-help')
  }
})
it('opens the coach from the row above the answer in the compact layout, as on phones', async () => {
  const media = vi.spyOn(window, 'matchMedia').mockImplementation(query => ({ matches: query === '(max-width: 860px)', media: query, addEventListener: vi.fn(), removeEventListener: vi.fn() }) as unknown as MediaQueryList)
  try {
    useNavigationStore.getState().openConversation('chat')
    const view = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    // Once the docked panel no longer fits, there is one way to the coach at
    // every width: the button in the row above the answer, not a tab on the
    // conversation's edge.
    const coach = screen.getByRole('button', { name: 'Coach' })
    expect(coach).toHaveClass('chat-coach')
    expect(coach.closest('.composer-assist')).not.toBeNull()
    expect(document.querySelector('.chat-coach-edge')).toBeNull()
    expect(coach).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(coach)
    expect(useNavigationStore.getState().mobileSurface).toBe('panel')
    view.rerender(<ConversationPage nativePicker={null} mobileSurface="panel" active />)
    expect(screen.getByRole('button', { name: 'Coach' })).toHaveAttribute('aria-expanded', 'true')
    expect(document.querySelector('.split.mobile-coach .coach-scrim')).not.toBeNull()
    view.unmount()
  } finally {
    media.mockRestore()
  }
})
it('keeps a dragged recording panel height and draws the live spectrogram while recording', async () => {
  const fixture = (await import('../../../tools/spectrogram-fixture.json')).default[0].spectrogram
  const feed = createSpectrumFeed()
  feed.set({ endSeconds: 1, data: fixture } as never)
  microphone.recording = true
  microphone.spectrum = feed
  try {
    const view = render(<ConversationPage nativePicker={null} mobileSurface="chat" active />)
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    // The stream is the same waveform-over-spectrogram as Practice.
    const face = document.querySelector('.composer .voice-face')!
    expect(face.querySelector('.live-recording')).not.toBeNull()
    expect(face.querySelector('.inspection-spectrogram')).not.toBeNull()
    // The divider above the panel sets its height and keeps it.
    const divider = screen.getByRole('separator', { name: 'Resize the recording panel' })
    const composer = divider.closest('.composer')!
    expect(composer).not.toHaveAttribute('data-voice-sized')
    fireEvent.keyDown(divider, { key: 'ArrowUp' })
    expect(composer).toHaveAttribute('data-voice-sized')
    expect((composer as HTMLElement).style.getPropertyValue('--chat-voice-height')).toBe('150px')
    expect(localStorage.getItem('skellyspeak_pane_chat-voice')).toBe('150')
    // Recording inspection opens from the message bubble, not a button above the panel.
    expect(screen.queryByRole('button', { name: 'Inspect recording' })).toBeNull()
    view.unmount()
  } finally {
    microphone.recording = false
    microphone.spectrum = null
  }
})

it('offers AI access settings alongside hosted sign-in on an unconfigured conversation', async () => {
  useSessionStore.setState({ connection: {
    route: 'custom', signedIn: false, email: '', revision: 1,
    configured: false, assessmentAdapter: 'jev_choice' as const, standardModel: 'standard', fastModel: 'fast', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false,
  } })
  const openSettings = vi.fn()
  render(<ConversationPage nativePicker={null} mobileSurface="chat" active onOpenSettings={openSettings} />)
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot()))
  expect(screen.getByText('Choose how to connect to AI.')).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Sign in with Google' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Or set up AI access in another way' }))
  expect(openSettings).toHaveBeenCalledOnce()
})
describe('native composer admission', () => {
  it.each(['Enter', 'Send'])('types into the extracted composer and submits once with %s', async (action) => {
    const user = userEvent.setup()
    const pending = deferred<Receipt>()
    submit = () => pending.promise
    render(page())
    await waitFor(() => expect(watches).toHaveLength(1))
    const composer = await draftField()
    await user.click(composer)
    await user.type(composer, 'Hola, ¿cómo estás?')
    expect(composer).toHaveFocus()
    expect(composer).toHaveValue('Hola, ¿cómo estás?')
    expect(commands()).toHaveLength(0)
    if (action === 'Enter') await user.keyboard('{Enter}')
    else await user.click(screen.getByRole('button', { name: 'Send' }))
    await waitFor(() => expect(commands()).toHaveLength(1))
    expect(commands()[0].action).toEqual({ kind: 'sendMessage', input: { modality: 'text', suggestion: false, scaffold: false, revision: false }, conversationId: 'a', expectedRevision: 7, text: 'Hola, ¿cómo estás?' })
    await act(async () => pending.reject(new Error('Authentication failed')))
    // The rejected message keeps its bubble, with Retry; the draft stays clear.
    expect(document.querySelector('.stream .msg.me')).toHaveTextContent('Hola, ¿cómo estás?')
    expect(composer).toHaveValue('')
    expect(screen.getByRole('button', { name: 'Retry' })).toBeEnabled()
    expect(commands()).toHaveLength(1)
  })

  it('preserves a newer draft when an earlier Send fails', async () => {
    const pending = deferred<Receipt>()
    submit = () => pending.promise
    render(page())
    await waitFor(() => expect(watches).toHaveLength(1))
    const composer = await draftField()
    fireEvent.change(composer, { target: { value: 'Earlier attempted message' } })
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    await waitFor(() => expect(commands()).toHaveLength(1))
    fireEvent.change(composer, { target: { value: 'New draft typed while waiting' } })
    await act(async () => pending.reject(new Error('Admission refused')))
    expect(composer).toHaveValue('New draft typed while waiting')
    expect(commands()).toHaveLength(1)
    const retry = deferred<Receipt>()
    submit = () => retry.promise
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(commands()).toHaveLength(2))
    expect(composer).toHaveValue('New draft typed while waiting')
    await act(async () => retry.resolve({ actionId: commands()[1].actionId, entityId: 'accepted', revision: 11 }))
    expect(composer).toHaveValue('New draft typed while waiting')
  })

  it('settings refresh and empty conversation hydration do not send an automatic greeting', async () => {
    render(page())
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(snapshot()))
    chrome.getSettings.mockResolvedValue({ ...SETTINGS, auto_translate: true, text_size: 150 })
    // A settings write landed elsewhere: the revision moves and the page re-reads
    // through the store rather than through a prop it was handed.
    await act(async () => { await useSettingsStore.getState().refresh() })
    expect(useSettingsStore.getState().revision).toBe(1)
    expect(useSettingsStore.getState().settings?.text_size).toBe(150)
    expect(commands()).toEqual([])
    expect(chrome.saveSettings).not.toHaveBeenCalled()
  })

  it('submits one command and keeps a rejected message in its bubble with Retry, which sends it again', async () => {
    const pending = deferred<Receipt>()
    submit = () => pending.promise
    render(page())
    await waitFor(() => expect(watches).toHaveLength(1))
    const composer = await draftField()
    fireEvent.change(composer, { target: { value: 'Keep this unsent text' } })
    const button = screen.getByRole('button', { name: 'Send' })
    fireEvent.click(button)
    fireEvent.click(button)
    await waitFor(() => expect(commands()).toHaveLength(1))
    await act(async () => pending.reject(new Error('Admission refused')))
    const failed = document.querySelector('.stream .msg.me') as HTMLElement
    expect(failed).toHaveTextContent('Keep this unsent text')
    expect(composer).toHaveValue('')
    expect(await screen.findByRole('alert')).toHaveTextContent('Request failed')
    expect(screen.getByText('Admission refused')).toBeVisible()
    expect(commands()[0].action).toEqual({ kind: 'sendMessage', input: { modality: 'text', suggestion: false, scaffold: false, revision: false }, conversationId: 'a', expectedRevision: 7, text: 'Keep this unsent text' })
    expect(commands()).toHaveLength(1)
    submit = async command => ({ actionId: command.actionId, entityId: 'accepted', revision: 11 })
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(commands()).toHaveLength(2))
    expect(commands()[1].action).toMatchObject({ kind: 'sendMessage', text: 'Keep this unsent text' })
    expect(document.querySelectorAll('.stream .msg.me')).toHaveLength(1)
  })

  it('shows the learner’s bubble, then the partner’s pending bubble, the moment Send is pressed', async () => {
    const pending = deferred<Receipt>()
    submit = () => pending.promise
    render(page())
    await waitFor(() => expect(watches).toHaveLength(1))
    await act(async () => watches[0].resolve(exchangeSnapshot()))
    const composer = await draftField()
    fireEvent.change(composer, { target: { value: 'Fui al mercado.' } })
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    // Held open before native storage has the message: the learner's text grows
    // into place, and the partner's bubble waits under it.
    const mine = [...document.querySelectorAll<HTMLElement>('.stream .msg.me')].at(-1)!
    expect(mine).toHaveTextContent('Fui al mercado.')
    expect(mine).toHaveAttribute('aria-busy', 'true')
    expect(mine).toHaveAttribute('data-arriving')
    const reply = document.querySelector('.stream .msg.bot.reply-pending') as HTMLElement
    expect(reply).toHaveTextContent('Replying…')
    expect(reply).toHaveAttribute('data-arriving')
    expect(composer).toHaveValue('')
    await waitFor(() => expect(commands()).toHaveLength(1))
    await act(async () => pending.resolve({ actionId: commands()[0].actionId, entityId: 'accepted', revision: 32 }))
    // Native storage has it: the landed turn takes the same place, without growing again.
    const landed = exchangeSnapshot()
    landed.revision = 32
    landed.contentRevision = '32'
    landed.messages.push({ ...landed.messages[0], id: 'sent', turnId: 'accepted', sequence: 3, text: 'Fui al mercado.' })
    landed.turns = [{ id: 'accepted', replacesTurnId: null, replacedBy: null, route: 'hosted', state: 'pending', paused: false, hold: null,
       }]
    await act(async () => watches[1].resolve(landed))
    const sent = [...document.querySelectorAll<HTMLElement>('.stream .msg.me')].filter(bubble => bubble.textContent?.includes('Fui al mercado.'))
    expect(sent).toHaveLength(1)
    expect(sent[0]).not.toHaveAttribute('aria-busy')
    const replies = document.querySelectorAll('.stream .msg.bot.reply-pending')
    expect(replies).toHaveLength(1)
    expect(replies[0]).not.toHaveAttribute('data-arriving')
  })
})

it('shows an edit in the bubble being edited, pending, the moment it is sent', async () => {
  const pending = deferred<Receipt>()
  submit = () => pending.promise
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(exchangeSnapshot()))
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Edit message' }))
  fireEvent.change((await draftField()), { target: { value: 'Yo fui ayer' } })
  const editingShell = document.querySelector('.turn-stack[data-editing]')
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  const bubbles = document.querySelectorAll('.stream .msg.me')
  expect(bubbles).toHaveLength(1)
  expect(bubbles[0]).toHaveTextContent('Yo fui ayer')
  expect(bubbles[0]).toHaveAttribute('aria-busy', 'true')
  expect(document.querySelector('.turn-stack[data-editing]')).toBe(editingShell)
  expect(bubbles[0]).not.toHaveAttribute('data-arriving')
  // In the conversation, the reply the edit replaces gives way to the partner's pending bubble.
  const stream = document.querySelector('.stream') as HTMLElement
  expect(within(stream).queryByText('¿Adónde fuiste?')).toBeNull()
  expect(document.querySelector('.stream .msg.bot.reply-pending')).toHaveTextContent('Replying…')
  await waitFor(() => expect(commands()).toHaveLength(1))
  // A rejected revision keeps the edit in the composer and restores the original.
  await act(async () => pending.reject({ code: 'connection', message: 'The connection is unavailable.' }))
  expect(document.querySelector('.stream .msg.me')).toHaveTextContent('Yo fue ayer')
  expect(within(stream).getByText('¿Adónde fuiste?')).toBeInTheDocument()
  expect((await draftField())).toHaveValue('Yo fui ayer')
})

it('auto-sends one native transcript and retains a later transcript while a reply is pending', async () => {
  const pending = deferred<Receipt>()
  submit = () => pending.promise
  chrome.getSettings.mockResolvedValue({ ...SETTINGS, auto_send: true })
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const input = await draftField()
  act(() => microphone.transcribe('Primera frase'))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(commands()[0].action).toMatchObject({ kind: 'sendMessage', text: 'Primera frase' })
  act(() => microphone.transcribe('Guardar esta frase'))
  expect(input).toHaveValue('Guardar esta frase')
  expect(commands()).toHaveLength(1)
  await act(async () => pending.reject(new Error('Rejected')))
  expect(input).toHaveValue('Guardar esta frase')
})

it('holds the learner’s bubble from the moment an Auto-send recording stops, then fills in its text', async () => {
  const pending = deferred<Receipt>()
  submit = () => pending.promise
  chrome.getSettings.mockResolvedValue({ ...SETTINGS, auto_send: true })
  const view = render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(exchangeSnapshot()))
  microphone.pendingRecordings = [{ recordingId: 'take', state: 'processing', failure: null }]
  view.rerender(page())
  const held = [...document.querySelectorAll<HTMLElement>('.stream .msg.me')].at(-1)!
  expect(held).toHaveAttribute('aria-busy', 'true')
  expect(held.querySelector('.reply-placeholder')).not.toBeNull()
  expect(held).toHaveTextContent('Transcribing…')
  // No reply is requested before the message has its text.
  expect(document.querySelector('.stream .msg.bot.reply-pending')).toBeNull()
  microphone.pendingRecordings = [{ recordingId: 'take', state: 'completed', failure: null }]
  act(() => microphone.transcribe('Fui al mercado.'))
  const filled = [...document.querySelectorAll<HTMLElement>('.stream .msg.me')].at(-1)!
  expect(filled).toBe(held)
  expect(filled).toHaveTextContent('Fui al mercado.')
  expect(document.querySelector('.stream .msg.bot.reply-pending')).toHaveTextContent('Replying…')
  await waitFor(() => expect(commands()).toHaveLength(1))
})

it('keeps a failed recording in its bubble with Retry, which sends its audio again', async () => {
  chrome.getSettings.mockResolvedValue({ ...SETTINGS, auto_send: true })
  const view = render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(exchangeSnapshot()))
  microphone.pendingRecordings = [{ recordingId: 'take', state: 'processing', failure: null }]
  view.rerender(page())
  microphone.pendingRecordings = [{ recordingId: 'take', state: 'failed', failure: new Error('Transcription provider unavailable') }]
  view.rerender(page())
  const failed = [...document.querySelectorAll<HTMLElement>('.stream .msg.me')].at(-1)!
  expect(failed.querySelector('.reply-placeholder')).not.toBeNull()
  expect(await screen.findByRole('alert')).toHaveTextContent('Request failed')
  expect(screen.queryByRole('button', { name: 'Record again' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
  await waitFor(() => expect(microphone.retry).toHaveBeenCalledExactlyOnceWith('take'))
})

it('binds recorded audio to the accepted turn even when a later message repeats its text', async () => {
  chrome.getSettings.mockResolvedValue({ ...SETTINGS, auto_send: true })
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot('a', 1, 'Earlier')))
  microphone.lastTranscription = { text: 'Hola', audioBase64: '', diagnostics: null,
    inspection: { ...spectra[0], recordingId: 'recording', owner: { kind: 'conversation', id: 'a' } } } as TranscriptionInspectionResult
  act(() => microphone.transcribe('Hola'))
  await waitFor(() => expect(commands().some(command => command.action.kind === 'sendMessage')).toBe(true))
  const next = snapshot('a', 2, 'Hola')
  next.messages[0].turnId = 'accepted'
  next.messages.push({ ...next.messages[0], id: 'later-source', turnId: 'later', sequence: 2, text: 'Hola again' })
  await act(async () => watches[1].resolve(next))
  const bubbles = document.querySelectorAll('.stream .msg.me')
  expect(bubbles).toHaveLength(2)
  expect(within(bubbles[0] as HTMLElement).getByRole('button', { name: 'Inspect recording' })).toBeEnabled()
  expect(within(bubbles[1] as HTMLElement).queryByRole('button', { name: 'Inspect recording' })).toBeNull()
})

function exchangeSnapshot(earlier = false): ConversationSnapshot {
  const value = snapshot('a', 31, 'Yo fue ayer')
  value.messages.push({ ...value.messages[0], id: 'reply', sequence: 2, role: 'assistant', text: '¿Adónde fuiste?' })
  value.revisionSuffixCounts = [{ turnId: 'a-turn', exchangeCount: earlier ? 1 : 0, coachTurnCount: earlier ? 2 : 0 }]
  return value
}

it('edits through the real page handler, sends durable identity and renders retained wording', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const initial = exchangeSnapshot()
  await act(async () => watches[0].resolve(initial))
  const presentationSlot = document.querySelector('.stream .turn-stack')
  openMenus()
  const edit = screen.getByRole('button', { name: 'Edit message' })
  expect(edit).toBeEnabled()
  fireEvent.click(within(edit.closest('.msg') as HTMLElement).getByRole('button', { name: 'Coach' }))
  fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Edit and resend message' }))
  const composer = (await draftField())
  expect(composer).toHaveValue('Yo fue ayer')
  fireEvent.change(composer, { target: { value: 'Yo fui ayer' } })
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(commands()).toHaveLength(3))
  expect(commands().filter(command => command.action.kind === 'requestMessageHelp').map(command => command.action)).toEqual([
    { kind: 'requestMessageHelp', messageId: initial.messages[0].id, help: 'assessment', retry: false },
    { kind: 'requestMessageHelp', messageId: initial.messages[0].id, help: 'coaching', retry: false },
  ])
  expect(screen.getByText('Edit saved — updating conversation…')).toBeVisible()
  expect(commands().find(command => command.action.kind === 'reviseTurn')?.action).toEqual({ kind: 'reviseTurn', conversationId: 'a', turnId: 'a-turn', text: 'Yo fui ayer', expectedRevision: 31, input: { modality: 'text', suggestion: false, scaffold: false, revision: true } })
  const revised: ConversationSnapshot = { ...initial, revision: 32, contentRevision: '32', messages: [
    ...initial.messages.map(message => ({ ...message, replacedBy: 'repair' })),
    { ...initial.messages[0], turnId: 'repair', replacesTurnId: 'a-turn', id: 'repair-user', sequence: 3, text: 'Yo fui ayer' },
    { ...initial.messages[1], turnId: 'repair', replacesTurnId: 'a-turn', id: 'repair-reply', sequence: 4, text: '¿Qué hiciste allí?' },
  ], revisionSuffixCounts: [{ turnId: 'repair', exchangeCount: 0, coachTurnCount: 0 }] }
  // The edited wording must replace the bubble as soon as native storage accepts
  // it, before a regenerated partner reply or word help has arrived.
  const pendingRevision = { ...revised, messages: revised.messages.filter(message => message.id !== 'repair-reply') }
  await act(async () => watches[1].resolve(pendingRevision))
  expect(document.querySelector('.stream .turn-stack')).toBe(presentationSlot)
  expect(screen.getAllByText('Yo fui ayer').some(element => element.closest('.msg.me'))).toBe(true)
  expect(screen.queryByText('Edit saved — updating conversation…')).not.toBeInTheDocument()
  expect(screen.queryByText('Yo fue ayer')).not.toBeInTheDocument()
  expect(screen.queryByText('¿Adónde fuiste?')).not.toBeInTheDocument()
  await waitFor(() => expect(watches).toHaveLength(3))
  await act(async () => watches[2].resolve({ ...revised, revision: 33, contentRevision: '33' }))
  expect(document.querySelector('.stream .turn-stack')).toBe(presentationSlot)
  expect(screen.getAllByText('Yo fui ayer').some(element => element.closest('.msg.me'))).toBe(true)
  expect(screen.queryByText('Edit saved — updating conversation…')).not.toBeInTheDocument()
  expect(screen.queryByText('Yo fue ayer')).not.toBeInTheDocument()
  expect(screen.queryByText('Earlier version')).not.toBeInTheDocument()
  openMenus()
  expect(screen.getAllByRole('button', { name: 'Edit message' })).toHaveLength(1)
})

it('confirms native suffix scope and retains the edit draft after a stale admission', async () => {
  submit = async () => { throw { code: 'conflict', message: 'Conversation changed; review the revision again.' } }
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(exchangeSnapshot(true)))
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Edit message' }))
  const composer = (await draftField())
  fireEvent.change(composer, { target: { value: 'Yo fui ayer' } })
  const stream = document.querySelector('.stream') as HTMLElement
  Object.defineProperty(stream, 'scrollHeight', { configurable: true, value: 1600 })
  Object.defineProperty(stream, 'clientHeight', { configurable: true, value: 400 })
  stream.scrollTop = 100
  fireEvent.scroll(stream)
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  expect(screen.getByRole('dialog', { name: 'Revise earlier message' })).toHaveTextContent('1 later conversation turns. Your edited message replaces the original; private coach history is kept.')
  expect(commands()).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'Revise and remove later turns' }))
  expect(stream.scrollTop).toBe(100)
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(composer).toHaveValue('Yo fui ayer')
  expect(await screen.findByRole('alert')).toHaveTextContent('Request failed')
  expect(commands()[0].action).toMatchObject({ expectedRevision: 31 })
})

it('allows editing and replacing a pending partner reply', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const value = exchangeSnapshot()
  value.turns = [{ id: 'pending', replacesTurnId: null, replacedBy: null, route: 'hosted', state: 'pending', paused: false, hold: null,   }]
  await act(async () => watches[0].resolve(value))
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Edit message' }))
  expect(screen.getByRole('button', { name: 'Send' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(commands()[0].action.kind).toBe('reviseTurn')
})

it.each(['resolve', 'reject'] as const)('ignores late revision %s after switching conversations', async outcome => {
  const pending = deferred<Receipt>()
  submit = async command => command.action.kind === 'reviseTurn' ? pending.promise : { actionId: command.actionId, entityId: 'b', revision: 40 }
  let newChat: (() => void) | null = null
  render(<ConversationPage nativePicker={null} mobileSurface="chat" active onNewChatReady={action => { if (action) newChat = action }} />)
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(exchangeSnapshot()))
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Edit message' }))
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  await act(async () => newChat!())
  await waitFor(() => expect(watches.some(watch => watch.conversationId === 'b')).toBe(true))
  const composer = (await draftField())
  fireEvent.change(composer, { target: { value: 'New conversation draft' } })
  await act(async () => outcome === 'resolve' ? pending.resolve({ actionId: 'revision', entityId: 'repair', revision: 41 }) : pending.reject(new Error('Old conversation failed')))
  expect(composer).toHaveValue('New conversation draft')
  expect(screen.queryByRole('alert')).toBeNull()
})

it('retains a repair draft after a genuine connection failure', async () => {
  submit = async () => { throw { code: 'connection', message: 'The connection is unavailable.' } }
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(exchangeSnapshot()))
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Edit message' }))
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(await screen.findByText('The connection is unavailable.')).toBeVisible()
  expect((await draftField())).toHaveValue('Yo fue ayer')
  expect(commands()).toHaveLength(1)
})

it('starts a partner-first exchange from a topic in one press while the composer remains usable', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const value = snapshot('a', 41)
  value.topicChoices = [{ id: 'food', glyph: '☕', target: 'Ordering food', romanized: null, translation: 'Ordering food' }]
  await act(async () => watches[0].resolve(value))
  expect((await draftField())).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: /Ordering food/ }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(commands()[0].action).toEqual({ kind: 'startConversation', conversationId: 'a', expectedRevision: workspace.revision, configuration: { difficulty: 'beginner', varietyId: '', direction: { topic: { kind: 'builtin', id: 'food' }, timeReference: 'any', usePersonaDetails: true } }, message: null, input: null })
  expect(screen.queryByText('¿Qué quieres beber?')).toBeNull()
  const response = snapshot('a', 42)
  response.messages = [{ ...exchangeSnapshot().messages[1], text: '¿Qué quieres beber?' }]
  await act(async () => watches[1].resolve(response))
  expect(within(document.querySelector('.msg.bot') as HTMLElement).getByText('¿Qué quieres beber?')).toBeVisible()
  expect(document.querySelector('.msg.me')).toBeNull()
})

it('shows a failed partner start without blocking the composer', async () => {
  submit = async () => { throw { code: 'conflict', message: 'This conversation already started.' } }
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot('a', 41)))
  fireEvent.click(screen.getByRole('button', { name: / starts$/ }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(commands()[0].action).toEqual({ kind: 'startConversation', conversationId: 'a', expectedRevision: workspace.revision, configuration: { difficulty: 'beginner', varietyId: '', direction: { topic: null, timeReference: 'any', usePersonaDetails: true } }, message: null, input: null })
  expect(await screen.findByRole('alert')).toHaveTextContent('This conversation already started.')
  expect((await draftField())).toBeEnabled()
})

it('persists Show answer through the real handler and renders only the returned native correction', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const value = exchangeSnapshot()
  value.messages[0].feedback = { issues: [], corrections: [], notes: [], meaningRecovered: 'full', items: [{ construct: 'past', quote: 'fue', outcome: 'partial', rationale: 'Past reference' }], candidatesSent: 18, itemsReturned: 1 }
  value.messages[0].coachDecision = { exposedMove: null, shown: { construct: 'past', quote: 'fue', move: 'hint', text: 'Which form goes with yo?' }, retryInvited: true, alsoNoticed: [], keptGoing: false }
  await act(async () => watches[0].resolve(value))
  fireEvent.click(screen.getByRole('group', { name: 'Your message' }))
  expect(commands()).toHaveLength(0)
  fireEvent.click(screen.getByRole('button', { name: 'View coaching help' }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(commands()[0].action).toEqual({ kind: 'coachControl', turnId: 'a-turn', control: 'open_card' })
  expect(screen.queryByText('Which form goes with yo?')).toBeNull()
  const exposed = structuredClone(value)
  exposed.revision++
  exposed.contentRevision = String(exposed.revision)
  exposed.messages[0].coachDecision!.exposedMove = 'hint'
  await act(async () => watches[1].resolve(exposed))
  expect(within(screen.getByRole('region', { name: 'Conversation coaching' })).getByText('Which form goes with yo?')).toBeVisible()
  expect(screen.queryByText('Yo fui ayer.')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Show answer' }))
  await waitFor(() => expect(commands()).toHaveLength(2))
  expect(commands()[1].action).toEqual({ kind: 'coachControl', turnId: 'a-turn', control: 'show_answer' })
  expect(screen.queryByText('Yo fui ayer.')).toBeNull()
  const next = structuredClone(exposed)
  next.revision++
  next.contentRevision = String(next.revision)
  next.messages[0].coachDecision!.exposedMove = 'explicit'
  next.messages[0].coachDecision!.shown = { ...value.messages[0].coachDecision!.shown!, move: 'explicit', text: 'Yo fui ayer.' }
  await act(async () => watches[2].resolve(next))
  expect(within(screen.getByRole('region', { name: 'Conversation coaching' })).getByText('Yo fui ayer.')).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Show answer' })).toBeNull()
})

it('opens empty starter content at the top but scrolls real messages to the bottom', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const stream = document.querySelector<HTMLDivElement>('.stream')!
  Object.defineProperty(stream, 'scrollHeight', { value: 1500, configurable: true })
  stream.scrollTop = 300
  await act(async () => watches[0].resolve(snapshot()))
  expect(stream.scrollTop).toBe(0)
  await act(async () => watches[1].resolve(exchangeSnapshot()))
  expect(stream.scrollTop).toBe(1500)
})

it('hides starters after accepting an opening and surfaces failure without a learner bubble', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const value = snapshot('a', 41)
  value.opening = { kind: 'partner' }
  value.turns = [{ id: 'opening', replacesTurnId: null, replacedBy: null, route: 'hosted', state: 'pending', paused: false, hold: null,   }]
  await act(async () => watches[0].resolve(value))
  expect(screen.queryByRole('button', { name: / starts$/ })).toBeNull()
  expect(screen.getByLabelText('Conversation opening')).toHaveTextContent('Replying')
  const paused = structuredClone(value)
  paused.revision++
  paused.contentRevision = String(paused.revision)
  paused.connection.paused = true
  await act(async () => watches[1].resolve(paused))
  expect(screen.getByLabelText('Conversation opening')).toHaveTextContent('Conversation opening is paused.')
  expect(screen.getByLabelText('Conversation opening')).not.toHaveTextContent('Replying')
  const resumed = structuredClone(paused)
  resumed.revision++
  resumed.contentRevision = String(resumed.revision)
  resumed.connection.paused = false
  await act(async () => watches[2].resolve(resumed))
  expect(screen.getByLabelText('Conversation opening')).toHaveTextContent('Replying')
  const failed = structuredClone(resumed)
  failed.revision++
  failed.contentRevision = String(failed.revision)
  failed.turns[0].state = 'failed'
  failed.turns[0].channel = 'persona_opening'
  failed.turns[0].hold = { code: 'credential', message: 'Opening provider refused access.', refusal: null }
  await act(async () => watches[3].resolve(failed))
  expect(screen.getByRole('alert')).toHaveTextContent('Opening provider refused access.')
  expect(screen.getByRole('button', { name: 'Open AI activity' })).toBeEnabled()
  expect(document.querySelector('.msg.me')).toBeNull()
})

it('keeps the committed contact selection after failed navigation and follows another opened contact', async () => {
  workspace.contacts = ['a', 'b', 'c'].map(id => ({ id: `contact-${id}`, learnerId: 'learner', personaId: `persona-${id}`, archived: false, revision: 1, createdAt: 'today' }))
  workspace.personas = ['a', 'b', 'c'].map(id => ({ id: `persona-${id}`, languageId: 'spanish', revision: 1, details: { name: id.toUpperCase(), vibe: [] } } as unknown as Snapshot['personas'][number]))
  workspace.conversations = ['a', 'b', 'c'].map((id, index) => ({ ...directory().conversations[0], id, contactId: `contact-${id}`, lastUsed: 3 - index }))
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot()))
  const partner = (name: string) => {
    if (!screen.queryByRole('menu', { name: 'Partners' })) fireEvent.click(document.querySelector('.persona-picker-toggle')!)
    return screen.getByRole('menuitemradio', { name: new RegExp(`^${name}`) })
  }
  expect(partner('A')).toHaveAttribute('aria-checked', 'true')
  submit = async () => { throw new Error('Navigation refused') }
  fireEvent.click(partner('B'))
  await screen.findByText('Navigation refused')
  expect(partner('A')).toHaveAttribute('aria-checked', 'true')
  expect(partner('B')).toHaveAttribute('aria-checked', 'false')
  submit = async command => ({ actionId: command.actionId, entityId: 'accepted', revision: 11 })
  fireEvent.click(partner('B'))
  await waitFor(() => expect(watches.some(watch => watch.conversationId === 'b')).toBe(true))
  await act(async () => watches.find(watch => watch.conversationId === 'b')!.resolve(snapshot('b')))
  expect(partner('B')).toHaveAttribute('aria-checked', 'true')
  fireEvent.click(partner('C'))
  await waitFor(() => expect(watches.some(watch => watch.conversationId === 'c')).toBe(true))
  await act(async () => watches.find(watch => watch.conversationId === 'c')!.resolve(snapshot('c')))
  expect(partner('C')).toHaveAttribute('aria-checked', 'true')
  fireEvent.click(partner('B'))
  await waitFor(() => expect(commands().filter(command => command.action.kind === 'openConversation' && command.action.conversationId === 'b')).toHaveLength(3))
})

it('offers explicit read recovery in the chat without issuing inference', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].reject(new Error('Native read failed')))
  expect(screen.getByText(/Native read failed/)).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Retry reading conversation' }))
  await waitFor(() => expect(watches).toHaveLength(2))
  await act(async () => watches[1].resolve(exchangeSnapshot()))
  expect(screen.queryByText(/Native read failed/)).toBeNull()
  expect(commands()).toEqual([])
})

it('suggests the authored greeting in a new conversation, set apart as target-language text', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  await act(async () => watches[0].resolve(snapshot('a', 41)))
  const greeting = await screen.findByText('hola')
  expect(greeting.closest('.target-word')).not.toBeNull()
  expect(greeting.closest('.voice-prompt')).toHaveTextContent('Say hola to start')
  // Once there is a conversation, the prompt is the plain one again.
  await act(async () => watches[1].resolve(exchangeSnapshot()))
  expect(screen.queryByText('Say hola to start')).toBeNull()
})

it('captures tense and difficulty with the real first learner message', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const value = snapshot('a', 41)
  value.topicChoices = [{ id: 'food', glyph: '☕', target: 'Ordering food', romanized: null, translation: 'Ordering food' }]
  await act(async () => watches[0].resolve(value))
  // The start card's options go with whichever way the conversation starts.
  fireEvent.click(screen.getByRole('button', { name: /^Options/ }))
  fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Time frame' })).getByRole('radio', { name: 'Past events' }))
  fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Difficulty' })).getByRole('radio', { name: 'Absolute zero' }))
  expect(commands()).toHaveLength(0)
  fireEvent.change((await draftField()), { target: { value: 'Comí arroz.' } })
  fireEvent.click(screen.getByRole('button', { name: 'Send' }))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(commands()[0].action).toMatchObject({ kind: 'startConversation', conversationId: 'a', message: 'Comí arroz.', input: { scaffold: false }, configuration: { difficulty: 'absolute_zero', direction: { topic: null, timeReference: 'past' } } })
})

it('revises directly when only private coach turns follow the source', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const value = exchangeSnapshot(true)
  value.revisionSuffixCounts = value.revisionSuffixCounts.map(scope => ({...scope, exchangeCount:0, coachTurnCount:2}))
  await act(async () => watches[0].resolve(value))
  openMenus()
  fireEvent.click(screen.getByRole('button', {name:'Edit message'}))
  fireEvent.click(screen.getByRole('button', {name:'Send'}))
  await waitFor(() => expect(commands()).toHaveLength(1))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(commands()[0].action.kind).toBe('reviseTurn')
})

it('routes bubble selection to its message side and keeps it while a newer exchange arrives', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const initial = exchangeSnapshot()
  await act(async () => watches[0].resolve(initial))
  const learner = within(document.querySelector('.stream') as HTMLElement).getByRole('group', { name: 'Your message' })
  const partner = within(document.querySelector('.stream') as HTMLElement).getByRole('group', { name: 'Partner replied' })
  fireEvent.click(learner)
  expect(learner).toHaveAttribute('aria-current', 'true')
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.querySelector('.break')).toHaveTextContent('Yo fue ayer')
  fireEvent.click(partner)
  expect(partner).toHaveAttribute('aria-current', 'true')
  expect(learner).not.toHaveAttribute('aria-current')
  expect(document.querySelector('.break')).toHaveTextContent('¿Adónde fuiste?')
  expect(document.querySelector('.break')).not.toHaveTextContent('Yo fue ayer')
  const panel = document.querySelector('.break') as HTMLElement
  expect(within(panel).getByRole('button', { name: 'Explain grammar' })).toBeVisible()
  expect(within(panel).getByRole('button', { name: 'Suggest a reply' })).toBeVisible()
  expect(within(panel).queryByRole('button', { name: 'Help with this reply' })).toBeNull()
  expect(panel.querySelector('.msg.bot')!.compareDocumentPosition(panel.querySelector('.reply-help')!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  await waitFor(() => expect(watches).toHaveLength(2))
  await act(async () => watches[1].resolve({ ...initial, revision: 32, contentRevision: '32', messages: [...initial.messages,
    { ...initial.messages[0], id: 'next-user', turnId: 'next', sequence: 3, text: 'Another message' },
    { ...initial.messages[1], id: 'next-reply', turnId: 'next', sequence: 4, text: 'Another reply' },
  ] }))
  expect(partner).toHaveAttribute('aria-current', 'true')
  expect(document.querySelector('.break')).toHaveTextContent('¿Adónde fuiste?')
  expect(document.querySelector('.break')).not.toHaveTextContent('Another reply')
  expect(within(panel).getByRole('button', { name: 'Suggest a reply' })).toBeVisible()
})

it('automatically selects the latest learner message, then its reply, without requesting analysis', async () => {
  render(page())
  await waitFor(() => expect(watches).toHaveLength(1))
  const initial = snapshot('a', 30, 'Yo fue ayer')
  await act(async () => watches[0].resolve(initial))
  expect(within(document.querySelector('.stream') as HTMLElement).getByRole('group', { name: 'Your message' })).toHaveAttribute('aria-current', 'true')
  await act(async () => watches[1].resolve(exchangeSnapshot()))
  expect(within(document.querySelector('.stream') as HTMLElement).getByRole('group', { name: 'Your message' })).not.toHaveAttribute('aria-current')
  expect(within(document.querySelector('.stream') as HTMLElement).getByRole('group', { name: 'Partner replied' })).toHaveAttribute('aria-current', 'true')
  expect(commands()).toHaveLength(0)
  const next = exchangeSnapshot()
  await act(async () => watches[2].resolve({ ...next, revision: 32, contentRevision: '32', messages: [...next.messages,
    { ...next.messages[0], id: 'next-user', turnId: 'next', sequence: 3, text: 'Another message' },
  ] }))
  expect(within(document.querySelector('.stream') as HTMLElement).getAllByRole('group', { name: 'Your message' })[1]).toHaveAttribute('aria-current', 'true')
  expect(within(document.querySelector('.stream') as HTMLElement).getByRole('group', { name: 'Partner replied' })).not.toHaveAttribute('aria-current')
  expect(commands()).toHaveLength(0)
})


it('keeps coach controls equal to chat and opens analysis and editing from the coach copy', async () => {
  const read = vi.fn().mockResolvedValue({ gloss: null, translation: null, audioBase64: null, audioAlignment: null, receipt: null,
    explanations: { cards: [{ quote: '¿Adónde fuiste?', title: 'Past-tense question', body: 'Fuiste asks where the listener went.', example: '¿Adónde fuiste ayer?', contrast: '' }] } })
  render(<ReadingHelp services={{ read, speak: vi.fn(), activity: vi.fn() }} languages={[]}>{page()}</ReadingHelp>)
  await waitFor(() => expect(watches).toHaveLength(1))
  const initial = exchangeSnapshot()
  // Old turn help can contain a successful empty result. It is not text analysis.
  initial.messages[1].replyExplanations = { cards: [] }
  initial.messages[1].explanationsState = 'succeeded'
  await act(async () => watches[0].resolve(initial))
  const stream = document.querySelector('.stream') as HTMLElement
  const panel = document.querySelector('.break') as HTMLElement
  const controls = (root: Element) => within(root as HTMLElement).getAllByRole('button').map(button => ({
    label: button.getAttribute('aria-label') ?? button.textContent,
    disabled: (button as HTMLButtonElement).disabled,
  }))
  expect(controls(panel.querySelector('.msg.bot')!)).toEqual(controls(stream.querySelector('.msg.bot')!))
  fireEvent.click(within(panel).getByRole('button', { name: 'Analysis' }))
  const analysis = screen.getByRole('dialog', { name: 'Message analysis' })
  await waitFor(() => expect(analysis).toHaveTextContent('Fuiste asks where the listener went.'))
  expect(read).toHaveBeenCalledExactlyOnceWith({ language: 'spanish', variety: '', explanation: 'english', explanationVariety: 'english-united-states', conversationId: 'a', text: '¿Adónde fuiste?', aid: 'explanations' }, expect.any(AbortSignal))
  expect(analysis).not.toHaveTextContent('Yo fue ayer')
  expect(analysis).not.toHaveTextContent('Nothing to flag')
  expect(within(analysis).queryByText('Skills')).toBeNull()
  expect(commands()).toHaveLength(0)
  fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Close Message analysis' }))
  fireEvent.click(within(stream).getByRole('button', { name: 'Analysis' }))
  await waitFor(() => expect(read).toHaveBeenCalledTimes(2))
  expect(screen.getByRole('dialog', { name: 'Message analysis' })).not.toHaveTextContent('Yo fue ayer')
  fireEvent.click(screen.getByRole('button', { name: 'Close Message analysis' }))
  fireEvent.click(within(stream).getByRole('group', { name: 'Your message' }))
  expect(controls(panel.querySelector('.msg.me')!)).toEqual(controls(stream.querySelector('.msg.me')!))
  fireEvent.click(within(panel).getByRole('button', { name: 'Coach' }))
  const feedback = screen.getByRole('dialog', { name: 'Feedback on your message' })
  expect(feedback).toBeVisible()
  expect(feedback).toHaveTextContent('Yo fue ayer')
  expect(read).toHaveBeenCalledTimes(2)
  fireEvent.click(within(feedback).getByRole('button', { name: 'Edit and resend message' }))
  expect(await draftField()).toHaveValue('Yo fue ayer')
})
