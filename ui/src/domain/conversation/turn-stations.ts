import type { ChatMessage, SpeechRequestView } from '../../generated/contracts'
import type { InspectionSnapshot } from '../../generated/graph-contracts'
import type { Tone } from '../activity/graph-overlay'
import { messageKey } from '../localization'

/// Stations around a conversation turn's run: the learner's message, which
/// `sendMessage` turns into the graph inputs that carry it, and the playback of
/// the audio result, which the app streams after the run. Each station names
/// only the ports the artifact declares, so a graph without them has none.

/// A fact shown on a station: plain text, or a moment to format in the viewer's locale.
export type StationValue = { kind: 'text'; text: string } | { kind: 'time'; at: string }

export interface TurnStation {
  id: string
  side: 'before' | 'after'
  nameKey: string
  tone: Tone | 'none'
  /// Graph inputs supplied (before) or graph results taken (after).
  ports: string[]
  rows: Array<{ labelKey: string; value: StationValue }>
}

/// Graph inputs that carry the learner's message: the captured context whose last
/// entry is the message, and the message as a reading source.
const MESSAGE_INPUTS = ['context', 'learner_source']
const AUDIO_RESULT = 'audio'

/// Speech request states, as the conversation reports them, mapped to tones.
const PLAYBACK_TONE: Record<string, Tone> = { ready: 'waiting', pending: 'waiting', running: 'running', succeeded: 'done', failed: 'problem', unknown: 'problem', held: 'held', cancelled: 'ended' }

export function turnStations(graph: InspectionSnapshot, message: ChatMessage | undefined, speech: SpeechRequestView | undefined): TurnStation[] {
  const stations: TurnStation[] = []
  const definition = graph.artifact.definition
  const messagePorts = MESSAGE_INPUTS.filter(input => Object.hasOwn(definition.inputs, input))
  if (message && messagePorts.length) {
    stations.push({
      id: 'station:message', side: 'before', nameKey: messageKey('Your message'), tone: 'none', ports: messagePorts,
      rows: [{ labelKey: messageKey('text'), value: { kind: 'text', text: message.text } }, { labelKey: messageKey('sent'), value: { kind: 'time', at: message.createdAt } }],
    })
  }
  if (speech && Object.hasOwn(definition.results, AUDIO_RESULT)) {
    const tone = PLAYBACK_TONE[speech.state]
    if (!tone) throw new Error(`Unknown speech request state: ${speech.state}`)
    stations.push({
      id: 'station:playback', side: 'after', nameKey: messageKey('Playback'), tone, ports: [AUDIO_RESULT],
      rows: [{ labelKey: messageKey('state'), value: { kind: 'text', text: speech.state } }, { labelKey: messageKey('request'), value: { kind: 'text', text: speech.id } }],
    })
  }
  return stations
}
