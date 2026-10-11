import type { ComponentType } from 'react'
import type { useI18n } from '../../components/localization/i18n'
import { ChatDemo } from './demo/ChatDemo'
import { DrillDemo } from './demo/DrillDemo'
import { ProgressDemo } from './demo/ProgressDemo'
import { AiDemo } from './demo/AiDemo'

export type TourViewId = 'chat' | 'drill' | 'progress' | 'ai'

export interface TourStop {
  key: string
  /// Where this stop's control sits in its view's demo page. One selector: the
  /// demo always has this content, unlike the learner's own, possibly empty, app.
  selector: string
  title: (tr: ReturnType<typeof useI18n>) => string
  text: (tr: ReturnType<typeof useI18n>) => string
}

export interface TourView {
  id: TourViewId
  label: (tr: ReturnType<typeof useI18n>) => string
  Demo: ComponentType
  stops: readonly TourStop[]
}

/// The tour walks a filled-in demo of each surface — the same production
/// components the real app mounts, fed fixed content — rather than the
/// learner's own app, which may have nothing in it yet.
export const TOUR_VIEWS: readonly TourView[] = [
  {
    id: 'chat', label: tr => tr('Chat'), Demo: ChatDemo,
    stops: [
      { key: 'word', selector: '.partner-turn .reading-word',
        title: tr => tr('Tap a word'),
        text: tr => tr('Tap any word in a partner message to see its meaning. Words shows every word at once.') },
      { key: 'speak', selector: '.partner-turn .message-tools-play',
        title: tr => tr('Play aloud'),
        text: tr => tr('Plays the message in the partner’s voice.') },
      { key: 'tools', selector: '.partner-turn .message-actions',
        title: tr => tr('Translate and Analysis'),
        text: tr => tr('Translate shows the whole message in your language. Analysis explains the grammar and usage in it.') },
      { key: 'record', selector: '.composer .voice-pad',
        title: tr => tr('Reply'),
        text: tr => tr('Press the microphone and speak, or choose Type. Your recording is transcribed before it is sent.') },
      { key: 'score', selector: '.stream .feedback-badge',
        title: tr => tr('Feedback on your message'),
        text: tr => tr('Grammar and conversation-fit scores for what you wrote. Select it to open the corrections in Coach.') },
      { key: 'coach', selector: '.coach-heading',
        title: tr => tr('Coach'),
        text: tr => tr('Corrections for your last message. Ask the coach about any message, or use Help with this reply for suggestions.') },
      { key: 'deck', selector: '.stream .message-add-drill',
        title: tr => tr('Add to Practice'),
        text: tr => tr('Saves the message as a card. Press again to remove it.') },
      { key: 'progress', selector: '.profile-trigger',
        title: tr => tr('Progress'),
        text: tr => tr('Opens your XP and skills.') },
      { key: 'ai', selector: '.ai-status-pill',
        title: tr => tr('AI status'),
        text: tr => tr('Shows whether AI access is connected. Opens the AI panel with every model request for each turn.') },
    ],
  },
  {
    id: 'drill', label: tr => tr('Practice'), Demo: DrillDemo,
    stops: [
      { key: 'phrases', selector: '.drill-rail, .drill-phrase-bar',
        title: tr => tr('Practice cards'),
        text: tr => tr('Your saved practice cards. Select one to practise.') },
      { key: 'hear', selector: '.drill-reference .drill-play',
        title: tr => tr('Play reference'),
        text: tr => tr('Plays the reference recording. The speed control slows the voice. The spectrogram shows pitch and timing.') },
      { key: 'record', selector: '.drill-voice',
        title: tr => tr('Record an attempt'),
        text: tr => tr('Say the card. Recording settings control how recording starts and stops.') },
      { key: 'yours', selector: '.drill-media-take .drill-play',
        title: tr => tr('Play yours'),
        text: tr => tr('Plays your attempt. Its spectrogram sits under the reference for comparison.') },
      { key: 'compare', selector: '.drill-comparison-panel',
        title: tr => tr('Word match'),
        text: tr => tr("The card's words against what was heard. Coloured words differ from the card.") },
      { key: 'history', selector: '.drill-word-grid',
        title: tr => tr('Attempts'),
        text: tr => tr('Select an attempt to inspect its words and timing.') },
    ],
  },
  {
    id: 'progress', label: tr => tr('Progress'), Demo: ProgressDemo,
    stops: [
      { key: 'level', selector: '.skill-levels-head',
        title: tr => tr('Skill level'),
        text: tr => tr('Your level is your weakest skill’s level. Replies earn skill points, and each skill levels up as its points grow.') },
      { key: 'chart', selector: '.skill-chart',
        title: tr => tr('Skill chart'),
        text: tr => tr('One arm or bar per skill. Switch between radial and bars, and between normalized and to scale.') },
      { key: 'focus', selector: '.skill-levels-focus',
        title: tr => tr('Selected skill'),
        text: tr => tr('What the selected skill covers, what counts for it, and the points it still needs.') },
      { key: 'next', selector: '.skill-levels-next',
        title: tr => tr('Next level'),
        text: tr => tr('The skills still short of the next level, and how many points each needs.') },
    ],
  },
  {
    id: 'ai', label: tr => tr('AI panel'), Demo: AiDemo,
    stops: [
      { key: 'summary', selector: '.ai-view-head',
        title: tr => tr('Activity'),
        text: tr => tr('What is running for the current turn, and how many requests have finished.') },
      { key: 'graph', selector: '.ai-workspace-layer',
        title: tr => tr('Requests'),
        text: tr => tr('Each bubble is one step of the AI’s work. Lines carry a step’s result to the steps that use it, colored by the kind of value.') },
      { key: 'kinds', selector: '.ai-workspace-layer',
        title: tr => tr('Three kinds of model'),
        text: tr => tr('Text to text writes replies, glosses, translations, coaching and assessments. Speech to text transcribes your recordings. Text to speech reads messages aloud.') },
      { key: 'details', selector: '.ai-run-toolbar',
        title: tr => tr('Details'),
        text: tr => tr('Click a step to open it in place: its inputs, outputs and latest attempt. Click its title for every recorded fact, including the exact response.') },
    ],
  },
]

export function tourView(id: TourViewId): TourView {
  const view = TOUR_VIEWS.find(item => item.id === id)
  if (!view) throw new Error(`Unknown tour view: ${id}`)
  return view
}
