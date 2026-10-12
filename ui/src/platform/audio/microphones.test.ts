// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
const native = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('../ipc/native', () => native)
import { listMicrophones } from './microphones'
import { presenceOf } from './useMicrophonePresence'

afterEach(() => { vi.unstubAllGlobals(); native.invoke.mockReset() })

it('returns natively listed devices without touching the browser', async () => {
  const list = { source: 'native', devices: [{ id: 'Yeti', label: 'Yeti', isDefault: true, channels: 2, sampleRate: 48000, unavailable: null }] }
  native.invoke.mockResolvedValue(list)
  const enumerateDevices = vi.fn()
  vi.stubGlobal('navigator', { mediaDevices: { enumerateDevices } })
  expect(await listMicrophones(true)).toEqual(list)
  expect(native.invoke).toHaveBeenCalledWith('list_microphones')
  expect(enumerateDevices).not.toHaveBeenCalled()
})

it('lists browser inputs, asking for access only on request to reveal labels', async () => {
  native.invoke.mockResolvedValue({ source: 'browser', devices: [] })
  let granted = false
  const enumerateDevices = vi.fn(async () => [
    { kind: 'audioinput', deviceId: 'mic-a', label: granted ? 'Headset' : '' },
    { kind: 'audiooutput', deviceId: 'speaker', label: '' },
  ])
  const stop = vi.fn()
  const getUserMedia = vi.fn(async () => { granted = true; return { getTracks: () => [{ stop }] } })
  vi.stubGlobal('navigator', { mediaDevices: { enumerateDevices, getUserMedia } })
  expect((await listMicrophones(false)).devices).toEqual([{ id: 'mic-a', label: '', isDefault: false, channels: null, sampleRate: null, unavailable: null }])
  expect(getUserMedia).not.toHaveBeenCalled()
  const listed = await listMicrophones(true)
  expect(listed).toMatchObject({ source: 'browser', devices: [{ id: 'mic-a', label: 'Headset' }] })
  expect(getUserMedia).toHaveBeenCalledWith({ audio: true })
  expect(stop).toHaveBeenCalledOnce()
})

it.each([{ inputs: [] }, { inputs: [{ kind: 'audioinput', deviceId: '', label: '' }] }])('handles hidden browser inputs without claiming disconnection: $inputs', async ({ inputs }) => {
  native.invoke.mockResolvedValue({ source: 'browser', devices: [] })
  const enumerateDevices = vi.fn().mockResolvedValue(inputs)
  const stop = vi.fn()
  const getUserMedia = vi.fn(async () => {
    enumerateDevices.mockResolvedValue([{ kind: 'audioinput', deviceId: 'mic', label: 'Phone microphone' }])
    return { getTracks: () => [{ stop }] }
  })
  vi.stubGlobal('navigator', { mediaDevices: { enumerateDevices, getUserMedia } })
  const hidden = await listMicrophones(false)
  expect(presenceOf(hidden, null).state).toBe('unknown')
  expect(presenceOf(hidden, 'mic').state).toBe('unknown')
  expect(getUserMedia).not.toHaveBeenCalled()
  expect((await listMicrophones(true)).devices).toMatchObject([{ id: 'mic', label: 'Phone microphone' }])
  expect(getUserMedia).toHaveBeenCalledExactlyOnceWith({ audio: true })
  expect(stop).toHaveBeenCalledOnce()
})
