import { beforeEach, expect, it, vi } from 'vitest'
import { replaySelectionAudio, speakSelection } from './reading-speech'
import { interruptSpeech, setPlaybackAllowed } from './speech'
const api=vi.hoisted(()=>({read:vi.fn(),play:vi.fn()}))
vi.mock('../ipc/reading',()=>({readSelection:api.read}))
vi.mock('./speech-player',()=>({playSpeechAudio:api.play}))
beforeEach(()=>{vi.clearAllMocks();setPlaybackAllowed(true)})

it('retains alignment and exact display context through fetched and cached playback', async () => {
  const alignment = { sourceText: 'go', original: { characters: ['go'], starts: [0], ends: [1] }, normalized: null }
  api.read.mockResolvedValue({ audioBase64: 'AA==', audioAlignment: alignment, receipt: {} })
  api.play.mockImplementation((_audio, finish) => ({ play: async () => finish(), stop: vi.fn() }))
  const observer = { followSource: { text: 'go go', start: 3 } }
  await speakSelection({ text: 'go', language: 'english', variety: null, explanation: 'english', explanationVariety: null, aid: 'speech' }, new AbortController().signal, vi.fn(), 1, 1, observer)
  expect(api.play.mock.calls[0][0]).toMatchObject({ alignment })
  expect(api.play.mock.calls[0][5]).toMatchObject({ ...observer, sourceText: 'go' })
  await replaySelectionAudio('AA==', new AbortController().signal, vi.fn(), 1, 1, observer, alignment)
  expect(api.play.mock.calls[1][0]).toMatchObject({ alignment })
  expect(api.read).toHaveBeenCalledOnce()
})
it('revokes a pending token request when another utterance starts and never plays late audio',async()=>{
  let complete!:(result:unknown)=>void
  api.read.mockImplementation(()=>new Promise(resolve=>{complete=resolve}))
  const result=speakSelection({text:'sí',language:'spanish',variety:null,explanation:'english',explanationVariety:null,aid:'speech'},new AbortController().signal,vi.fn(),1,1)
  const sourceSignal=api.read.mock.calls[0][1] as AbortSignal
  interruptSpeech()
  expect(sourceSignal.aborted).toBe(true)
  complete({audioBase64:'late',receipt:{}})
  await expect(result).rejects.toThrow()
  expect(api.play).not.toHaveBeenCalled()
})

it('starts playback while inspection is pending and interruption stops it', async () => {
  api.read.mockResolvedValue({ audioBase64: 'AA==', receipt: {} })
  const stop = vi.fn()
  api.play.mockImplementation(() => ({ play: async () => {}, stop }))
  let inspected!: () => void
  const onAudio = vi.fn(() => new Promise<void>(resolve => { inspected = resolve }))
  const pending = speakSelection({ text: 'sí', language: 'spanish', variety: null, explanation: 'english', explanationVariety: null, aid: 'speech' }, new AbortController().signal, vi.fn(), 1, 1, { onAudio })
  await vi.waitFor(() => expect(onAudio).toHaveBeenCalledOnce())
  expect(api.play).toHaveBeenCalledOnce()
  interruptSpeech()
  inspected()
  await expect(pending).rejects.toThrow()
  expect(stop).toHaveBeenCalledOnce()
})


it('reports inspection failure and stops playback without leaving it detached', async () => {
  api.read.mockResolvedValue({ audioBase64: 'AA==', receipt: {} })
  const stop = vi.fn()
  api.play.mockImplementation(() => ({ play: async () => {}, stop }))
  const failure = new Error('Inspection failed')
  await expect(speakSelection({ text: 'go', language: 'english', variety: null, explanation: 'english', explanationVariety: null, aid: 'speech' }, new AbortController().signal, vi.fn(), 1, 1, { onAudio: async () => { throw failure } })).rejects.toBe(failure)
  expect(api.play).toHaveBeenCalledOnce()
  expect(stop).toHaveBeenCalledOnce()
})

it('retains cancellation when playback finishes before inspection', async () => {
  api.read.mockResolvedValue({ audioBase64: 'AA==', receipt: {} })
  api.play.mockImplementation((_audio, finish) => ({ play: async () => finish(), stop: vi.fn() }))
  let inspected!: () => void
  const onAudio = vi.fn(() => new Promise<void>(resolve => { inspected = resolve }))
  const pending = speakSelection({ text: 'go', language: 'english', variety: null, explanation: 'english', explanationVariety: null, aid: 'speech' }, new AbortController().signal, vi.fn(), 1, 1, { onAudio })
  await vi.waitFor(() => expect(onAudio).toHaveBeenCalledOnce())
  interruptSpeech()
  inspected()
  await expect(pending).rejects.toThrow()
})

it('forwards cancellation and its reason without AbortSignal.any', async () => {
  const any = Object.getOwnPropertyDescriptor(AbortSignal, 'any')
  Object.defineProperty(AbortSignal, 'any', { configurable: true, value: undefined })
  try {
    let complete!: (value: unknown) => void
    api.read.mockImplementation(() => new Promise(resolve => { complete = resolve }))
    const controller = new AbortController()
    const pending = speakSelection({ text: 'go', language: 'english', variety: null, explanation: 'english', explanationVariety: null, aid: 'speech' }, controller.signal, vi.fn(), 1, 1)
    const lifetime = api.read.mock.calls[0][1] as AbortSignal
    const reason = new DOMException('Stopped', 'AbortError')
    controller.abort(reason)
    expect(lifetime.aborted).toBe(true)
    expect(lifetime.reason).toBe(reason)
    complete({ audioBase64: 'late', receipt: {} })
    await expect(pending).rejects.toBe(reason)
    expect(api.play).not.toHaveBeenCalled()
  } finally {
    if (any) Object.defineProperty(AbortSignal, 'any', any)
    else Reflect.deleteProperty(AbortSignal, 'any')
  }
})
