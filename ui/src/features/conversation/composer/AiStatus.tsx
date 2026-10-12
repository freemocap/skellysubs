import type { ReactNode } from 'react'
import { AiStatusPill } from '../../../components/feedback/AiStatusPill'
import { aiStatus } from '../../../domain/activity/ai-status'
import type { TurnView } from '../../../generated/contracts'
import { openAiActivity, useAiActivityOpen } from '../../../state/navigation/ai-activity'
import { useAiAccess } from '../../../state/session/ai-access'
import { LatestTurnActivity } from '../messages/TurnActivityLine'

/// The AI in Chat's composer status line: what the conversation's recording,
/// send, turns and reply speech are doing, on the shared AI pill. Connected,
/// the pill opens the AI View; not connected, AI access.
export function AiStatus({ transcribing, scheduling, turns, synthesizing, buffering = false, latest, onInspectLatest, beside }: {
  transcribing: boolean
  scheduling: boolean
  /// The conversation's recorded turns, newest first.
  turns: readonly TurnView[]
  /// A partner message's speech is requested for playback and has not started.
  synthesizing: boolean
  buffering?: boolean
  /// The newest exchange with a landed reply: its failed or held follow-on work.
  latest?: TurnView
  onInspectLatest?: () => void
  /// Controls that sit right after the pill.
  beside: ReactNode
}) {
  const access = useAiAccess()
  const open = useAiActivityOpen()
  const { busy, line } = aiStatus({ transcribing, scheduling, turns, audio: buffering ? 'buffering' : synthesizing ? 'partner' : null,
    connection: access.status, models: access.models })
  return <AiStatusPill access={access} open={open} onOpen={() => openAiActivity(access.status)} busy={busy} line={line} beside={beside}
    fallback={latest ? <LatestTurnActivity execution={latest} onActivity={onInspectLatest} fallback={null} /> : null} />
}
