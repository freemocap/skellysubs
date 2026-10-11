// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { CoachAnalysisPanel } from './CoachAnalysisPanel'
import type { ConversationSnapshot, Snapshot } from '../../../generated/contracts'

const backend = vi.hoisted(() => ({ read: vi.fn(), execute: vi.fn(), watch: vi.fn() }))
vi.mock('../../../platform/ipc/tauri', () => ({ isTauri: true }))
vi.mock('../../../platform/ipc/workspace', () => ({ readWorkspace: backend.read, executeAction: backend.execute, watchConversation: backend.watch, nativeError: (error: unknown) => String(error) }))
vi.mock('../../../domain/input/back', () => ({ openOverlay: () => () => {} }))
vi.mock('../progress/ConversationMap', () => ({ ConversationMap: () => null }))
const snapshot = { conversationId: 'chat-1', sessionId: 'session', revision: 7, coachMessages: [], turns: [], messages: [], revisionSuffixCounts: [], hasOlder: false } as unknown as ConversationSnapshot
function panel(chatId = 'chat-1', draftQuestion = '', autoSendDraft = false, conversationBusy = false, revealRequest = 0) {
  return <CoachAnalysisPanel targetLanguageName="Spanish" autoSendDraft={autoSendDraft} revealRequest={revealRequest} chatId={chatId} conversationBusy={conversationBusy} tab="coaching" onTab={vi.fn()} draftQuestion={draftQuestion} onDraftConsumed={vi.fn()} pinnedTurn={null} nativeLanguageName="English" showRomanization={false} rtl={false} />
}
beforeEach(() => {
  vi.resetAllMocks()
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open') }
  backend.watch.mockResolvedValueOnce(snapshot).mockImplementation(() => new Promise(() => {}))
  backend.read.mockResolvedValue({ sessionId: 'session', conversations: [{ id: 'chat-1', archived: false, revision: 19 }] } as unknown as Snapshot)
  backend.execute.mockResolvedValue({ revision: 20 })
})
describe('native private coaching', () => {
  it('reveals an externally submitted guide question without resubmitting and respects subsequent minimization', async () => {
    backend.watch.mockReset().mockResolvedValueOnce({ ...snapshot, coachMessages: [{ id: 'guide', role: 'user', text: 'Explain this skill', guideContext: { markdown: 'Selected skill context' } }] }).mockImplementation(() => new Promise(() => {}))
    const view = render(panel())
    await screen.findByText('Explain this skill')
    expect(screen.getByLabelText('Coach conversation')).not.toBeVisible()
    view.rerender(panel('chat-1', '', false, false, 1))
    expect(screen.getByLabelText('Coach conversation')).toBeVisible()
    expect(screen.getByText('Skill guide')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Minimize coach chat' }))
    view.rerender(panel('chat-1', '', false, true, 1))
    expect(screen.getByLabelText('Coach conversation')).not.toBeVisible()
    view.rerender(panel('chat-1', '', false, false, 2))
    expect(screen.getByLabelText('Coach conversation')).toBeVisible()
    expect(backend.execute).not.toHaveBeenCalled()
  })
  it('uses declared coach ownership and native previews without legacy operations', async () => {
    backend.watch.mockReset().mockResolvedValueOnce({ ...snapshot, turns: [{ id: 'native', channel: 'coach', state: 'pending', operations: [], attempts: [],
      nativePreview: { live: true, capture: { text: 'Streaming native reply' } } }] }).mockImplementation(() => new Promise(() => {}))
    render(panel())
    await waitFor(() => expect(screen.getByLabelText('Coach conversation')).toHaveTextContent('Streaming native reply'))
    expect(screen.getByLabelText('Message your coach')).toBeDisabled()
    expect(backend.execute).not.toHaveBeenCalled()
  })
  it('observes saved messages without requesting AI and exposes unavailable lesson controls', async () => {
    backend.watch.mockReset().mockResolvedValueOnce({ ...snapshot, coachMessages: [{ id: 'm', role: 'assistant', text: 'Saved private answer' }] }).mockImplementation(() => new Promise(() => {}))
    render(panel())
    await waitFor(() => expect(screen.getByLabelText('Coach conversation')).toHaveTextContent('Saved private answer'))
    expect(backend.watch).toHaveBeenNthCalledWith(2, 'chat-1', 7)
    expect(backend.execute).not.toHaveBeenCalled()
    expect(screen.getByLabelText('Coach conversation')).not.toBeVisible()
    expect(screen.getByRole('tab', { name: 'Coach' })).toBeVisible(); expect(screen.queryByRole('tab', { name: 'Analysis' })).toBeNull()
    expect(screen.getByRole('tab', { name: 'Progress' })).toBeVisible()
    expect(screen.getAllByText('Coach')).toHaveLength(1)
  })
  it('submits once with current conversation revision and displays only saved replies', async () => {
    render(panel())
    await waitFor(() => expect(backend.watch).toHaveBeenCalledTimes(2))
    fireEvent.change(screen.getByLabelText('Message your coach'), { target: { value: 'Explain this' } })
    fireEvent.submit(screen.getByLabelText('Message your coach').closest('form')!)
    fireEvent.submit(screen.getByLabelText('Message your coach').closest('form')!)
    await waitFor(() => expect(backend.execute).toHaveBeenCalledTimes(1))
    expect(backend.execute).toHaveBeenCalledWith(expect.objectContaining({ sessionId: 'session' }), { kind: 'askCoach', conversationId: 'chat-1', text: 'Explain this', expectedRevision: 19 })
    await waitFor(() => expect(screen.getByLabelText('Message your coach')).toHaveValue(''))
    expect(screen.getByLabelText('Coach conversation')).toBeVisible()
    expect(screen.queryByText('Explain this')).toBeNull()
  })
  it('retains input after rejection without an automatic retry', async () => {
    backend.execute.mockRejectedValue(new Error('Admission held'))
    render(panel())
    await waitFor(() => expect(backend.watch).toHaveBeenCalledTimes(2))
    fireEvent.change(screen.getByLabelText('Message your coach'), { target: { value: 'My question' } })
    fireEvent.click(screen.getByLabelText('Send to coach'))
    expect(await screen.findByRole('alert')).toHaveTextContent('Coach')
    expect(screen.getByText(/Admission held/)).toBeVisible()
    expect(screen.getByLabelText('Message your coach')).toHaveValue('My question')
    expect(backend.execute).toHaveBeenCalledTimes(1)
  })
  it('restores the resized chat on Enter submission and allows minimizing while the request runs', async () => {
    let publishReply!: (value: ConversationSnapshot) => void
    backend.watch.mockReset().mockResolvedValueOnce(snapshot)
      .mockImplementationOnce(() => new Promise<ConversationSnapshot>(resolve => { publishReply = resolve }))
      .mockImplementation(() => new Promise(() => {}))
    backend.execute.mockImplementation(() => new Promise(() => {}))
    render(panel())
    await waitFor(() => expect(backend.watch).toHaveBeenCalledTimes(2))
    const separator = screen.getByRole('separator', { name: 'Coach chat height' })
    fireEvent.keyDown(separator, { key: 'End' })
    fireEvent.keyDown(separator, { key: 'ArrowDown' })
    fireEvent.click(screen.getByRole('button', { name: 'Minimize coach chat' }))
    fireEvent.change(screen.getByLabelText('Message your coach'), { target: { value: 'Explain this' } })
    fireEvent.keyDown(screen.getByLabelText('Message your coach'), { key: 'Enter' })
    await waitFor(() => expect(backend.execute).toHaveBeenCalledTimes(1))
    expect(separator).toHaveAttribute('aria-valuenow', '65')
    fireEvent.click(screen.getByRole('button', { name: 'Minimize coach chat' }))
    publishReply({ ...snapshot, revision: 8, coachMessages: [{ id: 'reply', role: 'assistant', text: 'Saved new reply' }] } as ConversationSnapshot)
    await screen.findByText('Saved new reply')
    expect(screen.getByLabelText('Coach conversation')).not.toBeVisible()
  })
  it('keeps the chat minimized for blank or blocked submissions', async () => {
    const view = render(panel())
    await waitFor(() => expect(backend.watch).toHaveBeenCalledTimes(2))
    const input = screen.getByLabelText('Message your coach')
    fireEvent.change(input, { target: { value: '   ' } })
    fireEvent.submit(input.closest('form')!)
    view.rerender(panel('chat-1', '', false, true))
    fireEvent.change(input, { target: { value: 'Explain this' } })
    fireEvent.submit(input.closest('form')!)
    expect(backend.execute).not.toHaveBeenCalled()
    expect(screen.getByLabelText('Coach conversation')).not.toBeVisible()
  })
  it('does not publish an old watch result after changing conversations', async () => {
    let resolveOld!: (value: ConversationSnapshot) => void
    backend.watch.mockReset().mockImplementationOnce(() => new Promise<ConversationSnapshot>(resolve => { resolveOld = resolve }))
      .mockResolvedValueOnce({ ...snapshot, conversationId: 'chat-2' }).mockImplementation(() => new Promise(() => {}))
    const view = render(panel())
    view.rerender(panel('chat-2'))
    resolveOld({ ...snapshot, coachMessages: [{ id: 'old', role: 'assistant', text: 'Old private answer' }] } as ConversationSnapshot)
    await waitFor(() => expect(backend.watch).toHaveBeenCalledWith('chat-2', 7))
    expect(screen.queryByText('Old private answer')).toBeNull()
  })
  it('places a supplied draft in the study composer without dispatching or opening a dialog', async () => {
    render(panel('chat-1', 'Explain this phrase'))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(screen.getByLabelText('Message your coach')).toHaveValue('Explain this phrase')
    expect(backend.execute).not.toHaveBeenCalled()
  })
})

it('offers read-only reconnection after coach observation fails without losing the draft', async () => {
  backend.watch.mockReset().mockRejectedValueOnce(new Error('Read disconnected')).mockImplementation(() => new Promise(() => {}))
  render(panel())
  await screen.findByText(/Read disconnected/)
  fireEvent.change(screen.getByLabelText('Message your coach'), { target: { value: 'Keep my draft' } })
  backend.watch.mockResolvedValueOnce(snapshot)
  fireEvent.click(screen.getByRole('button', { name: 'Retry reading conversation' }))
  await waitFor(() => expect(backend.watch).toHaveBeenCalledTimes(3))
  expect(screen.queryByText(/Read disconnected/)).toBeNull()
  expect(screen.getByLabelText('Message your coach')).toHaveValue('Keep my draft')
  expect(backend.execute).not.toHaveBeenCalled()
})


it('sends an external Ask action once through the normal coach submission path', async () => {
  const view = render(panel('chat-1', 'Explain this token', true, true))
  await waitFor(() => expect(backend.watch).toHaveBeenCalledTimes(2))
  expect(backend.execute).not.toHaveBeenCalled()
  view.rerender(panel('chat-1', 'Explain this token', true))
  await waitFor(() => expect(backend.execute).toHaveBeenCalledTimes(1))
  expect(backend.execute).toHaveBeenCalledWith(expect.anything(), {
    kind: 'askCoach', conversationId: 'chat-1', text: 'Explain this token', expectedRevision: 19,
  })
  await waitFor(() => expect(screen.getByLabelText('Message your coach')).toHaveValue(''))
  expect(screen.getByLabelText('Coach conversation')).toBeVisible()
  view.rerender(panel('chat-1', 'Explain this token', true))
  expect(backend.execute).toHaveBeenCalledTimes(1)
})

it('retains a rejected automatic question for explicit retry without resending it', async () => {
  backend.execute.mockRejectedValue(new Error('Admission held'))
  const view = render(panel('chat-1', 'Explain this token', true))
  await screen.findByRole('alert')
  expect(screen.getByLabelText('Message your coach')).toHaveValue('Explain this token')
  view.rerender(panel('chat-1', 'Explain this token', true))
  expect(backend.execute).toHaveBeenCalledTimes(1)
})
