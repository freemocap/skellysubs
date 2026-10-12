// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import type { ChatMessage, ConversationSnapshot, TurnView } from '../../../generated/contracts'
import { watchConversation } from '../../../platform/ipc/workspace'
import { mergeConversationPages, useConversationSnapshot } from './useConversationSnapshot'
vi.mock('../../../platform/ipc/workspace', () => ({ watchConversation: vi.fn(), nativeError: String }))
const watch = vi.mocked(watchConversation)
function messages(first: number, last: number): ChatMessage[] {
  return Array.from({ length: last - first + 1 }, (_, index) => {
    const sequence = first + index
    return { id: `m${sequence}`, sequence, turnId: `t${Math.ceil(sequence / 2)}`, role: sequence % 2 ? 'user' : 'assistant', text: `Message ${sequence}`, replacesTurnId: null, replacedBy: null } as ChatMessage
  })
}
function page(first: number, last: number, revision = 1, chatId = 'chat'): ConversationSnapshot {
  return { sessionId: 'session', conversationId: chatId, revision, contentRevision: String(revision), messages: messages(first, last), turns: [], revisionSuffixCounts: [], hasOlder: first > 1 } as unknown as ConversationSnapshot
}
function deferred<T>() { let resolve!: (value: T) => void; let reject!: (reason: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject } }
beforeEach(() => { vi.resetAllMocks(); watch.mockImplementation(() => new Promise(() => {})) })
it.each([undefined, null, '', 'invalid', 1])('rejects an incompatible native content revision %s before showing a false empty conversation', async contentRevision => {
  const initial = { ...page(1, 0), contentRevision } as unknown as ConversationSnapshot
  watch.mockResolvedValueOnce(initial)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(hook.result.current.readError).toContain('native app and interface are incompatible'))
  expect(hook.result.current.snapshot).toBeNull()
  expect(watch).toHaveBeenCalledTimes(1)
})

it('stops observation when a later response loses its content revision instead of silently retaining stale content', async () => {
  const update = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(1, 0)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  const changed = { ...page(1, 1, 2), contentRevision: undefined } as unknown as ConversationSnapshot
  await act(async () => { update.resolve(changed) })
  await waitFor(() => expect(hook.result.current.readError).toContain('native app and interface are incompatible'))
  expect(hook.result.current.snapshot?.revision).toBe(1)
  expect(watch).toHaveBeenCalledTimes(2)
})

it('preserves loaded history and object identities through navigation, but publishes same-revision content changes', async () => {
  const navigation = deferred<ConversationSnapshot>(), update = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(3, 4)).mockImplementationOnce(() => navigation.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockResolvedValueOnce(page(1, 2))
  await act(async () => { await hook.result.current.loadOlder() })
  const previous = hook.result.current.snapshot!
  watch.mockImplementationOnce(() => update.promise)
  await act(async () => { navigation.resolve({ ...page(3, 4, 2), contentRevision: '1' }) })
  expect(hook.result.current.snapshot?.revision).toBe(2)
  expect(hook.result.current.snapshot?.messages).toBe(previous.messages)
  expect(hook.result.current.snapshot?.turns).toBe(previous.turns)
  expect(watch).toHaveBeenCalledTimes(4)
  watch.mockImplementation((_id, _revision, before) => before === 3 ? Promise.resolve(page(1, 2, 2)) : new Promise(() => {}))
  const changed = page(3, 4, 2)
  changed.messages[0].text = 'Updated without metadata revision'
  await act(async () => { update.resolve(changed) })
  await waitFor(() => expect(hook.result.current.snapshot?.messages[2].text).toBe('Updated without metadata revision'))
  expect(hook.result.current.snapshot?.messages).toHaveLength(4)
})

it('rejects a different session even when content and observation revisions match', async () => {
  const update = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(1, 2)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  await act(async () => { update.resolve({ ...page(1, 2), sessionId: 'replacement' }) })
  await waitFor(() => expect(hook.result.current.readError).toContain('scope mismatch'))
})

it('does not let delayed same-revision history overwrite newer content', async () => {
  const update = deferred<ConversationSnapshot>(), old = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(3, 4)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockImplementationOnce(() => old.promise)
  let loading!: Promise<void>
  act(() => { loading = hook.result.current.loadOlder() })
  const newer = { ...page(3, 4), contentRevision: '10' }
  newer.messages[0].text = 'Newest content'
  watch.mockImplementation((_id, _revision, before) => before === 3 ? Promise.resolve({ ...page(1, 2), contentRevision: '10' }) : new Promise(() => {}))
  await act(async () => { update.resolve(newer) })
  await waitFor(() => expect(hook.result.current.snapshot?.contentRevision).toBe('10'))
  await act(async () => { old.resolve({ ...page(1, 3), contentRevision: '9' }); await loading })
  expect(hook.result.current.snapshot?.messages).toHaveLength(4)
  expect(hook.result.current.snapshot?.messages[2].text).toBe('Newest content')
  expect(hook.result.current.snapshot?.contentRevision).toBe('10')
})
it.each([true, false])('recovers an %s initial/read failure only after explicit read retry', async initial => {
  if (!initial) watch.mockResolvedValueOnce(page(1, 2))
  watch.mockRejectedValueOnce(new Error('Read failed'))
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(hook.result.current.readError).toContain('Read failed'))
  expect(hook.result.current.snapshot?.messages.length ?? 0).toBe(initial ? 0 : 2)
  const calls = watch.mock.calls.length
  await act(async () => { await Promise.resolve() })
  expect(watch).toHaveBeenCalledTimes(calls)
  watch.mockResolvedValueOnce(page(1, 4, 2))
  act(() => hook.result.current.retryRead())
  await waitFor(() => expect(hook.result.current.snapshot?.revision).toBe(2))
  expect(hook.result.current.readError).toBeNull()
  expect(watch).toHaveBeenCalledWith('chat', -1)
})
it('loads bounded older pages, deduplicates split exchanges and retains old history after live updates', async () => {
  const update = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(102, 201)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(hook.result.current.snapshot?.messages).toHaveLength(100))
  watch.mockResolvedValueOnce(page(2, 101))
  await act(async () => { await hook.result.current.loadOlder() })
  expect(watch).toHaveBeenCalledWith('chat', -1, 102)
  expect(hook.result.current.snapshot?.messages).toHaveLength(200)
  // Advancing the live tail changes page boundaries; refresh back to the revealed floor.
  watch.mockImplementation((_id, _revision, before) => before === 104 ? Promise.resolve(page(4, 103, 2))
    : before === 4 ? Promise.resolve(page(1, 3, 2)) : new Promise(() => {}))
  await act(async () => { update.resolve(page(104, 203, 2)) })
  await waitFor(() => expect(hook.result.current.snapshot?.messages).toHaveLength(203))
  expect(hook.result.current.snapshot?.messages.map(item => item.sequence)).toEqual(Array.from({ length: 203 }, (_, i) => i + 1))
  expect(hook.result.current.snapshot?.hasOlder).toBe(false)
})
it('keeps a newer live reply and revision metadata when an older page resolves late', async () => {
  const update = deferred<ConversationSnapshot>(); const old = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(101, 200)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockImplementationOnce(() => old.promise).mockResolvedValueOnce(page(2, 101, 2))
  let loading!: Promise<void>
  act(() => { loading = hook.result.current.loadOlder() })
  await act(async () => { update.resolve(page(102, 201, 2)) })
  await waitFor(() => expect(hook.result.current.snapshot?.revision).toBe(2))
  // The page must also bridge message101 from the earlier tail. Native page remains <=100;
  // retaining the captured tail closes that seam even when its request was overtaken.
  await act(async () => { old.resolve(page(1, 100)); await loading })
  expect(hook.result.current.snapshot?.messages.at(-1)?.sequence).toBe(201)
  expect(hook.result.current.snapshot?.revision).toBe(2)
  expect(hook.result.current.snapshot?.messages.map(item => item.sequence)).toEqual(Array.from({ length: 201 }, (_, i) => i + 1))
})
it('rejects a page from another scope and ignores late old-scope loads', async () => {
  watch.mockResolvedValueOnce(page(101, 200))
  const hook = renderHook(({ id }) => useConversationSnapshot(id), { initialProps: { id: 'chat' } })
  await waitFor(() => expect(hook.result.current.snapshot).not.toBeNull())
  watch.mockResolvedValueOnce(page(1, 100, 1, 'wrong'))
  await act(async () => { await hook.result.current.loadOlder() })
  expect(hook.result.current.olderError).toContain('scope mismatch')
  const older = deferred<ConversationSnapshot>(); watch.mockImplementationOnce(() => older.promise)
  let loading!: Promise<void>; act(() => { loading = hook.result.current.loadOlder() })
  watch.mockResolvedValueOnce(page(1, 2, 3, 'other'))
  hook.rerender({ id: 'other' })
  await waitFor(() => expect(hook.result.current.snapshot?.conversationId).toBe('other'))
  await act(async () => { older.resolve(page(1, 100)); await loading })
  expect(hook.result.current.snapshot?.messages).toHaveLength(2)
  expect(hook.result.current.olderError).toBeNull()
})
it('merges by durable message identity and newest revision, independent of arrival', () => {
  const older = page(1, 4)
  const newer = page(3, 6, 2)
  newer.messages[0] = { ...newer.messages[0], replacedBy: 'replacement', text: 'Revised metadata' }
  const merged = mergeConversationPages([newer, older])
  expect(merged.messages).toHaveLength(6)
  expect(merged.messages[2]).toMatchObject({ replacedBy: 'replacement', text: 'Revised metadata' })
  expect(merged.hasOlder).toBe(false)
})

it('bridges multiple live pages while an older-page request remains in flight', async () => {
  const update = deferred<ConversationSnapshot>(); const old = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(101, 200)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockImplementationOnce(() => old.promise).mockImplementation((_id, _revision, before) => before === 401 ? Promise.resolve(page(301, 400, 2))
    : before === 301 ? Promise.resolve(page(201, 300, 2)) : before === 201 ? Promise.resolve(page(101, 200, 2)) : new Promise(() => {}))
  let loading!: Promise<void>
  act(() => { loading = hook.result.current.loadOlder() })
  await act(async () => { update.resolve(page(401, 500, 2)) })
  await waitFor(() => expect(hook.result.current.snapshot?.revision).toBe(2))
  await act(async () => { old.resolve(page(1, 100)); await loading })
  expect(hook.result.current.snapshot?.messages.map(item => item.sequence)).toEqual(Array.from({ length: 500 }, (_, i) => i + 1))
})

it('publishes successive live updates while a history refresh is still pending', async () => {
  const update = deferred<ConversationSnapshot>()
  const history = deferred<ConversationSnapshot>()
  const second = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(102, 201)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(hook.result.current.snapshot?.revision).toBe(1))
  watch.mockResolvedValueOnce(page(2, 101))
  await act(async () => { await hook.result.current.loadOlder() })
  watch.mockImplementation((_id, revision, before) => before === 104 ? history.promise
    : before === 4 ? Promise.resolve(page(1, 3, 2)) : revision === 2 ? second.promise : new Promise(() => {}))
  await act(async () => { update.resolve(page(104, 203, 2)) })
  await waitFor(() => expect(watch).toHaveBeenCalledWith('chat', -1, 104))
  expect(hook.result.current.snapshot?.revision).toBe(2)
  expect(hook.result.current.snapshot?.messages.at(-1)?.sequence).toBe(203)
  expect(hook.result.current.snapshot?.messages[0].sequence).toBe(2)
  await act(async () => { second.resolve(page(106, 205, 3)) })
  expect(hook.result.current.snapshot?.revision).toBe(3)
  expect(hook.result.current.snapshot?.messages.at(-1)?.sequence).toBe(205)
  await act(async () => { history.resolve(page(4, 103, 2)) })
  await waitFor(() => expect(hook.result.current.snapshot?.messages[0].sequence).toBe(1))
  expect(hook.result.current.snapshot?.revision).toBe(3)
  expect(hook.result.current.snapshot?.messages.at(-1)?.sequence).toBe(205)
})

it('updates old records without rolling back newer overlapping messages, turns or edit counts', async () => {
  const update = deferred<ConversationSnapshot>(), old = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(101, 200)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockImplementationOnce(() => old.promise)
  let loading!: Promise<void>
  act(() => { loading = hook.result.current.loadOlder() })
  const refreshed = page(3, 102, 2)
  refreshed.messages[47] = { ...refreshed.messages[47], replacedBy: 'replacement' }
  refreshed.turns = [{ id: 'shared', state: 'failed', replacedBy: null, operations: [] }] as unknown as TurnView[]
  refreshed.revisionSuffixCounts = [{ turnId: 'shared', exchangeCount: 1, coachTurnCount: 0 }]
  const fresh = page(103, 202, 3)
  fresh.turns = [{ id: 'shared', state: 'succeeded', replacedBy: 'replacement', operations: [] }] as unknown as TurnView[]
  fresh.revisionSuffixCounts = [{ turnId: 'shared', exchangeCount: 2, coachTurnCount: 0 }]
  watch.mockImplementation((_id, _revision, before) => before === 103 ? Promise.resolve(refreshed) : new Promise(() => {}))
  await act(async () => { update.resolve(fresh) })
  await waitFor(() => expect(hook.result.current.snapshot?.messages.find(m => m.sequence === 50)?.replacedBy).toBe('replacement'))
  await act(async () => { old.resolve(page(1, 100)); await loading })
  expect(hook.result.current.snapshot?.revision).toBe(3)
  expect(hook.result.current.snapshot?.messages.find(m => m.sequence === 50)?.replacedBy).toBe('replacement')
  expect(hook.result.current.snapshot?.turns[0]).toMatchObject({ state: 'succeeded', replacedBy: 'replacement' })
  expect(hook.result.current.snapshot?.revisionSuffixCounts[0].exchangeCount).toBe(2)
  expect(hook.result.current.snapshot?.messages).toHaveLength(202)
})

it('retains the published reply on history failure, then refreshes only after explicit retry', async () => {
  const update = deferred<ConversationSnapshot>(), history = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(101, 200)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(() => useConversationSnapshot('chat'))
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockResolvedValueOnce(page(1, 100))
  await act(async () => { await hook.result.current.loadOlder() })
  watch.mockImplementation((_id, _revision, before) => before === 103 ? history.promise : new Promise(() => {}))
  await act(async () => { update.resolve(page(103, 202, 2)) })
  expect(hook.result.current.snapshot?.messages.at(-1)?.sequence).toBe(202)
  await act(async () => { history.reject(new Error('History unavailable')) })
  expect(hook.result.current.readError).toContain('History unavailable')
  expect(hook.result.current.snapshot?.messages.at(-1)?.sequence).toBe(202)
  const calls = watch.mock.calls.length
  await act(async () => { await Promise.resolve() })
  expect(watch).toHaveBeenCalledTimes(calls)
  watch.mockImplementation((_id, revision, before) => before === 103 ? Promise.resolve(page(3, 102, 3))
    : before === 3 ? Promise.resolve(page(1, 2, 3)) : revision === -1 ? Promise.resolve(page(103, 202, 3)) : new Promise(() => {}))
  act(() => hook.result.current.retryRead())
  await waitFor(() => expect(hook.result.current.snapshot?.revision).toBe(3))
  expect(hook.result.current.readError).toBeNull()
  expect(hook.result.current.snapshot?.messages).toHaveLength(202)
})

it('ignores a delayed refresh after switching conversations', async () => {
  const update = deferred<ConversationSnapshot>(), history = deferred<ConversationSnapshot>()
  watch.mockResolvedValueOnce(page(101, 200)).mockImplementationOnce(() => update.promise)
  const hook = renderHook(({ id }) => useConversationSnapshot(id), { initialProps: { id: 'chat' } })
  await waitFor(() => expect(watch).toHaveBeenCalledTimes(2))
  watch.mockResolvedValueOnce(page(1, 100))
  await act(async () => { await hook.result.current.loadOlder() })
  watch.mockImplementation((_id, _revision, before) => before === 103 ? history.promise : new Promise(() => {}))
  await act(async () => { update.resolve(page(103, 202, 2)) })
  watch.mockResolvedValueOnce(page(1, 2, 4, 'other'))
  hook.rerender({ id: 'other' })
  await waitFor(() => expect(hook.result.current.snapshot?.conversationId).toBe('other'))
  await act(async () => { history.resolve(page(3, 102, 3)) })
  expect(hook.result.current.snapshot?.messages).toHaveLength(2)
  expect(hook.result.current.readError).toBeNull()
})
