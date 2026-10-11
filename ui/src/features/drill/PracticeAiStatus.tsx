import { AiStatusPill } from '../../components/feedback/AiStatusPill'
import { aiStatus } from '../../domain/activity/ai-status'
import { openAiActivity, useAiActivityOpen } from '../../state/navigation/ai-activity'
import { useAiAccess } from '../../state/session/ai-access'

/// The AI above Practice's recorder, on the shared AI pill: attempts on their
/// way through the speech service, and a card's audio on its way to playback.
/// Practice records no conversation turns, so nothing else reports here.
export function PracticeAiStatus({ transcribing, fetchingAudio }: { transcribing: boolean; fetchingAudio: boolean }) {
  const access = useAiAccess()
  const open = useAiActivityOpen()
  const { busy, line } = aiStatus({ transcribing, scheduling: false, turns: [], audio: fetchingAudio ? 'card' : null,
    connection: access.status, models: access.models })
  return <AiStatusPill access={access} open={open} onOpen={() => openAiActivity(access.status)} busy={busy} line={line} beside={null} />
}
