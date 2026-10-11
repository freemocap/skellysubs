import { MessageSkillAnalysis } from './MessageSkillAnalysis'
import { ErrorDetails } from '../../../components/feedback/ErrorDetails'
import { useI18n } from '../../../components/localization/i18n'
import { AnalysisSentence } from './AnalysisSentence'
import { ExplanationCards } from '../../../components/reading/ExplanationCards'
import { SavedReadingProvider } from '../../../components/reading/SavedReadingProvider'
import { useReadingScope } from '../../../components/reading/ReadingContext'
import { anchoredTokenGlosses } from '../../../domain/reading/gloss-display'
import { memo, useMemo, useEffect, useRef, type ReactNode } from 'react'
import { HelpStatus, useHelpRequest } from '../composer/HelpRequest'
import { requestReplyHelp } from '../composer/TurnReplyHelp'
import { MessageReadingScope } from './MessageReadingScope'
import { useNavigationStore } from '../../../state/navigation/navigation'
import type { StoredTurn } from '../../../types'
import { ActivityIndicator } from '../../../components/feedback/ActivityIndicator'

/// Just the parts of a turn this pane renders. Deriving from `StoredTurn` keeps
/// this view synchronized with the canonical shape.
export type AnalysedTurn = Pick<StoredTurn, 'id' | 'user' | 'analysisState' | 'assistant' | 'userSavedGloss' | 'userTranslation' | 'turnId'>

interface AnalysisContentProps {
  showMessages?: boolean
  replyHelp?: ReactNode
  partnerOnly?: boolean
  requestOnOpen?: boolean
  conversationId?: string
  onAsk?: (question: string) => void
  turn: AnalysedTurn
  nativeLanguageName: string
  showRomanization: boolean
  rtl: boolean
}

/// The pinned turn's full breakdown: learner words, tutor words, per-token
/// gloss lists, grammar mechanics, and the analysis Q&A thread.
export const AnalysisContent = memo(function AnalysisContent({
  turn,
  partnerOnly = false,
  showMessages = true,
  replyHelp,
  requestOnOpen = true,
  conversationId,
  onAsk,
  nativeLanguageName,
  showRomanization: _showRomanization,
  rtl: _rtl,
}: AnalysisContentProps) {
  const tr = useI18n()
  const scope = useReadingScope()
  const a = turn.assistant
  const lane = a?.help?.lanes.grammar ?? { state: a?.explanationsState ?? null }
  const eligible = a?.messageId && conversationId && !['cancelled','invalidated'].includes(lane.state ?? '')
  const request = useHelpRequest(lane, eligible ? () => requestReplyHelp(a!.messageId!, 'grammar') : undefined)
  const requested = useRef('')
  useEffect(() => {
    if (!requestOnOpen || !eligible || lane.state !== null || request.pending || request.failure || a?.mechanics.length) return
    const key = `${conversationId}:${a!.messageId}`
    if (requested.current === key) return
    requested.current = key
    request.submit(() => requestReplyHelp(a!.messageId!, 'grammar'))
  }, [requestOnOpen, eligible, lane.state, request.pending, request.failure, request.submit, conversationId, a])
  const sources = useMemo(() => scope && a ? [
    {scope:a.help?.scope ?? scope, text:a.reply, segments:a.savedGloss?.segments ?? anchoredTokenGlosses(a.reply,a.tokens)},
    ...(turn.user ? [{scope:a.help?.scope ?? scope,text:turn.user,segments:turn.userSavedGloss?.segments ?? anchoredTokenGlosses(turn.user,a.user_tokens)}] : []),
  ] : [], [scope, a, turn.user, turn.userSavedGloss])
  // A quote of an accepted message already has that message's translation.
  const quoteTranslation = (quote: string) => quote === a?.reply ? a.translation
    : quote === turn.user ? turn.userTranslation ?? a?.user_translation ?? null : null
  if (!a) return <>
    {showMessages && !partnerOnly && turn.user && <AnalysisSentence label={tr("You said")} text={turn.user} gloss={turn.userSavedGloss} translation={turn.userTranslation} />}
    <p className="center-note">{tr("No partner reply yet.")}</p>
  </>

  return (
    <MessageReadingScope scope={a.help?.scope}><SavedReadingProvider sources={sources}>
      {!a.help && ['ready', 'running', 'waiting_dependencies'].includes(a.explanationsState ?? '') && (
        <p className="sect-k pending activity-line" data-phase={a.explanationsState === 'running' ? 'running' : 'waiting'}>
          <ActivityIndicator label={tr("Working out the grammar…")} /></p>
      )}

      {showMessages && !partnerOnly && turn.user && <AnalysisSentence label={tr("You said")} text={turn.user} gloss={turn.userSavedGloss} tokens={a.user_tokens} translation={turn.userTranslation ?? a.user_translation} />}
      {showMessages && <AnalysisSentence label={tr("Partner replied")} side="bot" text={a.reply} gloss={a.savedGloss} tokens={a.tokens} translation={a.translation} />}

      {replyHelp}

      {!a.help && a.explanationsError && <ErrorDetails label={tr("Message analysis")} errorKey={a.explanationsError}>{a.explanationsError}</ErrorDetails>}
      {a.errors.length > 0 && (
        <div className="turn-errors">
          {a.errors.map((e, i) => (
            <ErrorDetails key={i} label={tr("Message analysis")} errorKey={e}>{e}</ErrorDetails>
          ))}
        </div>
      )}

      {!replyHelp && (a.help || eligible) && <>
        <HelpStatus lane={lane} pending={request.pending} failure={request.failure} label={tr('Working out the grammar…')}
          onRetry={eligible ? () => request.submit(() => requestReplyHelp(a.messageId!, 'grammar', true)) : undefined}
          onInspect={conversationId ? () => useNavigationStore.getState().inspectAi({conversationId, turnId:turn.turnId ?? null, operationKind:'reply_explanations'}) : undefined} />
        {a.help?.grammar?.cards.length === 0 && lane.state === 'succeeded' && <p>{tr('Nothing to flag in this reply.')}</p>}
      </>}
      {!partnerOnly && turn.user && <MessageSkillAnalysis messageId={turn.id} source={turn.user} conversationId={conversationId} />}
      {!replyHelp && a.mechanics.length > 0 && <>
        <p className="sect-k">{tr("What's happening")}</p>
        <ExplanationCards cards={a.mechanics} nativeLanguageName={nativeLanguageName} translationOf={quoteTranslation}
          onTerm={onAsk ? (term, card) => onAsk(`Explain [[${term}]] in this partner message: ${a.reply}. Saved explanation: ${JSON.stringify(card)}`) : undefined} />
      </>}

    </SavedReadingProvider></MessageReadingScope>
  )
})
