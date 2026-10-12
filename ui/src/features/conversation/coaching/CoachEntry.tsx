import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { useI18n } from '../../../components/localization/i18n'
import { TargetPassage } from '../../../components/reading/TargetPassage'
import { TargetText } from '../../../components/reading/TargetText'
import type { CoachDecision, CoachObservationView } from '../../../generated/contracts'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { coachFlags } from '../../../domain/conversation/coach-marks'
import { correctionChanges } from '../../../domain/conversation/correction-diff'

/** Learner-facing explanations only. Native diagnostics never substitute for help. */
export function CoachEntry({ graphTarget, decision, feedback, source, error, revealSelected = false }: {
  graphTarget?: Record<string, string>
  /** Opening the feedback dialog reveals its already-saved selection immediately. */
  revealSelected?: boolean
  decision?: CoachDecision; feedback?: CoachObservationView; source: string | null; error?: string
}) {
  const tr = useI18n()
  const shown = decision?.shown && !decision.keptGoing && (revealSelected || decision.exposedMove === decision.shown.move) ? decision.shown : null
  const flags = coachFlags(feedback, decision)
  const corrections = [...(shown ? [shown] : []), ...(decision?.exposedMove === 'explicit' && !decision.keptGoing ? feedback?.corrections ?? [] : [])].filter((item, index, all) => all.findIndex(other => other.quote === item.quote && other.text === item.text) === index)
  const explanations = !decision?.keptGoing ? (feedback?.items ?? []).filter(item => item.rationale.trim() && item.outcome !== 'not_observed').filter((item, index, all) => all.findIndex(other => other.quote === item.quote && other.rationale === item.rationale) === index) : []
  return <div className="coach-entry" {...graphTarget}>
    {source && <div className="coach-entry-said"><TargetPassage side="me" text={source} /></div>}
    {error && <ErrorNotice as="p" error={error} className="turn-errors">{error}</ErrorNotice>}
    {corrections.map((shown, index) => <section key={index} className="coach-card coach-card-help" aria-label={tr("Coaching suggestion")}>
      <h3 className="coach-card-title" aria-hidden="true"><ToolbarIcon name="idea" size={15} />{tr("Coaching suggestion")}</h3>
      {shown.move === 'explicit' ? <ExplicitCorrection quote={shown.quote} text={shown.text} explanation={shown.explanation} /> : <>
        <blockquote><TargetPassage side="me" text={shown.quote} /></blockquote>
        <p className="coach-remark" dir="auto">{shown.text}</p>
      </>}
    </section>)}
    {feedback && !corrections.length && !explanations.length && !feedback.notes.length && !error && !decision?.keptGoing && <p className="coach-remark">{tr(flags.length ? 'Suggestions found' : feedback.meaningRecovered === 'none' ? 'The meaning could not be determined. Add context to clarify what you intended.' : feedback.meaningRecovered === 'partial' ? 'Only part of the meaning was clear. Add context to clarify what you intended.' : 'No correction identified.', { count: flags.length })}</p>}
    {explanations.map((explanation, index) => <section key={index} className="coach-card coach-card-explanation" aria-label={tr("Language explanation")}>
      <blockquote><TargetPassage side="me" text={explanation.quote} /></blockquote>
      <p className="coach-remark" dir="auto">{explanation.rationale}</p>
    </section>)}
    {!!feedback?.notes.length && <details><summary>{tr("Details")}</summary>{feedback.notes.map(note => <p key={note}>{note}</p>)}</details>}
  </div>
}

/** An explicit correction: the changed words first, when a compact view is
 * clear, then the explanation and the full original and replacement, which
 * are always kept. The compact rows are presentation only. */
function ExplicitCorrection({ quote, text, explanation }: { quote: string; text: string; explanation?: string }) {
  const tr = useI18n()
  const changes = correctionChanges(quote, text)
  const full = <div className="cor-line">
    <del className="cor-removed"><span className="sr-only">{tr("Original")}: </span><TargetText text={quote} interactive={false} /></del>
    <span className="cor-arrow" aria-hidden="true">→</span>
    <div className="cor-replacement"><span className="sr-only">{tr("Coaching suggestion")}: </span><TargetPassage text={text} /></div>
  </div>
  return <>
    {changes && <ul className="cor-changes" aria-label={tr("Changed words")}>{changes.map((change, index) => <li key={index}>
      {change.removed && <del className="cor-change-removed"><span className="sr-only">{tr("Original")}: </span><TargetText text={change.removed} interactive={false} /></del>}
      <span className="cor-arrow" aria-hidden="true">→</span>
      {change.added && <ins className="cor-change-added"><span className="sr-only">{tr("Coaching suggestion")}: </span><TargetText text={change.added} interactive={false} /></ins>}
    </li>)}</ul>}
    {explanation && <p className="cor-why" dir="auto">{explanation}</p>}
    {changes ? <details className="cor-full" open><summary>{tr("Full original and suggestion")}</summary>{full}</details> : full}
  </>
}
