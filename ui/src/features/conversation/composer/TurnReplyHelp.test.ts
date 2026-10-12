import { beforeEach, expect, it, vi } from 'vitest'
import { requestReplyHelp } from './TurnReplyHelp'
const backend = vi.hoisted(() => ({ executeAction: vi.fn(), readWorkspace: vi.fn(), watchConversation: vi.fn() }))
vi.mock('../../../platform/ipc/workspace', () => backend)
function snapshot(state?: string) {
  return {messages:[{id:'m',turnId:'t',explanationsState:state}],turns:[{id:'t'}]}
}
beforeEach(() => {
  vi.clearAllMocks()
  backend.readWorkspace.mockResolvedValue({sessionId:'session'})
  backend.watchConversation.mockResolvedValue(snapshot())
  backend.executeAction.mockResolvedValue({entityId:'op'})
})
it('requests grammar without waiting for a conversation projection', async () => {
  backend.watchConversation.mockReturnValue(new Promise(() => {}))
  const pending = requestReplyHelp('m','grammar')
  await vi.waitFor(() => expect(backend.executeAction).toHaveBeenCalledOnce())
  await pending
  expect(backend.watchConversation).not.toHaveBeenCalled()
  expect(backend.executeAction).toHaveBeenCalledExactlyOnceWith({sessionId:'session'},{kind:'requestExplanations',messageId:'m'})
})
it('requests an absent brief without treating its first request as a retry', async () => {
  await requestReplyHelp('m', 'brief')
  expect(backend.executeAction).toHaveBeenCalledExactlyOnceWith({ sessionId: 'session' }, { kind: 'requestMessageHelp', messageId: 'm', help: 'reply_brief', retry: false })
})
it('sends explicit retries to native admission without reconstructing help state', async () => {
  await requestReplyHelp('m','grammar',true)
  expect(backend.executeAction).toHaveBeenCalledExactlyOnceWith({sessionId:'session'},{kind:'retryReplyHelp',messageId:'m',helpKind:'grammar'})
  expect(backend.watchConversation).not.toHaveBeenCalled()
})
it('propagates native source rejection without substituting another request', async () => {
  const failure = new Error('Source unavailable')
  backend.executeAction.mockRejectedValueOnce(failure)
  await expect(requestReplyHelp('m', 'replies')).rejects.toBe(failure)
  expect(backend.executeAction).toHaveBeenCalledOnce()
})
