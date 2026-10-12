// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { presenceOf, useMicrophonePresence } from './useMicrophonePresence'
const list = vi.hoisted(() => vi.fn())
const reportFault = vi.hoisted(() => vi.fn())
vi.mock('./microphones', () => ({ listMicrophones: list }))
vi.mock('../diagnostics/faults', () => ({ reportFault }))

const usb = { id: 'usb', label: 'USB headset', isDefault: true, channels: 1, sampleRate: 48000, unavailable: null }
const builtIn = { id: 'built-in', label: 'Built-in', isDefault: false, channels: 2, sampleRate: 44100, unavailable: null }
let devices: EventTarget
beforeEach(() => {
  list.mockReset(); reportFault.mockReset()
  devices = new EventTarget()
  Object.defineProperty(navigator, 'mediaDevices', { value: devices, configurable: true })
})

it('tracks nothing until an owner names a device choice', () => {
  const { result } = renderHook(() => useMicrophonePresence(undefined))
  expect(result.current).toBeNull()
  expect(list).not.toHaveBeenCalled()
})

it('finds the saved device and names it, without asking for microphone access', async () => {
  list.mockResolvedValue({ source: 'native', devices: [builtIn, usb] })
  const { result } = renderHook(() => useMicrophonePresence('usb'))
  await act(async () => {})
  expect(result.current).toEqual({ state: 'connected', label: 'USB headset', detail: null })
  expect(list).toHaveBeenCalledExactlyOnceWith(false)
})

it('names the system default for the default choice, and reports no input at all as missing', async () => {
  list.mockResolvedValueOnce({ source: 'native', devices: [builtIn, usb] })
  const { result } = renderHook(() => useMicrophonePresence(null))
  await act(async () => {})
  expect(result.current).toEqual({ state: 'connected', label: 'USB headset', detail: null })
  list.mockResolvedValueOnce({ source: 'native', devices: [] })
  await act(async () => { devices.dispatchEvent(new Event('devicechange')) })
  expect(result.current).toEqual({ state: 'missing', label: null, detail: null })
})

it('keeps the default choice present on a browser list that names no default', async () => {
  list.mockResolvedValue({ source: 'browser', devices: [{ ...builtIn, isDefault: false, channels: null, sampleRate: null }] })
  const { result } = renderHook(() => useMicrophonePresence(null))
  await act(async () => {})
  expect(result.current).toEqual({ state: 'connected', label: null, detail: null })
})

it('reports an unplugged saved device as missing and finds it again when devices change', async () => {
  list.mockResolvedValueOnce({ source: 'native', devices: [builtIn] })
  const { result, unmount } = renderHook(() => useMicrophonePresence('usb'))
  await act(async () => {})
  expect(result.current).toEqual({ state: 'missing', label: null, detail: null })
  list.mockResolvedValueOnce({ source: 'native', devices: [builtIn, usb] })
  await act(async () => { devices.dispatchEvent(new Event('devicechange')) })
  expect(result.current?.state).toBe('connected')
  unmount()
  devices.dispatchEvent(new Event('devicechange'))
  expect(list).toHaveBeenCalledTimes(2)
})

it('relists when the saved choice changes', async () => {
  list.mockResolvedValue({ source: 'native', devices: [builtIn, usb] })
  const { result, rerender } = renderHook(({ device }) => useMicrophonePresence(device), { initialProps: { device: 'usb' as string | null } })
  await act(async () => {})
  expect(result.current?.label).toBe('USB headset')
  await act(async () => { rerender({ device: 'built-in' }) })
  expect(result.current?.label).toBe('Built-in')
  expect(list).toHaveBeenCalledTimes(2)
})

it.each([null, 'usb'])('reports an unavailable microphone for choice %s even when another input works', async device => {
  list.mockResolvedValue({ source: 'native', devices: [builtIn, { ...usb, unavailable: 'The device is in use.' }] })
  const { result } = renderHook(() => useMicrophonePresence(device))
  await act(async () => {})
  expect(result.current).toEqual({ state: 'missing', label: 'USB headset', detail: 'The device is in use.' })
})

it('updates default-device availability and clears its old error after recovery', async () => {
  list.mockResolvedValueOnce({ source: 'native', devices: [{ ...usb, unavailable: 'Cannot open device' }, builtIn] })
  const { result } = renderHook(() => useMicrophonePresence(null))
  await act(async () => {})
  expect(result.current?.state).toBe('missing')
  list.mockResolvedValueOnce({ source: 'native', devices: [usb, builtIn] })
  await act(async () => { devices.dispatchEvent(new Event('devicechange')) })
  expect(result.current).toEqual({ state: 'connected', label: 'USB headset', detail: null })
})

it('reports a failed listing as unknown, with the reason, and as a fault', async () => {
  list.mockRejectedValue(new Error('The system would not list microphones'))
  const { result } = renderHook(() => useMicrophonePresence('usb'))
  await act(async () => {})
  expect(result.current).toEqual({ state: 'unknown', label: null, detail: 'The system would not list microphones' })
  expect(reportFault).toHaveBeenCalledExactlyOnceWith('Listing microphones', expect.any(Error))
})

it.each([null, 'saved-mic'])('does not infer disconnection from an empty browser inventory for %s', device => {
  expect(presenceOf({ source: 'browser', devices: [] }, device)).toEqual({ state: 'unknown', label: null, detail: null })
})

it('does not infer a saved browser device is missing from a permission-filtered list', () => {
  expect(presenceOf({ source: 'browser', devices: [builtIn] }, 'saved-mic').state).toBe('unknown')
})

it('refreshes across capture without relying on devicechange and ignores the old in-flight list', async () => {
  let resolveInitial!: (value: { source: 'native'; devices: typeof usb[] }) => void
  list.mockReturnValueOnce(new Promise(resolve => { resolveInitial = resolve }))
  const { result, rerender } = renderHook(({ recording }) => useMicrophonePresence(null, recording), { initialProps: { recording: false } })
  list.mockResolvedValueOnce({ source: 'native', devices: [usb] })
  await act(async () => { rerender({ recording: true }) })
  expect(result.current?.state).toBe('connected')
  await act(async () => { resolveInitial({ source: 'native', devices: [] }) })
  expect(result.current?.state).toBe('connected')
  list.mockResolvedValueOnce({ source: 'native', devices: [usb] })
  await act(async () => { rerender({ recording: false }) })
  expect(result.current?.state).toBe('connected')
  expect(list).toHaveBeenCalledTimes(3)
})

it('ignores an older inventory that finishes after a devicechange refresh', async () => {
  let resolveInitial!: (value: { source: 'native'; devices: typeof usb[] }) => void
  list.mockReturnValueOnce(new Promise(resolve => { resolveInitial = resolve }))
  const { result } = renderHook(() => useMicrophonePresence(null))
  list.mockResolvedValueOnce({ source: 'native', devices: [usb] })
  await act(async () => { devices.dispatchEvent(new Event('devicechange')) })
  await act(async () => { resolveInitial({ source: 'native', devices: [] }) })
  expect(result.current?.state).toBe('connected')
})
