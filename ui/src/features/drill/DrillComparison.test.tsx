// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { I18nProvider } from '../../components/localization/i18n'
import type { AudioInspection } from '../../generated/contracts'
import { DrillComparison } from './DrillComparison'

function view(overrides: Partial<Parameters<typeof DrillComparison>[0]> = {}) {
  return <I18nProvider locale="english"><DrillComparison target={<p>Hola</p>} reference={null} referenceTime={0} onSeekReference={() => {}}
    onPlayReference={() => {}} playingReference={false} referenceNote="" attempt={null} attemptLabel={null} attemptFailure={null}
    onRetryAttempt={() => {}} attemptUnavailable={null} direction="ltr" onDirection={() => {}} timeScale="fit" onTimeScale={() => {}}
    holding={false} playingAttempt={false} onPlayAttempt={() => {}} {...overrides} /></I18nProvider>
}

it('draws every reference state inside the same frame', () => {
  const { container, rerender } = render(view())
  const frame = container.querySelector('.drill-plot-frame')
  expect(frame).toHaveAttribute('data-state', 'empty')
  expect(screen.getByText('Play the reference once to draw it here.')).toBeInTheDocument()
  expect(screen.getByRole('slider', { name: 'Seek reference audio' })).toBeDisabled()
  rerender(view({ playingReference: true }))
  expect(container.querySelector('.drill-plot-frame')).toBe(frame)
  expect(frame).toHaveAttribute('data-state', 'loading')
  expect(screen.getByText('Loading reference…')).toBeInTheDocument()
  rerender(view({ referenceFailure: <p role="alert">No audio</p> }))
  expect(container.querySelector('.drill-plot-frame')).toBe(frame)
  expect(frame).toHaveAttribute('data-state', 'failed')
  expect(frame).toContainElement(screen.getByRole('alert'))
})

it('keeps its frames with no card: the card area says so and nothing can be played', () => {
  const { container } = render(view({ target: null }))
  expect(container.querySelector('.drill-target-card')).toHaveTextContent('No card selected')
  expect(screen.getByRole('button', { name: 'Play reference' })).toBeDisabled()
  expect(screen.getByRole('button', { name: 'Play yours' })).toBeDisabled()
  const frames = [...container.querySelectorAll('.drill-plot-frame')]
  expect(frames).toHaveLength(2)
  // No instruction to play or record what does not exist.
  for (const frame of frames) {
    expect(frame).toHaveAttribute('data-state', 'empty')
    expect(frame.textContent).toBe('')
  }
})

it('keeps the take frame while its recording loads', () => {
  const { container } = render(view({ attemptLabel: 'Attempt 2' }))
  const frames = container.querySelectorAll('.drill-plot-frame')
  expect(frames).toHaveLength(2)
  expect(frames[0]).toHaveTextContent('Play the reference to compare it with this attempt.')
  expect(frames[1]).toHaveAttribute('data-state', 'loading')
})

const inspection: AudioInspection = {
  recordingId: 'recording-1', owner: { kind: 'drillItem', id: 'item-1' }, duration: 1, sampleRate: 16000,
  waveform: { binSeconds: 0.5, min: [-0.2], max: [0.2] },
  spectrogram: {
    frameSeconds: 0.5, frameStartSeconds: [0], windowSeconds: 0.5, fftSize: 512,
    bands: [{ lowHz: 50, centerHz: 100, highHz: 200 }], minFrequencyHz: 50, maxFrequencyHz: 8000,
    measuredMaxFrequencyHz: 8000, melScale: 'htk', normalization: 'unit-peak triangular filters',
    dbReference: '0 dB = full-scale power (1.0)', dbMin: -100, dbMax: 0, bins: [[-40]],
  },
  activity: { algorithm: 'fixture', noiseFloorDbfs: -60, thresholdDbfs: -40, regions: [], pauses: [], limitations: [] },
  wordTiming: { status: 'unavailable', reason: null, words: [], unsupported: [] },
}

it('disables alignment without word timing and shows no timing error', () => {
  const { container } = render(view({ reference: inspection, attempt: inspection, attemptLabel: 'Attempt 1', timeScale: 'words' }))
  expect(container.querySelectorAll('.drill-track .audio-spectrum-cursor')[1]).toHaveStyle({ left: '0%' })
  expect(screen.getByRole('radio', { name: 'Align words' })).toBeDisabled()
  expect(screen.getByRole('radio', { name: 'Fit' })).toHaveAttribute('aria-checked', 'true')
  expect(screen.queryByText(/Word timings unavailable/)).not.toBeInTheDocument()
})

it('aligns the word track, preserves playback, and resets the view when timing disappears', async () => {
  const timed = (duration: number, start: number, end: number): AudioInspection => ({ ...inspection, duration,
    wordTiming: { status: 'available', reason: null, unsupported: [], words: [
      { index: 0, word: 'Hola', start, end, providerStart: start, providerEnd: end, clipped: false },
    ] },
  })
  const reference = timed(2, 0.2, 1), attempt = timed(4, 1, 3)
  const onTimeScale = vi.fn(), onPlayAttempt = vi.fn()
  const props = { reference, attempt, attemptLabel: 'Attempt 1', onTimeScale, onPlayAttempt }
  const { container, rerender } = render(view(props))
  fireEvent.click(screen.getByRole('radio', { name: 'Align words' }))
  expect(onTimeScale).toHaveBeenCalledWith('words')
  rerender(view({ ...props, timeScale: 'words', attemptTime: 2 }))
  const cursor = container.querySelectorAll<HTMLElement>('.drill-track .audio-spectrum-cursor')[1]
  await waitFor(() => expect(parseFloat(cursor.style.left)).toBeCloseTo(40))
  await waitFor(() => expect(container.querySelectorAll('.timed-words[data-placement="overlay"]')[1].firstElementChild).toHaveStyle({ left: '10%' }))
  expect(container.querySelectorAll('.timed-word[data-outcome="same"]')).toHaveLength(2)
  expect(container.querySelector('.drill-word-slot')).toBeNull()
  expect(screen.getByText(/Word-aligned display; playback uses original timing/)).not.toBeVisible()
  fireEvent.click(screen.getAllByText('i')[0])
  expect(screen.getByText(/Word-aligned display; playback uses original timing/)).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: 'Play yours' }))
  expect(onPlayAttempt).toHaveBeenCalledOnce()
  rerender(view({ ...props, attempt: inspection, timeScale: 'words' }))
  expect(screen.getByRole('radio', { name: 'Align words' })).toBeDisabled()
  expect(screen.queryByText(/Word-aligned display; playback uses original timing/)).not.toBeInTheDocument()
})

it('leaves uncertain recognition orange and never warps its timing', () => {
  const timed = { ...inspection, wordTiming: { status: 'available' as const, reason: null, unsupported: [],
    words: [{ index: 0, word: 'Hola', start: 0.1, end: 0.8, providerStart: 0.1, providerEnd: 0.8, clipped: false }] } }
  const { container } = render(view({ reference: timed, attempt: timed, attemptLabel: 'Attempt 1', comparisonAccepted: false, timeScale: 'words' }))
  expect(screen.getByRole('radio', { name: 'Align words' })).toBeDisabled()
  expect(container.querySelectorAll('.timed-word[data-outcome="unknown"]')).toHaveLength(2)
})


it('opens narrow comparison controls in a native modal above the scrub surfaces', () => {
  const media = vi.spyOn(window, 'matchMedia').mockReturnValue({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaQueryList)
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open') }
  const modal = vi.spyOn(HTMLDialogElement.prototype, 'showModal')
  const onTimeScale = vi.fn(), onSeekReference = vi.fn(), onSeekAttempt = vi.fn()
  const rendered = render(view({ reference: inspection, attempt: inspection, onTimeScale, onSeekReference, onSeekAttempt }))
  fireEvent.click(screen.getByRole('button', { name: 'Comparison settings' }))
  const dialog = screen.getByRole('dialog', { name: 'Comparison settings' })
  expect(modal).toHaveBeenCalled()
  expect(dialog).toHaveAttribute('open')
  fireEvent.click(screen.getByRole('radio', { name: 'Same scale' }))
  expect(onTimeScale).toHaveBeenCalledWith('shared')
  expect(onSeekReference).not.toHaveBeenCalled()
  expect(onSeekAttempt).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Close Comparison settings' }))
  expect(screen.queryByRole('dialog', { name: 'Comparison settings' })).toBeNull()
  rendered.unmount()
  media.mockRestore()
  modal.mockRestore()
})


it('plays available audio before inspection and keeps that control usable if inspection fails', () => {
  const play = vi.fn()
  const { rerender } = render(view({ attemptAudioReady: true, attemptLabel: 'Attempt 1', onPlayAttempt: play }))
  fireEvent.click(screen.getByRole('button', { name: 'Play yours' }))
  expect(play).toHaveBeenCalledTimes(1)
  rerender(view({ attemptAudioReady: true, attemptLabel: 'Attempt 1', attemptFailure: new Error('Inspection failed'), onPlayAttempt: play }))
  expect(screen.getByRole('button', { name: 'Play yours' })).toBeEnabled()
  rerender(view({ attemptAudioReady: true, holding: true }))
  expect(screen.getByRole('button', { name: 'Play yours' })).toBeDisabled()
})
