// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { OriginalContent } from './OriginalContent'
const api = vi.hoisted(() => ({ listOriginalContent: vi.fn(), readOriginalContent: vi.fn(), replay: vi.fn() }))
vi.mock('../../platform/ipc/original-content', () => api)
vi.mock('../../platform/audio/reading-speech', () => ({ replaySelectionAudio: api.replay }))
const record = { id: 'original', kind: 'text.response', mediaType: 'application/json', state: 'complete', availability: 'retained', byteCount: 30, digest: 'digest', credentialsRemoved: false }
beforeEach(() => vi.resetAllMocks())
it('shows exact original text and current retention independently of a historical graph cut', async () => {
  api.listOriginalContent.mockResolvedValue([record])
  const text = ' {"text":"café 𠮷"}\n'
  api.readOriginalContent.mockResolvedValue({ record, text, base64: null })
  const { container } = render(<OriginalContent playback={{ rate: 1, volume: 0 }} source={{ engine: 'engine', execution: '42' }} />)
  await screen.findByText('Original content · current retention')
  await screen.findByText(/café/)
  expect(container.querySelector('pre')?.textContent).toBe(text)
  expect(api.listOriginalContent).toHaveBeenCalledWith({ engine: 'engine', execution: '42' })
})
it('does not show another source after an out-of-order response', async () => {
  let finish!: (value: unknown[]) => void
  api.listOriginalContent.mockReturnValueOnce(new Promise(resolve => { finish = resolve })).mockResolvedValueOnce([])
  const view = render(<OriginalContent playback={{ rate: 1, volume: 0 }} source={{ recording: 'old' }} />)
  view.rerender(<OriginalContent playback={{ rate: 1, volume: 0 }} source={{ recording: 'new' }} />)
  await screen.findByText('No original transport content was retained for this source.')
  await act(async () => finish([record]))
  expect(api.readOriginalContent).not.toHaveBeenCalled()
})
it('keeps evicted receipt metadata without fabricating a body', async () => {
  const evicted = { ...record, availability: 'evicted' }
  api.listOriginalContent.mockResolvedValue([evicted])
  api.readOriginalContent.mockResolvedValue({ record: evicted, text: null, base64: null })
  const { container } = render(<OriginalContent playback={{ rate: 1, volume: 0 }} source={{ engine: 'engine', execution: '42' }} />)
  await screen.findByText('digest', { selector: 'p' })
  expect(container.querySelector('pre')).toBeNull()
})

it('respects muted playback preferences for original recordings', async () => {
  const audio = { ...record, kind: 'recording.audio', mediaType: 'audio/wav' }
  api.listOriginalContent.mockResolvedValue([audio])
  api.readOriginalContent.mockResolvedValue({ record: audio, text: null, base64: 'V0FW' })
  api.replay.mockResolvedValue(undefined)
  render(<OriginalContent source={{ recording: 'source' }} playback={{ rate: 0.8, volume: 0 }} />)
  fireEvent.click(await screen.findByRole('button', { name: 'Play' }))
  expect(api.replay).toHaveBeenCalledWith('V0FW', expect.any(AbortSignal), expect.any(Function), 0.8, 0)
})
