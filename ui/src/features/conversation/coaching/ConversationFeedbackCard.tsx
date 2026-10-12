import { useI18n } from '../../../components/localization/i18n'
import type { ChoiceAssessment, ConversationFeedback } from '../../../generated/contracts'

/** The tone of each assessment choice, for its colour only; the word always accompanies it. */
const choiceTone: Record<string, 'good' | 'mixed' | 'poor' | 'none'> = {
  acceptable: 'good', understandable: 'good', local_errors: 'mixed', needs_clarification: 'mixed',
  major_errors: 'poor', unrecoverable: 'poor', insufficient_evidence: 'none',
}

/** One assessment: its choice, the saved probability distribution as a bar, a
 * fixed description of what that choice means (generic, not a reason for this
 * message) and the exact numbers under Assessment details. */
function Judgment({ label, answer }: { label: string; answer: ChoiceAssessment }) {
  const tr = useI18n()
  const labels: Record<string, string> = {
    acceptable: tr('Acceptable'), local_errors: tr('Local errors'), major_errors: tr('Major errors'),
    understandable: tr('Understood'), needs_clarification: tr('Needs clarification'),
    unrecoverable: tr('Meaning not recovered'), insufficient_evidence: tr('Insufficient evidence'),
  }
  const meanings: Record<string, string> = {
    acceptable: tr('The wording is grammatically acceptable for what you meant in this context.'),
    local_errors: tr('Some grammar errors, but the main construction can still be followed.'),
    major_errors: tr('Grammar errors get in the way of the construction or what it connects.'),
    understandable: tr('Your partner can follow what you meant without asking.'),
    needs_clarification: tr('Your partner can follow part of it but would need something clarified.'),
    unrecoverable: tr('Your partner cannot work out what you meant from the wording and context.'),
    insufficient_evidence: tr('There was not enough wording or context to judge.'),
  }
  const shares = Object.entries(answer.probabilities).filter(([, probability]) => probability > 0)
  return <div className="coach-score" data-tone={choiceTone[answer.choice] ?? 'none'}>
    <span>{label}</span><strong>{labels[answer.choice] ?? answer.choice}</strong>
    <span className="coach-score-shares" aria-hidden="true">{shares.map(([choice, probability]) =>
      <span key={choice} data-tone={choiceTone[choice] ?? 'none'} style={{ flexGrow: probability }} />)}</span>
    {meanings[answer.choice] && <p className="coach-score-meaning">{meanings[answer.choice]}</p>}
    <details>
      <summary>{tr('Assessment details')}</summary>
      <p>{tr('Model confidence')}: {tr.number(answer.confidence, { style: 'percent', maximumFractionDigits: 1 })}</p>
      <table>
        <caption>{tr('Model probabilities')}</caption>
        <tbody>{Object.entries(answer.probabilities).map(([choice, probability]) => <tr key={choice}>
          <th scope="row">{labels[choice] ?? choice}</th>
          <td>{tr.number(probability, { style: 'percent', maximumFractionDigits: 1 })}</td>
        </tr>)}</tbody>
      </table>
    </details>
  </div>
}

export function scoreText(value: number | null) { return value === null ? '—' : `${value}/10` }
/** How a 0–10 score reads: low, partial or strong. */
export function scoreLevel(value: number): 'low' | 'partial' | 'strong' {
  return value <= 4 ? 'low' : value <= 7 ? 'partial' : 'strong'
}
/** The detailed feedback card's ten-step score bar, coloured by its level. */
export function ScoreBar({ label, value }: { label: string; value: number }) {
  return <span className="coach-meter" data-level={scoreLevel(value)} role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={10} aria-valuenow={value} aria-valuetext={`${label}: ${value}/10`}>
    {Array.from({length:10},(_,i)=>i+1).map(step => <span key={step} data-on={step <= value} />)}
  </span>
}
/** A 0–10 score as a small ring for the line under a message; its name and value are text elsewhere. */
export function ScoreRing({ value }: { value: number | null }) {
  // Circumference of r=7 is 2π·7; the arc is the score's share of it.
  const circumference = 2 * Math.PI * 7
  return <svg className="score-ring" data-level={value === null ? undefined : scoreLevel(value)} viewBox="0 0 20 20" aria-hidden="true">
    <circle className="score-ring-track" cx="10" cy="10" r="7" />
    {value !== null && <circle className="score-ring-arc" cx="10" cy="10" r="7" strokeDasharray={`${circumference * value / 10} ${circumference}`} transform="rotate(-90 10 10)" />}
  </svg>
}
function ScoreMeter({ label, value }: { label: string; value: number | null }) {
  const tr = useI18n()
  if (value === null) return <div className="coach-score"><span>{label}</span><strong>{tr('Insufficient evidence')}</strong></div>
  return <div className="coach-score">
    <span className="coach-score-label">{label}</span>
    <span className="coach-score-row"><ScoreBar label={label} value={value} /><strong aria-hidden="true">{scoreText(value)}</strong></span>
  </div>
}
export function ConversationFeedbackCard({ feedback, graphTarget }: { feedback: ConversationFeedback; graphTarget?: Record<string, string> }) {
  const tr = useI18n()
  const grammar = feedback.answers.grammar
  const understanding = feedback.answers.understandability
  return <section {...graphTarget} className="coach-assessment" aria-label={tr('Message assessment')}>
    {grammar && understanding && <p className="coach-assessment-note">{tr('Each check is judged on its own and can differ from the coach’s suggestion.')}</p>}
    <div className="coach-scores" data-layout={grammar && understanding ? 'judgments' : 'meters'}>{grammar && understanding ? <>
      <Judgment label={tr('Grammar')} answer={grammar} />
      <Judgment label={tr('Partner understanding')} answer={understanding} />
    </> : <><ScoreMeter label={tr('Grammar')} value={feedback.grammar}/><ScoreMeter label={tr('Conversation fit')} value={feedback.conversation}/></>}</div>
  </section>
}
