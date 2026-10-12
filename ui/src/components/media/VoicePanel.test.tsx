// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { VoicePanel } from './VoicePanel'
import { I18nProvider } from '../localization/i18n'
const list = vi.hoisted(() => vi.fn())
const invoke = vi.hoisted(() => vi.fn())
vi.mock('../../platform/audio/microphones', () => ({ listMicrophones: list }))
vi.mock('../../platform/diagnostics/faults', () => ({ reportFault: vi.fn() }))
vi.mock('../../platform/ipc/native', () => ({ invoke }))
vi.mock('../../platform/audio/browser-recording', () => ({ startBrowserRecording: vi.fn() }))
beforeEach(() => {
  list.mockReset(); invoke.mockReset()
  invoke.mockImplementation(async (command: string) => command === 'microphone_test_start' ? { browserCapture: false, deviceLabel: 'USB input' } : command === 'microphone_test_samples' ? [0.2, -0.2] : undefined)
})

const panel = (props: Partial<Parameters<typeof VoicePanel>[0]> = {}) => <I18nProvider locale="english">
  <VoicePanel label="Message" phase="ready" face={null} mode="tap" onMode={() => {}}
    pad={{ label: 'Record audio', disabled: false, action: { kind: 'press', onPress: () => {} } }} {...props} /></I18nProvider>

it('keeps a permission-hidden mobile microphone neutral before and after a successful recording', async () => {
  list.mockResolvedValue({ source: 'browser', devices: [] })
  const { container, rerender } = render(panel({ device: null }))
  await waitFor(() => expect(list).toHaveBeenCalledTimes(1))
  expect(container.querySelector('.voice-lamp')).toHaveAttribute('data-tone', 'idle')
  list.mockResolvedValue({ source: 'browser', devices: [{ id: 'phone', label: 'Phone microphone', isDefault: false, unavailable: null }] })
  rerender(panel({ device: null, phase: 'recording', health: { signal: 'sound', level: 0.3, detected: true } }))
  await waitFor(() => expect(list).toHaveBeenCalledTimes(2))
  expect(container.querySelector('.voice-lamp')).toHaveAttribute('data-tone', 'live')
  list.mockResolvedValue({ source: 'browser', devices: [] })
  rerender(panel({ device: null }))
  await waitFor(() => expect(list).toHaveBeenCalledTimes(3))
  expect(container.querySelector('.voice-lamp')).toHaveAttribute('data-tone', 'idle')
  expect(screen.queryByText('Not connected')).not.toBeInTheDocument()
})

it('leads the control row with the microphone lamp, which opens Recording settings where the check lives', () => {
  const { container } = render(panel({ microphoneSelector: <select aria-label="Microphone" />, microphoneCheck: <p>Check row</p> }))
  const row = container.querySelector('.voice-controls')!
  const lamp = screen.getByRole('button', { name: 'Microphone' })
  expect(row.children[0]).toBe(screen.getByRole('button', { name: 'Recording settings' }))
  expect(row.children[1]).toContainElement(lamp)
  expect(screen.queryByText('Check row')).toBeNull()
  fireEvent.click(lamp)
  const dialog = screen.getByRole('dialog', { name: 'Recording settings' })
  expect(within(dialog).getByRole('combobox', { name: 'Microphone' })).toBeInTheDocument()
  expect(within(dialog).getByText('Check row')).toBeInTheDocument()
})

it('shows the saved device as not connected when it is missing from the list', async () => {
  list.mockResolvedValue({ source: 'native', devices: [] })
  render(panel({ device: 'usb' }))
  expect(await screen.findByRole('button', { name: /not connected/ })).toHaveTextContent('Not connected')
})

it('keeps the recorder free of device and signal prose while recording; the lamp carries them', () => {
  const { container } = render(panel({ phase: 'recording', deviceLabel: 'USB', health: { signal: 'sound', level: 0.5, detected: true } }))
  expect(container.querySelector('.voice-panel')).not.toHaveTextContent(/Using|Sound detected|Say something/)
  expect(screen.getByRole('button', { name: 'Sound detected' })).toHaveAttribute('title', 'USB · Sound detected')
  expect(screen.queryByRole('meter')).toBeNull()
})

it('bands the quiet warning across the stream while recording, and only then', () => {
  const { container, rerender } = render(panel({ phase: 'recording', health: { signal: 'quiet', level: 0.02, detected: true } }))
  const band = container.querySelector('.voice-face .voice-face-notice')
  expect(band).toHaveTextContent('No sound. Check mute or move closer.')
  expect(band).toHaveAttribute('aria-hidden', 'true')
  rerender(panel({ phase: 'recording', health: { signal: 'sound', level: 0.5, detected: true } }))
  expect(container.querySelector('.voice-face-notice')).toBeNull()
})

it('offers the check over the face after a silent recording and starts it in Recording settings', async () => {
  const { MicrophoneCheck } = await import('./MicrophoneCheck')
  const onDismiss = vi.fn()
  const { container } = render(panel({ silentTake: true, onDismissSilentTake: onDismiss, microphoneCheck: <MicrophoneCheck device={null} /> }))
  const card = container.querySelector('.voice-grid > .voice-face-card')!
  expect(card).toHaveTextContent('It sounds like nothing reached your microphone.')
  fireEvent.click(within(card as HTMLElement).getByRole('button', { name: 'Check microphone' }))
  expect(onDismiss).toHaveBeenCalledOnce()
  const dialog = screen.getByRole('dialog', { name: 'Recording settings' })
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('microphone_test_start', { testId: expect.any(String), device: null }))
  expect(within(dialog).getByRole('button', { name: 'Stop' })).toBeInTheDocument()
})

it('lets the learner dismiss the silent-recording card, and hides it while recording', () => {
  const onDismiss = vi.fn()
  const { container, rerender } = render(panel({ silentTake: true, onDismissSilentTake: onDismiss }))
  fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
  expect(onDismiss).toHaveBeenCalledOnce()
  rerender(panel({ silentTake: true, onDismissSilentTake: onDismiss, phase: 'recording' }))
  expect(container.querySelector('.voice-face-card')).toBeNull()
})
