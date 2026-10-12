import type { ReactNode } from 'react'
import type { GlossSegment } from '../../generated/contracts'
import { ToolbarIcon } from '../controls/ToolbarIcon'
import { AskCoachButton } from '../learning/AskCoachButton'
import { useI18n } from '../localization/i18n'
import { GlossHelpParts } from './GlossHelpParts'
import { TokenAudio } from './TokenAudio'
import { SpeechFollowText } from './SpeechFollowText'
import { useReadingActions, useReadingScope, type ReadingSelection } from './ReadingContext'

/** Identical word details and actions for saved, imported and newly fetched help. */
export function WordHelpContent({ text, start, end, parts, selection, blank = false, children, onClose }: {
  text: string; start: number; end: number; parts: GlossSegment[]
  selection?: ReadingSelection; blank?: boolean; children?: ReactNode; onClose: () => void
}) {
  const tr = useI18n()
  const actions = useReadingActions(), scope = useReadingScope()
  const selected = selection ?? (scope ? { text, start, end, scope } : null)
  const source = text.slice(start, end)
  const saved = parts.filter(part => part.start < end && part.end > start)
  const question = `Help me understand “${source}” in this passage: “${text}”. Selected range: [${start}, ${end}) in UTF-16 code units.`
    + (saved.length ? ` Saved reading information: ${JSON.stringify(saved)}` : '')
  return <>
    <AskCoachButton compact question={question} onClose={onClose} />
    {!blank && <TokenAudio text={text} start={start} end={end} />}
    <SpeechFollowText text={source} source={{ text, start }}><span data-speech-source className="reading-help-source" dir="auto">{source}</span></SpeechFollowText>
    {!blank && <GlossHelpParts text={text} parts={parts} />}
    {children}
    {actions && selected && <button type="button" className="reading-help-action" aria-label={tr('Analysis')} title={tr('Analysis')}
      onClick={() => { onClose(); actions.inspect(selected) }}><ToolbarIcon name="analysis" size={16} /></button>}
  </>
}
