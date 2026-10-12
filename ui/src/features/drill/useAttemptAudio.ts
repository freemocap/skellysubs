import { useEffect, useState } from 'react'
import { drillAttemptAudio, inspectDrillAudio } from '../../platform/ipc/drill'
import type { AudioInspection, DrillAttemptView } from '../../generated/contracts'

/** A retained attempt recording and native's analysis of it. */
export interface AttemptAudio { attemptId: string; base64: string; inspection: AudioInspection | null }

/** Read one attempt's retained audio and analyse it against its item. A pruned
 * or never-kept recording loads nothing; a failed read is kept for the caller
 * to show, and `retry` reads it again without creating another attempt. */
export function useAttemptAudio(itemId: string | null, attempt: DrillAttemptView | null) {
  const [audio, setAudio] = useState<AttemptAudio | null>(null)
  const [failure, setFailure] = useState<unknown>(null)
  const [retries, setRetries] = useState(0)
  const attemptId = attempt?.id ?? null
  const retained = attempt !== null && attempt.audioBytes !== null && attempt.audioPrunedAt === null
  useEffect(() => {
    setAudio(null); setFailure(null)
    if (!itemId || !attemptId || !retained) return
    let current = true
    void drillAttemptAudio(attemptId)
      .then(async base64 => {
        if (!current) return
        setAudio({ attemptId, base64, inspection: null })
        const inspection = await inspectDrillAudio(itemId, base64, { attemptId })
        if (current) setAudio({ attemptId, base64, inspection })
      })
      .catch(error => { if (current) setFailure(error) })
    return () => { current = false }
  }, [itemId, attemptId, retained, retries])
  return {
    audio: audio?.attemptId === attemptId ? audio : null,
    failure,
    retry: () => setRetries(value => value + 1),
  }
}
