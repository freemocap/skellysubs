import { useCallback, useEffect, useRef, useState } from 'react'
import type { GraphHoverTarget } from '../../domain/activity/graph-surface'
import { currentWindowLabel, emitGraphHover, onGraphHoverReply } from '../../platform/ipc/window'
import { reportFault } from '../../platform/diagnostics/faults'
import { useGraphHoverStore } from '../../state/navigation/graph-hover'

/// Publishes downstream-surface hints from one AI view. In the main window it
/// writes the shared store directly; from the popped-out window it sends ordered
/// messages to the main window. Updates are coalesced to one per frame; a clear
/// cancels any scheduled update and is sent at once with the next sequence.
export function useGraphHoverPublisher(remote: boolean) {
  const session = useRef(crypto.randomUUID())
  const seq = useRef(0)
  const frame = useRef<number | null>(null)
  const pending = useRef<GraphHoverTarget | null>(null)
  const label = useRef<string | null>(null)
  const [visible, setVisible] = useState<{ seq: number; matched: number; visible: number } | null>(null)

  const send = useCallback((target: GraphHoverTarget | null) => {
    seq.current += 1
    const message = { session: session.current, seq: seq.current, replyTo: remote ? label.current ?? 'ai' : 'local', target }
    if (!remote) { useGraphHoverStore.getState().show(message); return }
    emitGraphHover(message).catch(error => reportFault('Sending an AI graph hover hint', error))
  }, [remote])

  const publish = useCallback((target: GraphHoverTarget | null) => {
    if (target === null) {
      if (frame.current !== null) cancelAnimationFrame(frame.current)
      frame.current = null
      pending.current = null
      setVisible(null)
      send(null)
      return
    }
    pending.current = target
    if (frame.current !== null) return
    frame.current = requestAnimationFrame(() => {
      frame.current = null
      const next = pending.current
      pending.current = null
      if (next) send(next)
    })
  }, [send])

  useEffect(() => {
    if (!remote) return
    let active = true
    let stop: () => void = () => {}
    void currentWindowLabel().then(value => { if (active) label.current = value })
      .catch(error => reportFault('Reading the AI window identity', error))
    void onGraphHoverReply(reply => {
      if (reply.session === session.current && reply.seq === seq.current) setVisible(reply)
    }).then(unlisten => { if (active) stop = unlisten; else unlisten() })
      .catch(error => reportFault('Listening for AI graph hover replies', error))
    return () => { active = false; stop() }
  }, [remote])

  const localReply = useGraphHoverStore(state => state.replies[session.current])
  useEffect(() => {
    if (remote || !localReply || localReply.seq !== seq.current) return
    setVisible(localReply)
  }, [remote, localReply])

  useEffect(() => {
    const clear = () => publish(null)
    const hidden = () => { if (document.visibilityState === 'hidden') clear() }
    window.addEventListener('blur', clear)
    window.addEventListener('pagehide', clear)
    document.addEventListener('visibilitychange', hidden)
    return () => {
      window.removeEventListener('blur', clear)
      window.removeEventListener('pagehide', clear)
      document.removeEventListener('visibilitychange', hidden)
      clear()
    }
  }, [publish])

  return { publish, visible: visible && visible.seq === seq.current ? visible : null }
}
