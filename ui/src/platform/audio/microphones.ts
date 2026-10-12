import { invoke } from '../ipc/native'
import type { MicrophoneDevice, MicrophoneList } from '../../generated/contracts'

/** The inputs this device can record from, in the form the stored selection
 * uses. Native lists desktop devices; where the webview records, the browser
 * lists them. Browsers withhold labels until microphone access is granted, so
 * `requestAccess` asks for it once and releases the stream immediately. */
export async function listMicrophones(requestAccess: boolean): Promise<MicrophoneList> {
  const native = await invoke<MicrophoneList>('list_microphones')
  if (native.source === 'native') return native
  if (!navigator.mediaDevices?.enumerateDevices) throw new Error('This device does not list microphones.')
  let inputs = (await navigator.mediaDevices.enumerateDevices()).filter(device => device.kind === 'audioinput')
  if (requestAccess && (!inputs.length || inputs.some(device => !device.label || !device.deviceId))) {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    stream.getTracks().forEach(track => track.stop())
    inputs = (await navigator.mediaDevices.enumerateDevices()).filter(device => device.kind === 'audioinput')
  }
  const devices: MicrophoneDevice[] = inputs.filter(device => device.deviceId).map(device => ({
    id: device.deviceId, label: device.label, isDefault: false,
    // Browsers do not report a device's format without opening it.
    channels: null, sampleRate: null, unavailable: null,
  }))
  return { source: 'browser', devices }
}
