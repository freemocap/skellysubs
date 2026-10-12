// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('../ipc/tauri', () => ({ isTauri: true }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
import { clearLogs, getLogs, logDiagnostic, installDiagnosticCapture, logInfo, diagnosticDeliveryState } from './log'
import { reportUnhandledError, useFaultStore } from './faults'
import { mediaError } from '../audio/media-error'
let dispose: (() => void) | undefined
beforeEach(() => { invoke.mockReset(); invoke.mockResolvedValue({}); clearLogs() })
afterEach(() => { dispose?.(); dispose = undefined; vi.restoreAllMocks() })
it('delivers browser audio explanations and causes through the diagnostic bridge', async () => {
  const error = mediaError(new Error('Failed to start the audio device', { cause: new Error('Missing autoaudiosink; token=PRIVATE') }), 'Creating reward audio context')
  await logDiagnostic('Enabling reward sounds', error)
  const saved = JSON.stringify(invoke.mock.calls[0][1])
  expect(saved).toContain('Failed to start the audio device')
  expect(saved).toContain('Missing autoaudiosink')
  expect(saved).not.toContain('PRIVATE')
})
it('persists precise safe causes and vetted commands without private bodies', async () => {
  await logDiagnostic('Microphone', { code: 'validation', message: 'This custom endpoint is configured for chat only. Enable transcription and set its model in AI access settings.' }, 7, 'native_command_failed', 'error', { command: 'mic_start' })
  expect(invoke.mock.calls[0][1].event).toMatchObject({ context: 'microphone', nativeCode: 'validation', command: 'mic_start', cause: 'custom_transcription_unconfigured', faultId: 7 })
  await logDiagnostic('PRIVATE_CONTEXT', { code: 'MODEL_UNAVAILABLE', message: 'Rendering failed; transcript=PRIVATE_TRANSCRIPT' }, undefined, 'ui_fault', 'error', { command: 'PRIVATE_COMMAND' })
  expect(JSON.stringify(invoke.mock.calls)).not.toContain('PRIVATE')
  expect(JSON.stringify(getLogs())).not.toContain('PRIVATE')
})
it('captures each console/helper occurrence once, redacts cyclic bodies and installs once', async () => {
  dispose = installDiagnosticCapture()
  expect(installDiagnosticCapture()).toBe(dispose)
  const cyclic: { secret: string; self?: unknown } = { secret: 'PRIVATE_TOKEN' }; cyclic.self = cyclic
  console.error('Rendering failed; transcript=PRIVATE_TRANSCRIPT', cyclic)
  console.info('PRIVATE_URL')
  logInfo('[settings] autosaving')
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(3))
  expect(invoke.mock.calls.map(call => call[1].event.redactedArgs)).toEqual([2, 1, 1])
  expect(invoke.mock.calls[2][1].event.eventName).toBe('settings_saving')
  expect(JSON.stringify(invoke.mock.calls)).not.toContain('PRIVATE')
})
it('captures window errors and rejections with useful scrubbed messages and stack locations', async () => {
  dispose = installDiagnosticCapture()
  window.dispatchEvent(new ErrorEvent('error', { error: new TypeError('Cannot render picker; token=PRIVATE_STACK') }))
  const rejection = new Event('unhandledrejection')
  Object.defineProperty(rejection, 'reason', { value: new Error('Selection failed; prompt=PRIVATE_REASON') })
  window.dispatchEvent(rejection)
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(2))
  expect(invoke.mock.calls[0][1].event.cause).toBe('type_error')
  expect(invoke.mock.calls[0][1].event.diagnostics.message).toContain('Cannot render picker')
  expect(invoke.mock.calls[0][1].event.diagnostics.stack).toBeTruthy()
  expect(invoke.mock.calls[1][1].event.code).toBe('unhandled_rejection')
  expect(JSON.stringify(invoke.mock.calls)).not.toContain('PRIVATE')
})
it('names resize-observer and resource-load events and surfaces everything but the resize notice', async () => {
  dispose = installDiagnosticCapture()
  const surfaced = vi.fn(); window.addEventListener('unhandled-ui-error', surfaced)
  window.dispatchEvent(new ErrorEvent('error', { message: 'ResizeObserver loop completed with undelivered notifications.' }))
  const image = document.createElement('img'); document.body.append(image); image.dispatchEvent(new Event('error'))
  window.dispatchEvent(new ErrorEvent('error', { error: new TypeError('Cannot render picker; token=PRIVATE_STACK') }))
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(3))
  expect(invoke.mock.calls.map(call => call[1].event.cause)).toEqual(['resize_observer_loop', 'resource_load_failed', 'type_error'])
  expect(surfaced).toHaveBeenCalledTimes(2)
  expect(surfaced.mock.calls[0][0].detail).toBe('A img element failed to load its resource.')
  expect(JSON.stringify(invoke.mock.calls)).not.toContain('PRIVATE')
  window.removeEventListener('unhandled-ui-error', surfaced); image.remove()
})
it('shows a repeating unhandled error once on the fault bar', () => {
  for (let i = 0; i < 5; i++) reportUnhandledError(new CustomEvent('unhandled-ui-error', { detail: new Error('Layout failed') }))
  expect(useFaultStore.getState().faults).toEqual([expect.objectContaining({ id: expect.any(Number), context: 'Unexpected error', message: 'Layout failed' })])
})
it('makes failed delivery explicit without forwarding its own failure recursively', async () => {
  await logDiagnostic('application', null)
  invoke.mockClear(); invoke.mockRejectedValue(new Error('Bridge refused delivery; token=PRIVATE_TRANSPORT_DETAILS'))
  const visible = vi.fn(); window.addEventListener('diagnostic-bridge-failed', visible)
  const error = vi.spyOn(console, 'error').mockImplementation(() => {})
  dispose = installDiagnosticCapture()
  expect(await logDiagnostic('application', null)).toBe(false)
  expect(await logDiagnostic('application', null)).toBe(false)
  expect(visible).toHaveBeenCalledOnce()
  expect(invoke).toHaveBeenCalledTimes(2)
  expect(error).toHaveBeenCalledTimes(2)
  expect(diagnosticDeliveryState().failed).toBeGreaterThanOrEqual(2)
  expect(JSON.stringify(getLogs())).toContain('Bridge refused delivery')
  expect(JSON.stringify(getLogs())).not.toContain('PRIVATE')
  window.removeEventListener('diagnostic-bridge-failed', visible)
})

it('retains browser filename and line when ErrorEvent has no Error object', async () => {
  dispose = installDiagnosticCapture()
  window.dispatchEvent(new ErrorEvent('error', {
    message: 'Language picker failed', filename: 'http://localhost:1420/src/LanguagePickers.tsx?token=secret', lineno: 51, colno: 9,
  }))
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
  expect(invoke.mock.calls[0][1].event.diagnostics).toMatchObject({
    message: 'Language picker failed', stack: 'at browser (src/LanguagePickers.tsx:51:9)',
  })
  expect(JSON.stringify(invoke.mock.calls).replaceAll('[secret redacted]', '')).not.toContain('secret')
})

it('retains playback lifecycle stages without source text or audio', async () => {
  for (const eventName of ['speech_requested', 'speech_suppressed', 'speech_playing', 'speech_finished'] as const) {
    await logDiagnostic('speech', null, undefined, 'ui_event', 'info', { eventName })
  }
  expect(invoke.mock.calls.map(call => call[1].event.eventName)).toEqual(['speech_requested', 'speech_suppressed', 'speech_playing', 'speech_finished'])
  for (const [, { event }] of invoke.mock.calls) {
    expect(event.context).toBe('speech')
    expect(event.redactedArgs).toBe(0)
    expect(event.command).toBeNull()
  }
})
