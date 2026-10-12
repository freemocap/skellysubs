import type { TurnView } from '../../generated/contracts'
import { messageKey } from '../localization/messages'

/// One step in three wordings, longest first: at most three words, then two,
/// then one. The composer shows the longest that fits beside the AI pill.
export type StatusWords = readonly [string, string, string]

/// Steps the recorded operations do not name: the recording, the send, the
/// queue, a prose reply's request and its token stream, and the connection.
/// Every wording names the machinery at work, so the line teaches how a turn
/// is made to anyone who glances at it.
export const STEP_WORDS = {
  transcribe: [messageKey('Transcribing recorded audio…'), messageKey('Transcribing audio…'), messageKey('Transcribing…')],
  schedule: [messageKey('Scheduling reply turn…'), messageKey('Scheduling turn…'), messageKey('Scheduling…')],
  dispatch: [messageKey('Awaiting request dispatch…'), messageKey('Awaiting dispatch…'), messageKey('Queued…')],
  replyRequest: [messageKey('Generating partner reply…'), messageKey('Generating reply…'), messageKey('Generating…')],
  replyStream: [messageKey('Streaming reply tokens…'), messageKey('Receiving reply…'), messageKey('Streaming…')],
  coachRequest: [messageKey('Generating coach reply…'), messageKey('Generating reply…'), messageKey('Generating…')],
  coachStream: [messageKey('Streaming coach tokens…'), messageKey('Receiving reply…'), messageKey('Streaming…')],
  check: [messageKey('Checking AI connection…'), messageKey('Checking connection…'), messageKey('Checking…')],
  offline: [messageKey('AI not connected'), messageKey('Not connected'), messageKey('Disconnected')],
  // A card's reference audio comes from native's speech cache or a new
  // synthesis; the reading result does not say which, so it is only fetched.
  cardAudio: [messageKey('Fetching card audio…'), messageKey('Fetching audio…'), messageKey('Fetching…')],
} as const satisfies Record<string, StatusWords>

const SPEECH: StatusWords = [messageKey('Synthesizing partner voice…'), messageKey('Synthesizing voice…'), messageKey('Synthesizing…')]
const BUFFERING: StatusWords = [messageKey('Buffering partner voice…'), messageKey('Buffering voice…'), messageKey('Buffering…')]

/// What one recording surface is doing. Chat supplies its turns; Practice has
/// none and reports its attempts' transcription and its cards' audio.
export interface AiStatusInput {
  /// Recorded audio is with the speech service.
  transcribing: boolean
  /// A send is with native storage, before its turn is recorded.
  scheduling: boolean
  /// The conversation's recorded turns, newest first.
  turns: readonly Pick<TurnView, 'id' | 'channel' | 'nativeGraph' | 'nativePreview'>[]
  /// Speech audio requested for playback, until it plays: a partner message's
  /// voice being synthesized, or a practice card's audio being fetched.
  audio: 'partner' | 'buffering' | 'card' | null
  connection: 'connected' | 'checking' | 'disconnected'
  /// The configured audio models, named beside the steps they run.
  models: { transcription: string | null; speech: string | null }
}

export interface AiStatusLine {
  /// A new id is a new line in the stream; the same id keeps its place.
  id: string
  tone: 'work' | 'check' | 'offline'
  /// Steps the learner started are announced; background work is only shown.
  announce: boolean
  /// Catalog wordings, longest first; null names the operation by its kind.
  words: StatusWords | null
  /// Executable node identities and known requested models behind the line.
  kinds: string[]
  models: string[]
}

function line(id: string, words: StatusWords | null, details: Partial<AiStatusLine> = {}): AiStatusLine {
  return { id, tone: 'work', announce: false, words, kinds: [], models: [], ...details }
}

/// Report the newest turn with running graph work. The reply preview identifies
/// its producing attempt; node names and ordering come from the artifact snapshot.
function runningWork(turns: AiStatusInput['turns']): AiStatusLine | null {
  for (const turn of turns) {
    const graph = turn.nativeGraph
    if (!graph) continue
    const running = Object.entries(graph.nodes).filter(([, state]) => state === 'Running').map(([node]) => node)
    if (!running.length) continue
    const preview = turn.nativePreview
    const reply = preview && running.find(node => graph.attempts[node]?.some(attempt =>
      attempt.id === preview.attempt && attempt.execution === preview.execution && attempt.state === 'Running'))
    if (reply) {
      const streamed = preview.live && preview.capture.text.length > 0
      const coach = turn.channel === 'coach'
      const words = streamed ? (coach ? STEP_WORDS.coachStream : STEP_WORDS.replyStream) : (coach ? STEP_WORDS.coachRequest : STEP_WORDS.replyRequest)
      return line(`${streamed ? 'stream' : 'request'}:${turn.id}:${reply}`, words, { kinds: [reply], announce: !streamed })
    }
    return line(`run:${turn.id}:${running.join(':')}`, null, { kinds: running })
  }
  return null
}

/// What the AI is doing now, as one line, and whether any of it is AI work.
/// Learner steps lead, then recorded operations, then queued work and replayed
/// speech; the connection speaks only when nothing else does. Held and failed
/// operations are not work: the latest turn's activity line reports them.
export function aiStatus(input: AiStatusInput): { busy: boolean; line: AiStatusLine | null } {
  const running = runningWork(input.turns)
  const queued = !running && input.turns.some(turn => Object.values(turn.nativeGraph?.nodes ?? {}).some(state => state === 'Ready' || state === 'Prepared' || state === 'Available'))
  const busy = input.transcribing || input.scheduling || input.audio !== null || running !== null || queued
  const model = (name: string | null) => name ? [name] : []
  if (input.transcribing) return { busy, line: line('transcribe', STEP_WORDS.transcribe,
    { announce: true, kinds: ['speech_transcription'], models: model(input.models.transcription) }) }
  if (input.scheduling) return { busy, line: line('schedule', STEP_WORDS.schedule, { announce: true }) }
  if (input.audio === 'buffering') return { busy, line: line('buffering', BUFFERING, { kinds: ['persona_speech'], models: model(input.models.speech) }) }
  if (running) return { busy, line: running }
  if (queued) return { busy, line: line('dispatch', STEP_WORDS.dispatch) }
  if (input.audio === 'partner') return { busy, line: line('synthesize', SPEECH, { kinds: ['persona_speech'], models: model(input.models.speech) }) }
  if (input.audio === 'card') return { busy, line: line('card-audio', STEP_WORDS.cardAudio, { models: model(input.models.speech) }) }
  if (input.connection === 'checking') return { busy, line: line('check', STEP_WORDS.check, { tone: 'check' }) }
  if (input.connection === 'disconnected') return { busy, line: line('offline', STEP_WORDS.offline, { tone: 'offline' }) }
  return { busy, line: null }
}
