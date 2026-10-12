import { useEffect, useRef } from 'react'
import type { TurnView } from '../../../generated/contracts'
import { requestMessageHelp } from '../../../platform/ipc/message-help'
import { useI18n } from '../../../components/localization/i18n'
import { useHelpRequest, HelpStatus } from '../composer/HelpRequest'

/** Mounted only inside the learner's open feedback card. */
export function MessageAssessmentRequests({ messageId, execution, assessed, coached, assessmentState, assessmentError, feedbackState, feedbackError }: {
  assessmentState?: string | null; assessmentError?: string | null; feedbackState?: string | null; feedbackError?: string | null
  messageId: string; execution?: TurnView; assessed: boolean; coached: boolean
}) {
  const tr = useI18n()
  return <>
    <Request messageId={messageId} execution={execution} state={assessmentState} error={assessmentError} help="assessment" saved={assessed} label={tr('Message assessment')} />
    <Request messageId={messageId} execution={execution} state={feedbackState} error={feedbackError} help="coaching" saved={coached} label={tr('Coaching feedback')} />
  </>
}

function Request({ messageId, execution, state: reportedState, error, help, saved, label }: {
  messageId: string; execution?: TurnView; state?: string | null; error?: string | null; help: 'assessment' | 'coaching'; saved: boolean; label: string
}) {
  const tr = useI18n()
  const state = reportedState ?? (saved ? 'succeeded' : null)
  const lane = { state, revision: `${state}:${execution?.nativeGraph?.revision}`,
    error, details: { graph: execution?.nativeGraph } }
  const request = useHelpRequest(lane)
  const opened = useRef(false)
  useEffect(() => {
    if (opened.current) return
    opened.current = true
    if (state === null) request.submit(() => requestMessageHelp(messageId, help))
  }, [messageId, help, state, request])
  if (saved && state === 'succeeded') return null
  return <section aria-label={label}>
    <strong>{label}</strong>
    {state === null && !request.pending && !request.failure && <p>{tr('Not assessed')}</p>}
    <HelpStatus lane={lane} pending={request.pending} failure={request.failure} label={tr('Analyzing…')} subject={label}
      onRetry={() => request.submit(() => requestMessageHelp(messageId, help, ['failed', 'unknown'].includes(state ?? '')))} />
  </section>
}
