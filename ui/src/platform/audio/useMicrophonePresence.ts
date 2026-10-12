import { useEffect, useState } from 'react'
import type { MicrophoneList } from '../../generated/contracts'
import { errorMessage } from '../diagnostics/error-details'
import { reportFault } from '../diagnostics/faults'
import { listMicrophones } from './microphones'

/** Whether the saved microphone choice can be recorded from right now. */
export interface MicrophonePresence {
  /** `missing` covers an unplugged device, one the system cannot open, and no input at all. */
  state: 'connected' | 'missing' | 'unknown'
  /** The device's name when the list gives one; the default choice takes the system default's. */
  label: string | null
  /** The system's reason a listed device cannot open, or why the list could not be read. */
  detail: string | null
}

/** Reads one device list against the saved choice (`null` is the system default). */
export function presenceOf(list: MicrophoneList, device: string | null): MicrophonePresence {
  const found = list.devices.find(entry => device === null ? entry.isDefault : entry.id === device)
  // Browser inventories are permission-filtered, including saved device IDs.
  // Absence from that list does not establish that capture cannot open it.
  const absent = list.source === 'browser' ? 'unknown' : 'missing'
  if (device === null && !found) {
    if (!list.devices.length) return { state: absent, label: null, detail: null }
    // Browser inventories can contain inputs without identifying a default.
    return { state: 'connected', label: null, detail: null }
  }
  if (!found) return { state: absent, label: null, detail: null }
  if (found.unavailable !== null) return { state: 'missing', label: found.label || null, detail: found.unavailable }
  return { state: 'connected', label: found.label || null, detail: null }
}

/** Tracks whether the saved microphone choice is present: listed on mount, when
 * the choice or recording state changes and when the platform reports a device
 * change. Capture may reveal devices without a devicechange event. It never
 * asks for microphone access, so browser lists may leave names empty until a
 * recording grants it. `undefined` means the owner has no choice to track yet. */
export function useMicrophonePresence(device: string | null | undefined, recording = false): MicrophonePresence | null {
  const [presence, setPresence] = useState<MicrophonePresence | null>(null)
  useEffect(() => {
    if (device === undefined) { setPresence(null); return }
    let alive = true
    let revision = 0
    const refresh = () => {
      const current = ++revision
      listMicrophones(false).then(list => { if (alive && current === revision) setPresence(presenceOf(list, device)) }, (error: unknown) => {
        if (!alive || current !== revision) return
        reportFault('Listing microphones', error)
        setPresence({ state: 'unknown', label: null, detail: errorMessage(error) })
      })
    }
    refresh()
    const devices = typeof navigator === 'undefined' ? undefined : navigator.mediaDevices
    devices?.addEventListener?.('devicechange', refresh)
    return () => { alive = false; devices?.removeEventListener?.('devicechange', refresh) }
  }, [device, recording])
  return presence
}
