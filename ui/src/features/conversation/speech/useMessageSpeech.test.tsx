// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import type { ConversationSnapshot } from '../../../generated/contracts'
import { setPlaybackAllowed } from '../../../platform/audio/speech'
import { useMessageSpeech } from './useMessageSpeech'
const native = vi.hoisted(() => ({ invoke: vi.fn(), execute: vi.fn(), fault: vi.fn(), play: vi.fn(), stop: vi.fn(), player: vi.fn(), seek: vi.fn(), rate: vi.fn(), volume: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }))
vi.mock('../../../platform/ipc/workspace', () => ({ executeAction: native.execute, nativeError: String }))
vi.mock('../../../platform/diagnostics/faults', () => ({ reportFault: native.fault }))
vi.mock('../../../platform/audio/speech-player', () => ({ playSpeechAudio: native.player }))
function snapshot(ids: string[], operation = true): ConversationSnapshot {
  return { conversationId: 'chat', sessionId: 'session', revision: ids.length, messages: ids.map((id, i) => ({ id, sequence: i, role: 'assistant', text: id })), turns: operation ? ids.map(id => ({ speech: { id: `speech-${id}`, state: 'running', sourceMessageId: id } })) : [] } as unknown as ConversationSnapshot
}
beforeEach(() => {
  setPlaybackAllowed(true)
  vi.clearAllMocks()
  native.play.mockResolvedValue(undefined)
  native.player.mockImplementation((_audio, _finish, _error, _rate, _volume, observer) => {
    const handle = { play: native.play, stop: native.stop, seek: native.seek, setRate: native.rate, setVolume: native.volume }
    observer?.onReady?.(handle)
    return handle
  })
  native.execute.mockResolvedValue({ entityId: 'manual' })
  native.invoke.mockImplementation(async (_command, { operationId }) => ({ status: 'ready', operationId, messageId: operationId === 'manual' ? 'old' : operationId.slice(7), attemptId: operationId, mime: 'audio/mpeg', audioBase64: '' }))
})

it('distinguishes requested and ready audio from actual playback, including buffering and stop', async () => {
  let ready!: (value: unknown) => void
  native.invoke.mockImplementationOnce(() => new Promise(resolve => { ready = resolve }))
  const view = renderHook(() => useMessageSpeech(snapshot(['old']), 'chat', true, true))
  expect(view.result.current.phase).toBe('idle')
  act(() => view.result.current.toggle('old'))
  expect(view.result.current.phase).toBe('preparing')
  await waitFor(() => expect(native.invoke).toHaveBeenCalledOnce())
  await act(async () => ready({ status: 'ready', operationId: 'manual', messageId: 'old', attemptId: 'attempt', mime: 'audio/mpeg', audioBase64: '' }))
  expect(native.play).toHaveBeenCalledOnce()
  expect(view.result.current.phase).toBe('preparing')
  const observer = native.player.mock.calls[0][5]
  act(() => observer.onPlaying(true))
  expect(view.result.current.phase).toBe('playing')
  act(() => observer.onPlaying(false))
  expect(view.result.current.phase).toBe('preparing')
  act(() => observer.onPlaying(true))
  act(() => view.result.current.stop())
  expect(view.result.current.phase).toBe('idle')
  act(() => observer.onPlaying(true))
  expect(view.result.current.phase).toBe('idle')
  expect(view.result.current.messageId).toBeNull()
})

it('clears preparation on playback rejection while retaining failure details', async () => {
  native.play.mockRejectedValueOnce(new Error('Playback denied'))
  const view = renderHook(() => useMessageSpeech(snapshot(['old']), 'chat', true, true))
  act(() => view.result.current.toggle('old'))
  await waitFor(() => expect(view.result.current.failure?.text).toContain('Playback denied'))
  expect(view.result.current.phase).toBe('idle')
  expect(view.result.current.failure?.details).toBeDefined()
})

it('keeps inspection on the same audio and clock, seeks it, and resumes without a generation command', async () => {
  const view = renderHook(({ rate }) => useMessageSpeech(snapshot(['old']), 'chat', true, true, rate), { initialProps: { rate: 1 } })
  act(() => view.result.current.toggle('old'))
  await waitFor(() => expect(native.play).toHaveBeenCalledOnce())
  expect(view.result.current.retained?.audio.operationId).toBe('manual')
  const observer = native.player.mock.calls[0][5]
  act(() => observer.onTime(1, 4))
  expect(view.result.current.time).toBe(1)
  act(() => view.result.current.seek('old', 2))
  expect(native.seek).toHaveBeenCalledWith(2)
  view.rerender({ rate: 1.2 })
  expect(native.rate).toHaveBeenCalledWith(1.2)
  act(() => view.result.current.stop())
  act(() => view.result.current.resume('old'))
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(2))
  expect(native.player.mock.calls[1][5].startSeconds).toBe(2)
  expect(native.execute).toHaveBeenCalledOnce()
  view.unmount()
})
it('does not request or play history on mount, preference changes or reopen', () => {
  const old = snapshot(['old'])
  const view = renderHook(({ enabled, active }) => useMessageSpeech(old, 'chat', enabled, active), { initialProps: { enabled: true, active: true } })
  view.rerender({ enabled: false, active: true }); view.rerender({ enabled: true, active: true })
  view.rerender({ enabled: true, active: false }); view.rerender({ enabled: true, active: true })
  expect(native.invoke).not.toHaveBeenCalled(); expect(native.execute).not.toHaveBeenCalled(); expect(native.play).not.toHaveBeenCalled()
})
it('reads and plays a newly arriving speech operation once, without generating it', async () => {
  const view = renderHook(({ state }) => useMessageSpeech(state, 'chat', true, true), { initialProps: { state: snapshot(['old']) } })
  view.rerender({ state: snapshot(['old', 'new']) })
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(1))
  view.rerender({ state: snapshot(['old', 'new']) })
  expect(native.invoke).toHaveBeenCalledTimes(1)
  expect(native.execute).not.toHaveBeenCalled()
  act(() => view.result.current.stop())
  expect(native.stop).toHaveBeenCalledTimes(1)
})
it('manual replay requests the selected source and duplicate clicks stop pending work', async () => {
  let finish!: (value: unknown) => void
  native.execute.mockImplementation((_state, action) => action.kind === 'requestMessageSpeech' ? new Promise(resolve => { finish = resolve }) : Promise.resolve({}))
  const view = renderHook(() => useMessageSpeech(snapshot(['old']), 'chat', true, true))
  act(() => view.result.current.toggle('old'))
  expect(view.result.current.phase).toBe('preparing')
  act(() => view.result.current.toggle('old'))
  expect(view.result.current.phase).toBe('idle')
  await act(async () => finish({ entityId: 'manual' }))
  expect(native.execute).toHaveBeenCalledWith(expect.anything(), { kind: 'requestMessageSpeech', messageId: 'old' })
  expect(native.execute).toHaveBeenCalledWith(expect.anything(), { kind: 'cancelMessageSpeech', operationId: 'manual' })
  expect(native.invoke).not.toHaveBeenCalled()
})
it('suppresses late audio and cancels pending speech when leaving the conversation', async () => {
  let finish!: (value: unknown) => void
  native.invoke.mockImplementation(() => new Promise(resolve => { finish = resolve }))
  const view = renderHook(({ state }) => useMessageSpeech(state, state.conversationId, true, true), { initialProps: { state: snapshot(['old']) } })
  view.rerender({ state: snapshot(['old', 'new']) })
  await waitFor(() => expect(native.invoke).toHaveBeenCalledTimes(1))
  view.rerender({ state: { ...snapshot(['elsewhere']), conversationId: 'other' } })
  await act(async () => finish({ status: 'ready', operationId: 'speech-new', messageId: 'new' }))
  expect(native.execute).toHaveBeenCalledWith({ sessionId: 'session' }, { kind: 'cancelMessageSpeech', operationId: 'speech-new' })
  expect(native.play).not.toHaveBeenCalled()
})
it('surfaces playback rejection without falling back or regenerating', async () => {
  native.play.mockRejectedValue(new Error('Playback denied'))
  const view = renderHook(() => useMessageSpeech(snapshot(['old']), 'chat', false, true))
  act(() => view.result.current.toggle('old'))
  await waitFor(() => expect(view.result.current.failure?.text).toContain('Playback denied'))
  expect(native.stop).toHaveBeenCalledTimes(1)
  expect(native.execute).toHaveBeenCalledTimes(1)
})

it('waits for source binding on a pre-created speech operation', async () => {
  const first = snapshot(['old'])
  first.turns.push({ speech: null } as never)
  const view = renderHook(({ state }) => useMessageSpeech(state, 'chat', true, true), { initialProps: { state: first } })
  view.rerender({ state: snapshot(['old', 'new']) })
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(1))
  expect(native.execute).not.toHaveBeenCalled()
})
it('stops audio when automatic playback is suspended for microphone input', async () => {
  const view = renderHook(({ state, enabled }) => useMessageSpeech(state, 'chat', enabled, true), { initialProps: { state: snapshot(['old']), enabled: true } })
  view.rerender({ state: snapshot(['old', 'new']), enabled: true })
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(1))
  view.rerender({ state: snapshot(['old', 'new']), enabled: false })
  expect(native.stop).toHaveBeenCalledTimes(1)
  view.rerender({ state: snapshot(['old', 'new']), enabled: true })
  expect(native.play).toHaveBeenCalledTimes(1)
})

it('manual replay consumes the native operation returned for the selected message', async () => {
  native.execute.mockResolvedValue({ entityId: 'speech-old' })
  const view = renderHook(() => useMessageSpeech(snapshot(['old']), 'chat', true, true))
  act(() => view.result.current.toggle('old'))
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(1))
  expect(native.invoke).toHaveBeenCalledWith('read_speech_audio', { sessionId: 'session', operationId: 'speech-old' })
  act(() => view.result.current.stop())
  act(() => view.result.current.toggle('old'))
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(2))
  expect(native.execute).toHaveBeenCalledTimes(2)
  expect(native.invoke).toHaveBeenCalledTimes(2)
})

it.each([false, true])('suppresses a delayed audio result after suspension (resumed: %s)', async (resume) => {
  let finish!: (value: unknown) => void
  native.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const view = renderHook(({ state }) => useMessageSpeech(state, 'chat', true, true), { initialProps: { state: snapshot(['old']) } })
  view.rerender({ state: snapshot(['old', 'new']) })
  await waitFor(() => expect(native.invoke).toHaveBeenCalledOnce())
  act(() => { setPlaybackAllowed(false); if (resume) setPlaybackAllowed(true) })
  await act(async () => finish({ status: 'ready', operationId: 'speech-new', messageId: 'new' }))
  expect(native.play).not.toHaveBeenCalled()
  expect(view.result.current.messageId).toBeNull()
  expect(native.execute).toHaveBeenCalledWith({ sessionId: 'session' }, { kind: 'cancelMessageSpeech', operationId: 'speech-new' })
})

it('stops a superseded reply and never autoplays a retained version', async () => {
  const view = renderHook(({ state }) => useMessageSpeech(state, 'chat', true, true), { initialProps: { state: snapshot(['old']) } })
  view.rerender({ state: snapshot(['old', 'new']) })
  await waitFor(() => expect(native.play).toHaveBeenCalledTimes(1))
  const revised = snapshot(['old', 'new'])
  revised.messages[1].replacedBy = 'replacement'
  revised.turns[1].replacedBy = 'replacement'
  view.rerender({ state: revised })
  expect(native.stop).toHaveBeenCalledTimes(1)
  expect(view.result.current.messageId).toBeNull()
  expect(native.play).toHaveBeenCalledTimes(1)
})

it('preserves the speech failure explanation and request metadata beside the message', async () => {
  native.invoke.mockResolvedValue({ status: 'unavailable', operationId: 'manual', messageId: 'old', reason: 'failed', message: 'Speech model is unavailable', attemptId: 'attempt-7', diagnostics: { request_id: 'req-7', requested_model: 'speech-model', response: { error: { message: 'Audio output is unsupported' } } } })
  const view = renderHook(() => useMessageSpeech(snapshot(['old']), 'chat', false, true))
  act(() => view.result.current.toggle('old'))
  await waitFor(() => expect(view.result.current.failure?.text).toContain('Audio output is unsupported'))
  expect(view.result.current.failure?.details).toMatchObject({ metadata: { request_id: 'req-7', requested_model: 'speech-model', attemptId: 'attempt-7', operationId: 'manual' } })
  expect(native.execute).toHaveBeenCalledTimes(1)
  expect(native.play).not.toHaveBeenCalled()
})


it('autoplays the native snapshot speech operation without requesting generation', async () => {
  const operationId = 'graph:["engine","run","audio"]'
  const view = renderHook(({ state }) => useMessageSpeech(state, 'chat', true, true), { initialProps: { state: snapshot(['old']) } })
  const next = snapshot(['old', 'new'])
  next.turns[1].nativeExecutionAvailable = true
  next.turns[1].speech!.id = operationId
  native.invoke.mockResolvedValue({ status: 'ready', operationId, messageId: 'new', attemptId: 'native', mime: 'audio/mpeg', audioBase64: '' })
  view.rerender({ state: next })
  await waitFor(() => expect(native.play).toHaveBeenCalledOnce())
  view.rerender({ state: next })
  expect(native.execute).not.toHaveBeenCalled()
  expect(native.invoke).toHaveBeenCalledOnce()
  view.unmount()
})

it('plays a new reply after transcription releases playback without replaying old messages', async () => {
  const view = renderHook(({ state, suspended }) => useMessageSpeech(state, 'chat', true, true, 1, 1, suspended),
    { initialProps: { state: snapshot(['old']), suspended: false } })
  view.rerender({ state: snapshot(['old']), suspended: true })
  view.rerender({ state: snapshot(['old', 'new']), suspended: true })
  expect(native.invoke).not.toHaveBeenCalled()
  view.rerender({ state: snapshot(['old', 'new']), suspended: false })
  await waitFor(() => expect(native.play).toHaveBeenCalledOnce())
  expect(native.invoke).toHaveBeenCalledWith('read_speech_audio', { sessionId: 'session', operationId: 'speech-new' })
  expect(native.execute).not.toHaveBeenCalled()
  view.unmount()
})
