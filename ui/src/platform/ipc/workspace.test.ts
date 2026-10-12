import { expect, it, vi } from 'vitest'
import type { ConversationSnapshot } from '../../generated/contracts'
const call = vi.hoisted(() => vi.fn())
vi.mock('./native', () => ({ invoke: call }))
import { readWorkspace, watchConversation } from './workspace'

it('shares concurrent identical watches, then reads afresh after completion or failure', async () => {
  let resolve!: (value: ConversationSnapshot) => void
  call.mockReturnValueOnce(new Promise<ConversationSnapshot>(done => { resolve = done }))
  const first = watchConversation('chat', 3)
  const second = watchConversation('chat', 3)
  expect(first).toBe(second)
  expect(call).toHaveBeenCalledTimes(1)
  const result = { conversationId: 'chat', revision: 4, turns: [] } as unknown as ConversationSnapshot
  resolve(result)
  expect(await first).toBe(result)
  call.mockRejectedValueOnce(new Error('read failed'))
  await expect(watchConversation('chat', 3)).rejects.toThrow('read failed')
  call.mockResolvedValue(result)
  await Promise.all([watchConversation('chat', 3), watchConversation('chat', 4), watchConversation('other', 3), watchConversation('chat', 3, 10)])
  expect(call).toHaveBeenCalledTimes(6)
})


it('workspace consumers share a burst without swallowing an invalidation during the read', async () => {
  call.mockReset()
  let finish!: (value: { revision: number }) => void
  call.mockReturnValueOnce(new Promise(resolve => { finish = resolve })).mockResolvedValue({ revision: 2 })
  const first = readWorkspace()
  const second = readWorkspace()
  const third = readWorkspace()
  await Promise.resolve()
  expect(call).toHaveBeenCalledTimes(1)
  expect(call).toHaveBeenCalledWith('get_snapshot')
  await expect(readWorkspace()).resolves.toEqual({ revision: 2 })
  expect(call).toHaveBeenCalledTimes(2)
  finish({ revision: 1 })
  expect(await Promise.all([first, second, third])).toEqual([{ revision: 1 }, { revision: 1 }, { revision: 1 }])
  call.mockRejectedValueOnce(new Error('Read failed'))
  await expect(readWorkspace()).rejects.toThrow('Read failed')
  await expect(readWorkspace()).resolves.toEqual({ revision: 2 })
})
