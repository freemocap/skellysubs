import { create } from 'zustand'
import type { GraphHoverMessage, GraphHoverReply, GraphHoverTarget } from '../../domain/activity/graph-surface'

/// The main window's current downstream-surface hint. Messages from each AI view
/// instance are ordered by their sequence; an older message never replaces a
/// newer one, so a delayed hover cannot outlive the clear that followed it.
interface GraphHoverStore {
  target: GraphHoverTarget | null
  origin: { session: string; seq: number; replyTo: string } | null
  latest: Record<string, number>
  /// The overlay's answer for the latest message from each session.
  replies: Record<string, GraphHoverReply>
  show: (message: GraphHoverMessage) => void
  reply: (reply: GraphHoverReply) => void
  clearAll: () => void
}

export const useGraphHoverStore = create<GraphHoverStore>((set, get) => ({
  target: null,
  origin: null,
  latest: {},
  replies: {},
  show: message => {
    const previous = get().latest[message.session]
    if (previous !== undefined && message.seq <= previous) return
    const latest = { ...get().latest, [message.session]: message.seq }
    if (message.target) set({ latest, target: message.target, origin: { session: message.session, seq: message.seq, replyTo: message.replyTo } })
    else if (get().origin?.session === message.session || get().origin === null) set({ latest, target: null, origin: null })
    else set({ latest })
  },
  reply: reply => {
    if (get().latest[reply.session] !== reply.seq) return
    set({ replies: { ...get().replies, [reply.session]: reply } })
  },
  clearAll: () => set({ target: null, origin: null }),
}))
