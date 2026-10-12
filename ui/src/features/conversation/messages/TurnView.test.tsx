// @vitest-environment jsdom
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { TurnView as SharedTurnView, type TurnViewProps } from './TurnView'

import { ReadingPreferencesContext } from '../../../components/reading/ReadingPreferences'
import type { TranscriptionInspectionResult } from '../../../generated/contracts'
const requestHelp = vi.hoisted(() => vi.fn().mockResolvedValue(undefined))
vi.mock('../../../platform/ipc/message-help', () => ({ requestMessageHelp: requestHelp }))

/** Opens every message's ⋯ menu, where Words, Analysis and Pronunciation live. */
const openMenus = () => screen.queryAllByRole('button', { name: 'More actions' }).forEach(button => { if (button.getAttribute('aria-expanded') !== 'true') fireEvent.click(button) })


/// The reading preferences the conversation page provides through context.
interface Reading { autoTranslate: boolean; alwaysRomanize: boolean; alwaysPronunciation: boolean; showRomanization: boolean }
type Props = TurnViewProps & Reading

function TurnView({ autoTranslate, alwaysRomanize, alwaysPronunciation, showRomanization, ...input }: Props) {
  return <ReadingPreferencesContext value={{autoTranslate, alwaysRomanize, alwaysPronunciation, supportsRomanization:showRomanization}}><SharedTurnView {...input} /></ReadingPreferencesContext>
}

beforeEach(() => {
  requestHelp.mockClear()
  HTMLDialogElement.prototype.showModal = function () { this.open = true }
  HTMLDialogElement.prototype.close = function () { this.open = false }
})

function props(): Props {
  const token = { text: 'Hola', gloss: 'Hello', pos: null, notable: false, romanization: null, pronunciation: null }
  return { turn: { id: 1, user: 'Hola', pendingText: '', assistant: { reply: 'Hola', tokens: [token], user_tokens: [token], translation: 'Persona translation', user_translation: 'Learner translation', mechanics: [], scaffolds: { replies: [], frames: [], starters: [] }, errors: [] } }, reviewing: false, focused: false, ttsReady: true, speaking: false, showRomanization: false, alwaysRomanize: false, alwaysPronunciation: false, autoTranslate: false, rtl: false, onBubbleTap: vi.fn(), onSpeak: vi.fn(), onAskCoach: vi.fn(), editing: false }
}
it('offers phrase starts on both durable speakers without starting on render', () => {
  const input = props()
  input.turn.userMessageId = 'user-source'
  input.turn.assistant!.messageId = 'partner-source'
  input.turn.assistant!.reply = 'Buenas tardes.'
  input.turn.assistant!.tokens = []
  const start = vi.fn().mockResolvedValue(undefined)
  render(<TurnView {...input} onStartPhrase={start} />)
  expect(start).not.toHaveBeenCalled()
  const buttons = screen.getAllByRole('button', { name: 'Start conversation from this message' })
  fireEvent.click(buttons[0]); fireEvent.click(buttons[1])
  expect(start).toHaveBeenNthCalledWith(1, 'user-source', 'Hola')
  expect(start).toHaveBeenNthCalledWith(2, 'partner-source', 'Buenas tardes.')
})

it('requests translations for the exact speaker source only after a click', async () => {
  const input = props()
  input.autoTranslate = true
  input.turn.userMessageId = 'learner-source'
  input.turn.assistant!.messageId = 'partner-source'
  input.turn.assistant!.translation = null
  input.turn.assistant!.user_translation = null
  render(<TurnView {...input} />)
  expect(requestHelp).not.toHaveBeenCalled()
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Translate your message' })) })
  expect(requestHelp).toHaveBeenCalledExactlyOnceWith('learner-source', 'translation', false)
  await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' })) })
  expect(requestHelp).toHaveBeenLastCalledWith('partner-source', 'translation', false)
})
it('uses matching reading tools and independently toggles pronunciation on either speaker', () => {
  const input = props()
  input.turn.assistant!.tokens[0].pronunciation = 'OH-lah'
  input.turn.assistant!.user_tokens[0] = { ...input.turn.assistant!.tokens[0] }
  const view = render(<TurnView {...input} />)
  const learner = view.container.querySelector('.msg.me') as HTMLElement
  const partner = view.container.querySelector('.msg.bot') as HTMLElement
  for (const bubble of [learner, partner]) {
    const toolbar = bubble.querySelector('.message-tools') as HTMLElement
    expect(within(toolbar).getAllByRole('button').filter(button => ['Translate', 'Words', 'Pronunciation', 'Analysis', 'Coach'].includes(button.title)).map(button => button.title))
      .toEqual(['Translate', 'Words', 'Pronunciation', bubble === learner ? 'Coach' : 'Analysis'])
    const pronunciation = within(toolbar).getByRole('button', { name: 'Pronunciation' })
    expect(pronunciation).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(pronunciation)
    expect(pronunciation).toHaveAttribute('aria-pressed', 'true')
    expect(bubble.querySelector('.wpronunciation')).toHaveTextContent('OH-lah')
    expect(bubble.querySelector('.wg')).toBeNull()
    expect((bubble === learner ? partner : learner).querySelector('.wpronunciation')).toBeNull()
    fireEvent.click(pronunciation)
    expect(bubble.querySelector('.wpronunciation')).toBeNull()
  }
})
it('places the persona reaction on the reply', () => {
  const input = props()
  input.turn.reaction = { kind: 'confused', answer: {choice:'confused',probabilities:{confused:1},confidence:1} }
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.partner-turn > .partner-reaction-slot > .persona-reaction')).not.toBeNull()
  expect(view.container.querySelector('.msg .persona-reaction')).toBeNull()
})
it('keeps an explicit message translation override when the default changes', () => {
  const input = props()
  const view = render(<TurnView {...input} />)
  expect(screen.queryByText('Learner translation')).toBeNull()
  expect(screen.queryByText('Persona translation')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  expect(screen.getByText('Persona translation')).toBeVisible()
  view.rerender(<TurnView {...input} autoTranslate={true} />)
  expect(screen.getByText('Persona translation')).toBeVisible()
  expect(screen.getByText('Learner translation')).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  expect(screen.queryByText('Persona translation')).toBeNull()
})

it('honors pronunciation when word meanings are shown', () => {
  const input = props()
  input.autoTranslate = true
  input.turn.assistant!.reply = 'Soy Elia.'
  input.turn.assistant!.tokens = [
    { text: 'Soy', gloss: 'I am', pronunciation: 'soy', romanization: null, pos: null, notable: false },
    { text: 'Elia.', gloss: 'Elia', pronunciation: 'Eh-lee-ah', romanization: null, pos: null, notable: false },
  ]
  const view = render(<TurnView {...input} />)
  const soy = within(view.container.querySelector('.msg.bot') as HTMLElement).getByRole('button', { name: 'Soy' })
  expect(soy.closest('.wu')).toHaveTextContent('I am')
  expect(view.container.querySelector('.msg.bot .wpronunciation')).toBeNull()
  expect(screen.queryByText('Eh-lee-ah')).toBeNull()
  expect(view.container.querySelector('.phrase-pronunciation')).toBeNull()
  view.rerender(<TurnView {...input} alwaysPronunciation={true} />)
  expect(screen.getByText('Eh-lee-ah').closest('.wu')).toHaveTextContent('Elia.')
  expect(screen.getByText('Eh-lee-ah').closest('.wu')).not.toHaveTextContent('I am')
})

it('renders source punctuation once and anchors feedback below the learner bubble', () => {
  const input = props()
  input.turn.assistant!.reply = '¡Hola! ¿Te gusta el sol?'
  input.turn.assistant!.tokens = ['¡', '¡Hola!', '!', '¿', '¿Te', 'gusta', 'el', 'sol?', '?'].map(text => ({text,gloss:null,pronunciation:null,romanization:null,pos:null,notable:false}))
  const view = render(<TurnView {...input} autoTranslate={false} />)
  expect(view.container.querySelector('.msg.bot .w')!.textContent).toBe('¡Hola! ¿Te gusta el sol?')
  const feedback = screen.getByRole('button', { name: 'Coach feedback for message 1' })
  expect(feedback.closest('.message-feedback-line')).not.toBeNull()
  expect(feedback.closest('.msg')).toBeNull()
})

it('does not expose playback without a connected action', () => {
  const input = props()
  const view = render(<TurnView {...input} onSpeak={undefined} />)
  expect(screen.queryByRole('button', { name: 'Play' })).not.toBeInTheDocument()
  view.rerender(<TurnView {...input} />)
  fireEvent.click(screen.getByRole('button', { name: 'Play' }))
  expect(input.onSpeak).toHaveBeenCalledWith('Hola', 1)
})

it('retries partial saved gloss only on explicit request and keeps reading during retry', async () => {
  const input = props()
  input.autoTranslate = true
  input.turn.assistant!.tokens = []
  input.turn.assistant!.savedGloss = { sourceMessageId: 'message', targetLanguageId: 'spanish', explanationLanguageId: 'english', formatVersion: 'format', templateVersion: 'template', boundaryPolicy: 'policy', operationId: 'operation', attemptId: 'attempt', coverage: 'partial', segments: [{start:0,end:4,kind:'gloss',gloss:'Hello'}] }
  input.turn.assistant!.glossState = 'succeeded'
  input.turn.assistant!.glossOperationId = 'operation'
  let finish!: () => void
  input.onRetryGloss = vi.fn(() => new Promise<void>(resolve => { finish = resolve }))
  const view = render(<TurnView {...input} />)
  expect(input.onRetryGloss).not.toHaveBeenCalled()
  fireEvent.click(view.container.querySelector('.msg.bot [data-source-start] [role="button"]')!)
  expect(view.container.querySelector('.msg.bot .wg')).toHaveTextContent('Hello')
  expect(input.onRetryGloss).not.toHaveBeenCalled()
  expect(screen.getByRole('button', {name:'Retry word meanings'}).closest('.message-actions')).toBeNull()
  expect(screen.getByRole('button', {name:'Retry word meanings'}).closest('.trans')).not.toBeNull()
  fireEvent.click(screen.getByRole('button', {name:'Retry word meanings'}))
  fireEvent.click(screen.getByRole('button', {name:'Retry word meanings'}))
  expect(input.onRetryGloss).toHaveBeenCalledExactlyOnceWith('operation')
  expect(screen.getByRole('button', {name:'Retry word meanings'})).toBeDisabled()
  expect(view.container.querySelector('.msg.bot .wg')).toHaveTextContent('Hello')
  await act(async () => finish())
})

it('keeps accepted meanings visible while background repair is pending', () => {
  const input = props()
  input.autoTranslate = true
  input.onRetryGloss = vi.fn()
  input.turn.assistant!.tokens = []
  input.turn.assistant!.savedGloss = { sourceMessageId: 'message', targetLanguageId: 'spanish', explanationLanguageId: 'english', formatVersion: 'format', templateVersion: 'policy', boundaryPolicy: 'policy', operationId: 'operation', attemptId: 'attempt', coverage: 'partial', segments: [{start:0,end:4,kind:'gloss',gloss:'Hello'}] }
  input.turn.assistant!.glossState = 'running'
  input.turn.assistant!.glossOperationId = 'operation'
  const view = render(<TurnView {...input} />)
  expect(screen.getByText('Finishing word meanings…')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Retry word meanings' })).toBeNull()
  fireEvent.click(view.container.querySelector('.msg.bot [data-source-start] [role="button"]')!)
  expect(view.container.querySelector('.msg.bot .wg')).toHaveTextContent('Hello')
  expect(input.onRetryGloss).not.toHaveBeenCalled()
})

it.each([null, 'running', 'failed', 'unknown'])('keeps reply words passive without saved gloss (%s)', state => {
  const input = props()
  input.turn.assistant!.tokens = []
  input.turn.assistant!.glossState = state
  const view = render(<TurnView {...input} />)
  const text = view.container.querySelector('.msg.bot .target-text')!
  expect(text).toHaveTextContent('Hola')
  expect(text.querySelector('[role="button"]')).toBeNull()
  fireEvent.click(text)
  fireEvent.contextMenu(text)
  fireEvent.keyDown(text, {key:'Enter'})
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(input.onBubbleTap).not.toHaveBeenCalled()
})

it.each([
  ['ready', 'Translating…'], ['running', 'Translating…'], ['waiting_dependencies', 'Translating…'],
  ['failed', 'Translation failed'], ['unknown', 'Translation outcome unknown'],
  ['cancelled', 'Translation cancelled'], ['invalidated', 'Translation unavailable'],
])('shows authoritative translation state %s without hiding saved text or invoking work', (state, label) => {
  const input = props()
  input.turn.assistant!.translationState = state
  input.onRetryGloss = vi.fn()
  const view = render(<TurnView {...input} />)
  expect(screen.getByRole('status')).toHaveTextContent(label)
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  expect(screen.getByText('Persona translation')).toBeVisible()
  expect(screen.getByRole('button', { name: 'Play' })).toBeEnabled()
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  expect(screen.queryByText('Persona translation')).toBeNull()
  expect(screen.getByRole('status')).toHaveTextContent(label)
  view.rerender(<TurnView {...input} autoTranslate={false} />)
  expect(input.onRetryGloss).not.toHaveBeenCalled()
  expect(input.onSpeak).not.toHaveBeenCalled()
})
it('distinguishes no requested translation from failure and clears progress when translation arrives', () => {
  const input = props()
  input.turn.assistant!.translation = null
  const view = render(<TurnView {...input} />)
  expect(screen.queryByRole('status')).toBeNull()
  const update = (translationState: string, translation: string | null = null) => view.rerender(<TurnView {...input} turn={{ ...input.turn, assistant: { ...input.turn.assistant!, translationState, translation } }} />)
  update('running')
  expect(screen.getByRole('status')).toHaveTextContent('Translating…')
  update('failed')
  expect(screen.getByRole('status')).toHaveTextContent('Translation failed')
  expect(screen.queryByText('Translating…')).toBeNull()
  update('succeeded', 'Saved translation')
  expect(screen.queryByRole('status')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  expect(screen.getByText('Saved translation')).toBeVisible()
  expect(screen.queryByRole('button', { name: /Retry translation/ })).toBeNull()
})

it('uses saved human glosses before a reply and separates scores from bottom actions', () => {
  const input = props()
  input.autoTranslate = true
  input.alwaysPronunciation = true
  input.turn.assistant = null
  input.turn.userTranslation = 'Hello there'
  input.turn.userSavedGloss = {
    sourceMessageId: 'human', targetLanguageId: 'spanish', explanationLanguageId: 'english',
    formatVersion: 'v1', templateVersion: 'v1', boundaryPolicy: 'v1',
    operationId: 'human-gloss', attemptId: 'attempt', coverage: 'complete',
    segments: [{ start: 0, end: 4, kind: 'gloss', gloss: 'Hello', pronunciation: 'OH-lah' }],
  }
  const view = render(<TurnView {...input} />)
  const word = screen.getByRole('button', { name: 'Hola', expanded: false })
  fireEvent.click(word)
  expect(word).toHaveAttribute('aria-expanded', 'true')
  expect(view.container.querySelector('.msg.me .wg')).toHaveTextContent('Hello')
  expect(view.container.querySelector('.msg.me .wpronunciation')).toHaveTextContent('OH-lah')
  expect(view.container.querySelector('.saved-word-help')).toHaveAttribute('popover', 'manual')
  const grade = screen.getByRole('button', { name: 'Coach feedback for message 1' })
  expect(grade.parentElement).toHaveClass('message-feedback-line')
  expect(grade).toHaveTextContent('Feedback')
  expect(grade.closest('.msg.me')).toBeNull()
  openMenus()
  expect(screen.getByRole('button', { name: 'Coach' }).closest('.message-actions')).not.toBeNull()
  expect(view.container.querySelector('.msg.me .trans')).toHaveTextContent('Hello there')
  fireEvent.click(word)
  expect(word).toHaveAttribute('aria-expanded', 'false')
})

it('lets a message hide translation while the conversation default stays on', () => {
  const input = props()
  const view = render(<TurnView {...input} autoTranslate />)
  expect(screen.getByText('Persona translation')).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  expect(screen.queryByText('Persona translation')).toBeNull()
  expect(screen.getByText('Learner translation')).toBeVisible()
  expect(screen.getByRole('button', { name: 'Translate partner message' })).toHaveAttribute('aria-pressed', 'false')
  view.rerender(<TurnView {...input} autoTranslate={false} />)
  view.rerender(<TurnView {...input} autoTranslate />)
  expect(screen.queryByText('Persona translation')).toBeNull()
})

it('reveals saved word meanings without starting inference or changing whole-message translation', () => {
  const input = props()
  input.autoTranslate = true
  input.turn.assistant!.tokens = []
  input.turn.assistant!.savedGloss = { sourceMessageId: 'message', targetLanguageId: 'spanish', explanationLanguageId: 'english', formatVersion: 'format', templateVersion: 'template', boundaryPolicy: 'policy', operationId: 'operation', attemptId: 'attempt', coverage: 'complete', segments: [{start:0,end:4,kind:'gloss',gloss:'Hello'}] }
  const view = render(<TurnView {...input} />)
  fireEvent.click(screen.getByRole('button', { name: 'Translate partner message' }))
  openMenus()
  const words = within(view.container.querySelector('.msg.bot') as HTMLElement).getByRole('button', {name:'Words'})
  expect(words).toBeEnabled()
  fireEvent.click(words)
  expect(view.container.querySelector('.msg.bot .wg')).toBeNull()
  fireEvent.click(words)
  expect(view.container.querySelector('.msg.bot .wg')).toHaveTextContent('Hello')
  expect(screen.queryByText('Persona translation')).toBeNull()
  fireEvent.click(words)
  expect(view.container.querySelector('.msg.bot .wg')).toBeNull()
})

it('shows a failed learner translation on the learner message without invented text', () => {
  const input = props()
  input.turn.userTranslationState = 'failed'
  input.turn.userTranslation = null
  input.turn.assistant!.user_translation = null
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.msg.me [role="status"]')?.textContent).toBe('Translation failed')
  expect(view.container.querySelector('.msg.bot [role="status"]')).toBeNull()
})


it('keeps message edit and playback actions isolated from token reveal and honors their availability', () => {
  const input = props()
  const edit = vi.fn()
  const view = render(<TurnView {...input} onEditUser={edit} rtl />)
  const pencil = screen.getByRole('button', { name: 'Edit message' })
  const speaker = screen.getByRole('button', { name: 'Play' })
  expect(pencil.querySelector('svg')).not.toBeNull()
  expect(pencil.closest('.msg.me')).toHaveClass('rtl')
  expect(pencil.closest('.message-tools-actions')).not.toBeNull()
  expect(speaker.closest('.msg.bot')).toHaveClass('rtl')
  expect(speaker.closest('.message-tools')).not.toBeNull()
  fireEvent.click(pencil)
  fireEvent.doubleClick(pencil)
  fireEvent.click(speaker)
  fireEvent.doubleClick(speaker)
  expect(edit).toHaveBeenCalledExactlyOnceWith(input.turn)
  expect(input.onSpeak).toHaveBeenCalledExactlyOnceWith('Hola', 1)
  view.rerender(<TurnView {...input} onEditUser={edit} editDisabled speaking />)
  expect(screen.getByRole('button', { name: 'Edit message' })).toBeDisabled()
  fireEvent.click(screen.getByRole('button', { name: 'Stop playback' }))
  expect(input.onSpeak).toHaveBeenCalledTimes(2)
  view.rerender(<TurnView {...input} onEditUser={undefined} ttsReady={false} />)
  expect(screen.queryByRole('button', { name: 'Edit message' })).toBeNull()
  expect(screen.queryByRole('button', { name: 'Play' })).toBeNull()
})


it('attaches both assistance rows to their source bubble and toggles only the learner bubble', () => {
  const input = props()
  input.autoTranslate = true
  const view = render(<TurnView {...input} />)
  const learner = view.container.querySelector('.msg.me') as HTMLElement
  const partner = view.container.querySelector('.msg.bot') as HTMLElement
  openMenus()
  for (const bubble of [learner, partner]) {
    expect(within(bubble).getByRole('button', { name: /Translate/ }).closest('.message-actions')).not.toBeNull()
    const label = bubble === learner ? 'Coach' : 'Analysis'
    const detail = within(bubble).getByRole('button', { name: label })
    expect(detail.closest('.message-actions')).not.toBeNull()
    // Row tools are icons: the name is the tooltip, not visible text.
    expect(detail).toHaveAttribute('title', label)
  }
  expect(within(learner).queryByRole('button', { name: 'Analysis' })).toBeNull()
  expect(within(partner).queryByRole('button', { name: 'Coach' })).toBeNull()
  fireEvent.click(within(partner).getByRole('button', { name: 'Analysis' }))
  expect(input.onBubbleTap).toHaveBeenCalledExactlyOnceWith(input.turn.id)
  expect(screen.queryByRole('dialog', { name: 'Feedback on your message' })).toBeNull()
  openMenus()
  const words = within(learner).getByRole('button', { name: 'Words' })
  expect(words).toHaveAttribute('aria-pressed', 'true')
  fireEvent.click(words)
  openMenus()
  expect(within(partner).getByRole('button', { name: 'Words' })).toHaveAttribute('aria-pressed', 'true')
  view.rerender(<TurnView {...input} />)
  expect(words).toHaveAttribute('aria-pressed', 'false')
  for (const action of learner.querySelectorAll('.message-feedback button')) fireEvent.doubleClick(action)
  for (const action of partner.querySelectorAll('.message-actions button')) fireEvent.doubleClick(action)
  openMenus()
  fireEvent.click(within(learner).getByRole('button', { name: 'Coach' }))
  expect(input.onBubbleTap).toHaveBeenCalledTimes(1)
  const dialog = screen.getByRole('dialog', { name: 'Feedback on your message' })
  expect(dialog.parentElement).toBe(document.body)
})

it('toggles saved learner meanings independently before a reply without requesting work', () => {
  const input = props()
  input.autoTranslate = true
  input.turn.assistant = null
  input.turn.userTranslation = 'Hello there'
  input.turn.userGlossState = 'running'
  input.turn.userSavedGloss = {
    sourceMessageId: 'human', targetLanguageId: 'spanish', explanationLanguageId: 'english',
    formatVersion: 'v1', templateVersion: 'v1', boundaryPolicy: 'v1',
    operationId: 'human-gloss', attemptId: 'attempt', coverage: 'partial',
    segments: [{ start: 0, end: 4, kind: 'gloss', gloss: 'Hello' }],
  }
  input.onRetryGloss = vi.fn()
  const view = render(<TurnView {...input} />)
  fireEvent.click(screen.getByRole('button', { name: 'Translate your message' }))
  openMenus()
  const words = screen.getByRole('button', { name: 'Words' })
  expect(words).toBeEnabled()
  expect(words).toHaveClass('is-hydrating')
  fireEvent.click(words)
  expect(view.container.querySelector('.msg.me .wg')).toBeNull()
  fireEvent.click(words)
  expect(words).toHaveAttribute('aria-pressed', 'true')
  expect(view.container.querySelector('.msg.me .wg')).toHaveTextContent('Hello')
  expect(screen.queryByText('Hello there')).toBeNull()
  fireEvent.click(words)
  expect(view.container.querySelector('.msg.me .wg')).toBeNull()
  expect(input.onRetryGloss).not.toHaveBeenCalled()
  expect(input.onAskCoach).not.toHaveBeenCalled()
})

it('keeps word actions disabled without annotations and allows them when saved meanings arrive', () => {
  const input = props()
  input.turn.assistant!.user_tokens = []
  input.turn.assistant!.tokens = []
  input.turn.userGlossState = 'running'
  input.turn.assistant!.glossState = 'failed'
  const view = render(<TurnView {...input} />)
  openMenus()
  for (const words of screen.getAllByRole('button', { name: 'Words' })) {
    expect(words).toBeDisabled()
    expect(words).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(words)
  }
  const token = { text: 'Hola', gloss: 'Hello', pos: null, notable: false, romanization: null, pronunciation: null }
  view.rerender(<TurnView {...input} turn={{ ...input.turn, assistant: { ...input.turn.assistant!, user_tokens: [token] } }} />)
  openMenus()
  expect(within(view.container.querySelector('.msg.me') as HTMLElement).getByRole('button', { name: 'Words' })).toBeEnabled()
})

it('reveals Arabic token meanings through the shaping-safe renderer on either side', () => {
  const input = props()
  input.autoTranslate = true
  const token = { text: 'بيوت', gloss: 'houses', romanization: 'buyūt', pronunciation: null, pos: null, notable: false }
  input.turn.user = 'بيوت'
  input.turn.assistant!.reply = 'بيوت'
  input.turn.assistant!.tokens = [token]
  input.turn.assistant!.user_tokens = [token]
  const view = render(<TurnView {...input} rtl />)
  const learner = view.container.querySelector('.msg.me') as HTMLElement
  const partner = view.container.querySelector('.msg.bot') as HTMLElement
  openMenus()
  fireEvent.click(within(partner).getByRole('button', { name: 'Words' }))
  openMenus()
  fireEvent.click(within(learner).getByRole('button', { name: 'Words' }))
  openMenus()
  fireEvent.click(within(learner).getByRole('button', { name: 'Words' }))
  expect(learner.querySelector('.wg')).toHaveTextContent('houses')
  expect(partner.querySelector('.wg')).toBeNull()
  openMenus()
  fireEvent.click(within(partner).getByRole('button', { name: 'Words' }))
  expect(partner.querySelector('.wg')).toHaveTextContent('houses')
})


it('keeps assistance in interface direction independently of the source script', async () => {
  const input = props()
  document.documentElement.dir = 'rtl'
  const view = render(<TurnView {...input} rtl={false} />)
  expect(view.container.querySelector('.message-feedback-line')).toHaveAttribute('dir', 'rtl')
  expect(view.container.querySelector('.msg.me .message-actions')).toHaveAttribute('dir', 'rtl')
  expect(view.container.querySelector('.msg.bot .message-actions')).toHaveAttribute('dir', 'rtl')
  await act(async () => { document.documentElement.dir = 'ltr' })
  view.rerender(<TurnView {...input} rtl />)
  expect(view.container.querySelector('.message-feedback-line')).toHaveAttribute('dir', 'ltr')
  expect(view.container.querySelector('.msg.me .message-actions')).toHaveAttribute('dir', 'ltr')
  expect(view.container.querySelector('.msg.bot .message-actions')).toHaveAttribute('dir', 'ltr')
})

it.each(['tokens', 'saved', 'joining'] as const)('updates enabled aids on both message sides through the %s path', path => {
  const input = props()
  const text = path === 'joining' ? 'بيوت' : 'Hola'
  const token = {text, gloss:'meaning', romanization:'roman', pronunciation:'redundant', pos:null, notable:false}
  input.turn.user = text
  input.turn.assistant!.reply = text
  input.turn.assistant!.tokens = [token]
  input.turn.assistant!.user_tokens = [token]
  if (path === 'saved') {
    input.turn.userSavedGloss = input.turn.assistant!.savedGloss = {
      sourceMessageId:'message', targetLanguageId:'language', explanationLanguageId:'english',
      formatVersion:'format', templateVersion:'template', boundaryPolicy:'policy',
      operationId:'operation', attemptId:'attempt', coverage:'complete',
      segments:[{start:0,end:text.length,kind:'gloss',gloss:'meaning',romanization:'roman',pronunciation:'redundant'}],
    }
  }
  const view = render(<TurnView {...input} autoTranslate alwaysRomanize alwaysPronunciation showRomanization />)
  for (const side of ['me','bot']) {
    const bubble = view.container.querySelector(`.msg.${side}`) as HTMLElement
    expect(bubble.querySelector('.wg')).toHaveTextContent('meaning')
    expect(bubble.querySelector('.wroman')).toHaveTextContent('roman')
    expect(bubble.querySelector('.wpronunciation')).toBeNull()
    openMenus()
    fireEvent.click(within(bubble).getByRole('button', {name:'Words'}))
    expect(bubble.querySelector('.wg')).toBeNull()
  }
  view.rerender(<TurnView {...input} autoTranslate={false} alwaysRomanize={false} alwaysPronunciation showRomanization />)
  expect(view.container.querySelector('.wg,.wroman,.wpronunciation')).toBeNull()
  view.rerender(<TurnView {...input} autoTranslate alwaysRomanize alwaysPronunciation showRomanization />)
  expect(view.container.querySelectorAll('.wg')).toHaveLength(2)
  expect(view.container.querySelectorAll('.wroman')).toHaveLength(2)
  expect(input.onAskCoach).not.toHaveBeenCalled()
})

it.each(['tokens', 'saved', 'joining'] as const)('Words explicitly reveals and hides each side with all defaults off (%s)', path => {
  const input = props()
  input.showRomanization = true
  const text = path === 'joining' ? 'بيوت باب' : 'Hola casa'
  const words = text.split(' ')
  const tokens = words.map((text, index) => ({text, gloss:index ? 'second meaning' : 'meaning', romanization:index ? null : 'roman', pronunciation:index ? 'fallback' : 'redundant', pos:null, notable:false}))
  input.turn.user = text
  input.turn.assistant!.reply = text
  input.turn.assistant!.tokens = tokens
  input.turn.assistant!.user_tokens = tokens
  if (path === 'saved') {
    input.turn.userSavedGloss = input.turn.assistant!.savedGloss = {
      sourceMessageId:'message', targetLanguageId:'language', explanationLanguageId:'english',
      formatVersion:'format', templateVersion:'template', boundaryPolicy:'policy',
      operationId:'operation', attemptId:'attempt', coverage:'complete',
      segments:tokens.map((token,index) => ({start:index ? words[0].length+1 : 0,end:index ? text.length : words[0].length,kind:'gloss',gloss:token.gloss,romanization:token.romanization ?? undefined,pronunciation:token.pronunciation})),
    }
  }
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.wg,.wroman,.wpronunciation')).toBeNull()
  for (const side of ['me','bot']) {
    const bubble = view.container.querySelector(`.msg.${side}`) as HTMLElement
    openMenus()
    const button = within(bubble).getByRole('button',{name:'Words'})
    expect(button).toHaveAttribute('aria-pressed','false')
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-pressed','true')
    expect(within(bubble).getByText('meaning')).toBeVisible()
    expect(within(bubble).getByText('roman')).toBeVisible()
    expect(within(bubble).getByText('fallback')).toBeVisible()
    expect(within(bubble).queryByText('redundant')).toBeNull()
    expect(within(bubble).queryByText('Persona translation')).toBeNull()
    expect(within(bubble).queryByText('Learner translation')).toBeNull()
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-pressed','false')
    expect(bubble.querySelector('.wg,.wroman,.wpronunciation')).toBeNull()
  }
  expect(input.onAskCoach).not.toHaveBeenCalled()
})

it('preserves source text and reading controls without inline XP tags', async () => {
  const { SkillEvidenceContext } = await import('../../../state/learning/useSkillEvidence')
  const { PracticeContext } = await import('../session/PracticeContext')
  const { RewardInspectionContext } = await import('../progress/RewardInspectionContext')
  const { skillDemo } = await import('../../../domain/learning/catalog/skillDemo')
  const { unreportedInput } = await import('../../../domain/learning/evidence/skills')
  const snapshot = structuredClone(skillDemo)
  snapshot.records = [{ attempt_id: 'jev', session_id: 'test', turn_id: 1, message_id: 1, replaces_message_id: null, construct_registry_hash: snapshot.construct_registry_hash, mapping_error: null, support_step: null, chat_id: 'chat', learner_id: snapshot.learner_id, target: snapshot.target, native: 'english', source: 'Hola', input: unreportedInput(), at_secs: 1, model: 'typesafe/jev-1.13', provider_mode: 'custom', catalog_version: snapshot.catalog_version, prompt_version: 'jev-choice-assessment-1', assessment_adapter: 'jev_choice', status: 'complete', error: null, assessment: { judgments: [{ skill_id: 'people_places', presence: 'direct', quotes: ['Hola'], rationale: '', evidence_kind: 'quoted' }] } }]
  snapshot.profile.credits = [{ attempt_id: 'jev', skill_id: 'people_places', xp: 10 }]
  const view = render(<SkillEvidenceContext value={{ snapshot, error: null }}><PracticeContext value={{ chatId: 'chat', selectionVersion: 0, selected: null, select: vi.fn() }}><RewardInspectionContext value={{ arrive: vi.fn() }}><TurnView {...props()} /></RewardInspectionContext></PracticeContext></SkillEvidenceContext>)
  expect(view.container.querySelector('.msg.me')).toHaveTextContent('Hola')
  expect(view.container.querySelector('.message-evidence')).toBeNull()
  expect(view.container.querySelector('.message-credit-badges')).toBeNull()
  expect(screen.queryByRole('button', { name: /Inspect .* XP/ })).toBeNull()
  fireEvent.click(screen.getAllByRole('button', { name: 'Hola' })[0])
  expect(view.container.querySelector('.message-credit-badges')).toBeNull()
  expect(view.container.querySelector('.whole-message-credit-source')).toBeNull()
})

it('carries pending translation on the Translate control unless the translation is set to show', () => {
  const input = props()
  input.turn.assistant!.translation = null
  input.turn.assistant!.translationState = 'running'
  const view = render(<TurnView {...input} />)
  const bubble = view.container.querySelector('.msg.bot') as HTMLElement
  expect(bubble.querySelector('.hydrating-slot')).toBeNull()
  expect(within(bubble).getByRole('status')).toHaveClass('hydrating-announce')
  expect(within(bubble).getByRole('button', { name: 'Translate partner message' })).toHaveClass('is-hydrating')
  view.rerender(<TurnView {...input} autoTranslate />)
  expect(bubble.querySelector('.hydrating-slot')).toHaveTextContent('Translating…')
  view.rerender(<TurnView {...input} autoTranslate turn={{ ...input.turn, assistant: { ...input.turn.assistant!, translationState: 'succeeded', translation: 'Arrived' } }} />)
  expect(bubble.querySelector('.hydrating-slot')).toBeNull()
  expect(within(bubble).getByText('Arrived')).toBeVisible()
})

it('keeps pending word meanings out of the layout and follow-on activity out of the thread', () => {
  const input = props()
  input.turn.assistant!.glossState = 'running'
  input.turn.execution = { id: 'turn', state: 'assisting', paused: false, hold: null, operations: [{ id: 'reply', kind: 'persona_reply', state: 'succeeded' }, { id: 'gloss', kind: 'word_gloss', state: 'running' }], attempts: [] } as never
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.msg.bot .trans[aria-label="Word meanings"]')).toBeNull()
  expect(screen.getByText('Word meanings pending')).toHaveClass('hydrating-announce')
  expect(view.container.querySelector('.turn-activity')).toBeNull()
})

it('shows coaching progress and then error marks without opening Analysis, independently of Jev', () => {
  const input = props()
  input.turn.conversationFeedback = { grammar: 4, conversation: 3 } as never
  input.turn.feedbackState = 'running'
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.message-coaching-pending')).toHaveTextContent('Hola')
  expect(view.container.querySelector('.msg.bot')).toHaveTextContent('Hola')
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  input.turn.feedbackState = 'succeeded'
  input.turn.coach = { issues: [{ quote: 'Hola', severity: 'error' }], corrections: [], notes: [], meaningRecovered: 'full', candidatesSent: 1, itemsReturned: 1, items: [] }
  view.rerender(<TurnView {...input} />)
  expect(view.container.querySelector('.message-coaching-pending')).toBeNull()
  expect(view.container.querySelector('.msg.me .coach-flag')).toHaveTextContent('Hola')
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  expect(requestHelp).not.toHaveBeenCalled()
})

it.each(['succeeded', 'failed', 'unknown', 'cancelled', 'held'])('stops the coaching underline when work is %s', state => {
  const input = props()
  input.turn.feedbackState = state
  const view = render(<TurnView {...input} reviewing />)
  expect(view.container.querySelector('.message-coaching-pending')).toBeNull()
})

it('keeps the feedback line under the learner message in every feedback state without replacing the bubble', () => {
  const input = props()
  const view = render(<TurnView {...input} reviewing />)
  const message = view.container.querySelector('.msg.me') as HTMLElement
  expect(message).toHaveClass('with-actions')
  view.rerender(<TurnView {...input} reviewing={false} turn={{ ...input.turn, conversationFeedback: { grammar: 4, conversation: 3 } as never }} />)
  expect(view.container.querySelector('.msg.me')).toBe(message)
  expect(message).toHaveClass('with-actions')
  const badge = screen.getByRole('button', { name: 'Coach feedback for message 1' })
  expect(badge.closest('.learner-turn')).toBe(message.closest('.learner-turn'))
  expect(badge).toHaveTextContent('Feedback')
  expect(badge).not.toHaveTextContent('Clean')
  expect(badge.querySelector('.coach-meter')).toBeNull()
})

it('inspects the learner message only when that message owns the recording, closed by default, with one Play and the full dialog one press away', () => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null)
  const expand = vi.fn()
  const bubble = vi.fn()
  const result: TranscriptionInspectionResult = {
    text: 'Hola', audioBase64: 'UklGRg==', diagnostics: null,
    inspection: { recordingId: 'recording', owner: { kind: 'conversation', id: 'chat' }, duration: 1, sampleRate: 16000,
      waveform: { binSeconds: 0.5, min: [-0.4, -0.2], max: [0.4, 0.2] },
      spectrogram: { frameSeconds: 0.5, frameStartSeconds: [0, 0.5], windowSeconds: 0.025, fftSize: 512, bands: [{ lowHz: 50, centerHz: 100, highHz: 200 }], minFrequencyHz: 50, maxFrequencyHz: 200, measuredMaxFrequencyHz: 200, melScale: 'htk', normalization: 'unit-peak triangular filters', dbReference: '0 dB = full-scale power (1.0)', dbMin: -80, dbMax: 0, bins: [[-60], [-70]] },
      activity: { algorithm: 'fixture', noiseFloorDbfs: -60, thresholdDbfs: -40, regions: [{ start: 0.1, end: 0.9 }], pauses: [], limitations: [] },
      wordTiming: { status: 'available', reason: null, words: [{ index: 0, word: 'Hola', providerStart: 0.1, providerEnd: 0.9, start: 0.1, end: 0.9, clipped: false }], unsupported: [] } },
  }
  const view = render(<TurnView {...props()} onBubbleTap={bubble} />)
  expect(document.querySelector('.msg.me')!.querySelector('[aria-label^="Inspect recording"]')).toBeNull()
  view.rerender(<TurnView {...props()} onBubbleTap={bubble} recording={{ result, rate: 1, volume: 1, enabled: true, onExpand: expand }} />)
  const button = screen.getByRole('button', { name: 'Inspect recording' })
  const learner = document.querySelector('.msg.me') as HTMLElement
  expect(button.closest('.msg.me')).toBe(learner)
  expect(button).toHaveAttribute('aria-pressed', 'false')
  expect(screen.queryByRole('region', { name: 'Recording inspection' })).toBeNull()
  // One Play on the spoken message, and it plays the recording the inspector draws.
  expect(within(learner).getAllByRole('button', { name: 'Play' })).toHaveLength(1)
  fireEvent.click(button)
  expect(button).toHaveAttribute('aria-pressed', 'true')
  const inspector = screen.getByRole('region', { name: 'Recording inspection' })
  expect(inspector.closest('.msg.me')).toBe(learner)
  expect(within(inspector).queryByRole('button', { name: 'Play' })).toBeNull()
  expect(inspector.querySelector('.timed-words[data-placement="overlay"]')).not.toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Recording inspection' }))
  expect(expand).toHaveBeenCalledOnce()
  fireEvent.click(button)
  expect(button).toHaveAttribute('aria-pressed', 'false')
  expect(screen.queryByRole('region', { name: 'Recording inspection' })).toBeNull()
  expect(bubble).not.toHaveBeenCalled()
})

it('marks the message being fixed and leaves other turns unmarked', () => {
  const input = props()
  const view = render(<TurnView {...input} editing />)
  expect(screen.getByText('Fixing this message')).toBeVisible()
  expect(view.container.querySelector('.turn-stack')).toHaveAttribute('data-editing')
  view.rerender(<TurnView {...input} editing={false} />)
  expect(screen.queryByText('Fixing this message')).toBeNull()
  expect(view.container.querySelector('.turn-stack')).not.toHaveAttribute('data-editing')
})

it('squiggles the phrases the coach flagged and leaves the edit to Fix it', () => {
  const input = props()
  input.onEditUser = vi.fn()
  input.turn.user = 'Me gustan son los tacos'
  input.turn.assistant!.user_tokens = []
  input.turn.coach = { issues: [{ quote: 'son', severity: 'error' }, { quote: 'los tacos', severity: 'partial' }], corrections: [], notes: [], meaningRecovered: 'full', candidatesSent: 1, itemsReturned: 2, items: [
    { construct: 'a', quote: 'son', outcome: 'not_demonstrated', rationale: '' },
    { construct: 'b', quote: 'los tacos', outcome: 'partial', rationale: '' },
  ] }
  const view = render(<TurnView {...input} />)
  const flagged = [...view.container.querySelectorAll('.coach-flag')].map(node => [node.textContent, node.getAttribute('data-severity')])
  expect(flagged).toEqual([['son', 'error'], ['los tacos', 'partial']])
  expect(screen.getByRole('button', { name: 'Fix it' })).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Edit message' })).toBeNull()
})

it('does not underline a corrected message for evidence without a correction', () => {
  const input = props()
  input.turn.user = 'Oui, je vais le porter.'
  input.turn.assistant!.user_tokens = []
  input.turn.coach = { issues: [], corrections: [], notes: [], meaningRecovered: 'full', candidatesSent: 1, itemsReturned: 1,
    items: [{ construct: 'ability_permission_necessity', quote: 'je vais le porter.', outcome: 'not_demonstrated', rationale: '' }] }
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.coach-flag')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Fix it' })).toBeNull()
})

it('selects the learner and partner separately without capturing embedded word or audio actions', () => {
  const onSelectMessage = vi.fn()
  const input = props()
  const view = render(<TurnView {...input} onSelectMessage={onSelectMessage} selectedSide="user" />)
  const learner = screen.getByRole('group', { name: 'Your message' })
  const partner = screen.getByRole('group', { name: 'Partner replied' })
  expect(learner).toHaveClass('focused')
  expect(partner).not.toHaveClass('focused')
  fireEvent.click(partner)
  expect(onSelectMessage).toHaveBeenLastCalledWith(1, 'assistant')
  fireEvent.keyDown(learner, { key: 'Enter' })
  expect(onSelectMessage).toHaveBeenLastCalledWith(1, 'user')
  onSelectMessage.mockClear()
  fireEvent.click(within(learner).getByRole('button', { name: 'Hola' }))
  fireEvent.click(within(partner).getByRole('button', { name: 'Play' }))
  expect(onSelectMessage).not.toHaveBeenCalled()
  view.rerender(<TurnView {...input} onSelectMessage={onSelectMessage} selectedSide="assistant" />)
  expect(partner).toHaveAttribute('aria-current', 'true')
  expect(learner).not.toHaveClass('focused')
})
it('counts the fixes sent for a message under its bubble, beside its XP', () => {
  const input = props()
  const view = render(<TurnView {...input} />)
  expect(view.container.querySelector('.feedback-fixes')).toBeNull()
  view.rerender(<TurnView {...input} turn={{ ...input.turn, replacesTurnId: 'earlier', fixes: 2 }} />)
  const fixes = view.container.querySelector('.feedback-fixes')!
  expect(fixes).toHaveTextContent('2 fixes')
  expect(fixes.closest('.message-feedback-line')).not.toBeNull()
  expect(screen.queryByText('Fixed')).toBeNull()
})

it('gives each selectable bubble its own selection ring, hidden from assistive technology', () => {
  const input = props()
  const view = render(<TurnView {...input} onSelectMessage={vi.fn()} selectedSide="user" />)
  // The ring is drawn by the bubble's stylesheet round the bubble and its tail;
  // it is decoration only, so it carries no text and no role.
  for (const name of ['Your message', 'Partner replied']) {
    const ring = screen.getByRole('group', { name }).querySelector(':scope > .msg-selection')
    expect(ring).toHaveAttribute('aria-hidden', 'true')
    expect(ring?.textContent).toBe('')
  }
  view.rerender(<TurnView {...input} />)
  expect(view.container.querySelector('.msg-selection')).toBeNull()
})
