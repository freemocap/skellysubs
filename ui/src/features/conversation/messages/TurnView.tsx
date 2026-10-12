import { turnGraphTarget } from '../../../domain/activity/graph-target'
import { useMessageToolDefinitions } from '../../../components/reading/useMessageToolDefinitions'
import { bubbleSelection } from '../../../components/reading/bubble-selection'
import { SelectionRing } from '../../../components/reading/SelectionRing'
import { MessageSkillAnalysis } from '../reading/MessageSkillAnalysis'
import { AddToDrillButton } from '../../../components/reading/AddToDrillButton'
import { MessageTools, type MessageTool } from '../../../components/reading/MessageTools'
import { useReadAloud } from '../../../components/reading/useReadAloud'
import { MessageSpeechInspection, type MessageSpeechPlayback } from '../speech/MessageSpeechInspection'
import { CompactInspection } from '../../../components/media/CompactInspection'
import { useRecordingPlayback } from '../../../components/media/useRecordingPlayback'
import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { MessageXpButton } from '../progress/MessageXpButton'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { useI18n } from '../../../components/localization/i18n'
import { AnalysisSentence } from '../reading/AnalysisSentence'
import { anchoredTokenGlosses } from '../../../domain/reading/gloss-display'
import { EvidenceMappingNotice } from '../../../components/learning/EvidenceMappingNotice'
import { GlossAssistance } from '../reading/GlossAssistance'
import { SavedGlossText } from '../../../components/reading/SavedGlossText'
import { TargetMessage } from '../../../components/reading/TargetMessage'
import { FixCount } from './FixCount'
import { useReadingAidSpace, useReadingPreferences } from '../../../components/reading/ReadingPreferences'
import { TargetText } from '../../../components/reading/TargetText'
import { ReplyStatus } from './ReplyStatus'
import { ReceivedText } from './PendingBubble'
import { useReplyReveal } from './useReplyReveal'
import { StableTurn } from './StableTurn'
import { PendingLearner, WAITING_REPLY } from './PendingTurn'
import type { PendingMessage } from '../session/usePendingMessage'
import { retainedReplyText } from '../../../domain/conversation/activity-summary'
import { TranslationStatus, translationPending } from '../../../components/reading/TranslationStatus'
import { SkillEvidenceContext } from '../../../state/learning/useSkillEvidence'
import { PracticeContext } from '../session/PracticeContext'
import { coachFlags, coachMarks } from '../../../domain/conversation/coach-marks'
import { memo, useContext, useEffect, useRef, useState } from 'react'
import { MessageFeedback } from '../coaching/MessageFeedback'
import { MessageAssessmentRequests } from '../coaching/MessageAssessmentRequests'
import { StartPhraseButton } from './StartPhraseButton'
import { useDeferredReading } from '../reading/useDeferredReading'
import { PersonaReaction } from '../partners/PersonaReaction'
import type { GuidedTurnResult } from '../../../types'
import type { CoachControl, CoachDecision, CoachObservationView } from '../../../generated/contracts'

export interface TurnShape {
  userMessageId?: string
  assessmentState?: string | null
  assessmentError?: string | null
  feedbackState?: string | null
  replyState?: import('../../../domain/conversation/reply-state').ReplyState
  execution?: import('../../../generated/contracts').TurnView
  turnId?: string
  replacesTurnId?: string | null
  replacedBy?: string | null
  /// Revisions sent in place of this message's earlier versions.
  fixes?: number
  userSavedGloss?: import('../../../generated/contracts').WordGlossView | null
  userGlossOperationId?: string | null
  userTranslation?: string | null
  userTranslationState?: string | null
  userGlossState?: string | null
  userGlossError?: string | null

  id: number
  user: string | null
  assistant: GuidedTurnResult | null
  pendingText: string
  coach?: CoachObservationView
  feedbackContext?: string
  conversationFeedback?: import('../../../generated/contracts').ConversationFeedback
  coachDecision?: CoachDecision
  coachError?: string
  reaction?: import('../../../types').PersonaReaction
  reactionError?: string
}

export interface TurnRecording {
  result: import('../../../generated/contracts').TranscriptionInspectionResult
  rate: number
  volume: number
  enabled: boolean
  onExpand: () => void
}

export interface TurnViewProps {
  onStartPhrase?: (messageId: string, phrase: string) => Promise<void>
  /** Restrict the coach copy to its selected message without changing its controls. */
  onlySide?: 'user' | 'assistant'
  turn: TurnShape
  reviewing: boolean
  onAskCoach: (question: string) => void
  selectedSide?: 'user' | 'assistant'
  onSelectMessage?: (id: number, side: 'user' | 'assistant') => void
  focused: boolean
  ttsReady: boolean
  speaking: boolean
  speechError?: { text: string; details: unknown }
  partnerSpeech?: MessageSpeechPlayback
  rtl: boolean
  onBubbleTap: (id: number) => void
  onSpeak?: (text: string, turnId: number) => void
  /** Present only on the learner message sent from the latest recording: its
   *  audio and analysis, the playback settings, and the full dialog. */
  recording?: TurnRecording
  /// Edit this turn's message and try again — the tutor (and coach) regenerate
  /// their response from the edited text. Omitted while a turn is in flight.
  onReplyControl?: (control: 'retry' | 'resume') => Promise<void>
  onActivity?: () => void
  onAddContext?: (note: string) => Promise<void>
  onRetryHelp?: () => Promise<void>
  onRetryGloss?: (operationId: string) => Promise<void>
  onCoachControl?: (turn: TurnShape, control: CoachControl) => Promise<void>
  editDisabled?: boolean
  onEditUser?: (turn: TurnShape) => void
  /** This turn's message is open in the composer to be fixed and resent. */
  editing: boolean
  pendingEdit?: PendingMessage
}

/// Memoized: during streaming, every delta re-renders only the turn that
/// changed — not the whole conversation.
export const TurnView = memo(function TurnView(props: TurnViewProps) {
  return <StableTurn className="turn-stack" data-editing={props.editing ? '' : undefined} data-pending={props.pendingEdit?.phase}>
    <TurnContents key={props.turn.turnId ?? props.turn.id} {...props} />
  </StableTurn>
})

// Record-specific controls reset on revision; the presentation floor stays put.
function TurnContents({
  onStartPhrase,
  onlySide,
  turn,
  reviewing,
  onAskCoach,
  focused, selectedSide, onSelectMessage,
  ttsReady,
  speaking,
  speechError,
  partnerSpeech,
  rtl,
  onBubbleTap,
  onSpeak,
  recording,
  onEditUser,
  editing,
  pendingEdit,
  onCoachControl,
  editDisabled,
  onRetryGloss,
  onRetryHelp,
  onAddContext,
  onReplyControl,
  onActivity,
}: TurnViewProps) {
  const tr = useI18n()
  const messageTools = useMessageToolDefinitions()
  const replyStream = turn.execution?.nativePreview?.capture ?? null
  const arrivedPending = useRef(!turn.assistant)
  useEffect(() => { if (!turn.assistant || pendingEdit) arrivedPending.current = true }, [turn.assistant, pendingEdit])
  const replyText = turn.assistant?.reply || replyStream?.text || retainedReplyText(turn.execution) || ''
  const reveal = useReplyReveal(replyText, arrivedPending.current && (Boolean(turn.assistant) || turn.replyState?.state === 'pending'))

  const { autoTranslate, alwaysRomanize, alwaysPronunciation, supportsRomanization } = useReadingPreferences()
  // Your recording's inspector opens only when you ask for it.
  const [inspectorOpen, setInspectorOpen] = useState(false)
  const [partnerInspectorOpen, setPartnerInspectorOpen] = useState(false)
  const [recordingFailure, setRecordingFailure] = useState<unknown>(null)
  const recordingPlayback = useRecordingPlayback({ audio: recording?.result.audioBase64 || null, duration: recording?.result.inspection.duration ?? 0,
    enabled: recording?.enabled ?? false, rate: recording?.rate ?? 1, volume: recording?.volume ?? 1, onError: setRecordingFailure })
  const aidsEnabled = autoTranslate || alwaysRomanize || alwaysPronunciation
  const [userWordsOverride, setUserWordsOverride] = useState<boolean | null>(null)
  const [userSoundOverride, setUserSoundOverride] = useState<boolean | null>(null)
  const userWordsOpen = userWordsOverride ?? aidsEnabled
  const userSoundOpen = userSoundOverride ?? (userWordsOpen && (userWordsOverride === true || alwaysRomanize || alwaysPronunciation))
  const aidSpace = useReadingAidSpace(userWordsOverride === true)
  useEffect(() => { setUserWordsOverride(null) }, [autoTranslate, alwaysRomanize, alwaysPronunciation])
  const { snapshot } = useContext(SkillEvidenceContext)
  const practice = useContext(PracticeContext)
  const graphTarget = (field: string) => turnGraphTarget(practice?.chatId, turn.execution, field)
  const [userTranslationOverride, setShowUserTranslation] = useState<boolean | null>(null)
  const showUserTranslation = userTranslationOverride ?? autoTranslate
  const source = turn.user ?? ''
  // Phrases the coach flagged carry a squiggle: red for errors, yellow for partly right.
  const flags = coachFlags(turn.coach, turn.coachDecision)
  const marks = coachMarks(source, flags)
  const boundaries = [...new Set([0, source.length, ...marks.flatMap(mark => [mark.start, mark.end])])].sort((a, b) => a - b)
  const plainMarked = boundaries.slice(0, -1).map((start, index) => {
    const end = boundaries[index + 1]
    const mark = marks.find(item => item.start < end && item.end > start)
    const text = source.slice(start, end)
    return mark ? <span key={start} className="coach-flag" data-severity={mark.severity}><TargetText text={text} /></span> : <TargetText key={start} text={text} />
  })
  const assistant = turn.assistant
  const eligible = !turn.replacedBy && !['cancelled', 'invalidated'].includes(turn.execution?.state ?? '')
  // Jev can finish before coaching. Its result must not hide the error check.
  const coachingPending = eligible && !turn.coachError && (turn.feedbackState
    ? ['ready', 'running', 'waiting_dependencies'].includes(turn.feedbackState)
    : reviewing && !turn.coach)
  const userReading = useDeferredReading(eligible ? turn.userMessageId : undefined, turn.userTranslationState, turn.userGlossState)
  const partnerReading = useDeferredReading(eligible ? assistant?.messageId : undefined, assistant?.translationState, assistant?.glossState)
  const userTranslation = turn.userTranslation ?? assistant?.user_translation

  const userSegments = turn.userSavedGloss?.segments ?? (turn.user && assistant ? anchoredTokenGlosses(turn.user, assistant.user_tokens) : [])
  // Meanings set to show that may still arrive keep their line pitch reserved.
  const userAidsReserved = userWordsOpen && userSegments.length === 0 && !['failed', 'unknown', 'held', 'cancelled', 'invalidated'].includes(turn.userGlossState ?? '')
  const decorateMarks = (node: React.ReactNode, start: number, end: number) => {
    const mark = marks.find(item => item.start < end && item.end > start)
    return mark ? <span className="coach-flag" data-severity={mark.severity}>{node}</span> : node
  }

  const readAloud = useReadAloud(source)
  const inspectable = recording ?? null
  // A message you spoke plays your recording; a typed one is read aloud.
  const learnerPlay = inspectable
    ? { playing: recordingPlayback.playing, disabled: !inspectable.enabled || !inspectable.result.audioBase64, onToggle: recordingPlayback.toggle }
    : readAloud
  const learnerTools: MessageTool[] = userTranslation || translationPending(turn.userTranslationState) || (eligible && turn.userMessageId)
    ? [messageTools.translate({ ariaLabel: tr("Translate your message"),
      pressed: showUserTranslation && Boolean(userTranslation), pending: translationPending(turn.userTranslationState), onSelect: () => { setShowUserTranslation(!userTranslation || !showUserTranslation); if (!userTranslation) void userReading.request('translation') } })]
    : []
  const learnerMore = (coachTool: MessageTool): MessageTool[] => [
    messageTools.words({ pressed: userWordsOpen, pending: turn.userGlossState === 'running', disabled: !userSegments.length && !(eligible && turn.userMessageId),
      onSelect: () => { setUserWordsOverride(!userSegments.length || !userWordsOpen); if (!userSegments.length) void userReading.request('word_gloss') } }),
    ...(userSegments.some(part => supportsRomanization ? part.romanization || part.pronunciation : part.pronunciation)
      ? [messageTools.pronunciation({ pressed: userSoundOpen, onSelect: () => setUserSoundOverride(!userSoundOpen) })] : []),
    coachTool,
  ]

  return (
    <>
      {onlySide !== 'assistant' && turn.user && (
        <div className="learner-turn">
          {editing && <span className="learner-turn-editing"><ToolbarIcon name="edit" size={12} />{tr("Fixing this message")}</span>}
          {pendingEdit ? <PendingLearner message={pendingEdit} rtl={rtl} /> : <>
          <MessageFeedback graphTargets={{ feedback: graphTarget('coach_feedback'), assessment: graphTarget('skill_assessment') }} requests={turn.userMessageId && !turn.replacedBy && !['cancelled', 'invalidated'].includes(turn.execution?.state ?? '') ? <MessageAssessmentRequests messageId={turn.userMessageId} execution={turn.execution} assessmentState={turn.assessmentState} assessmentError={turn.assessmentError} feedbackState={turn.feedbackState} feedbackError={turn.coachError} assessed={Boolean(turn.conversationFeedback)} coached={Boolean(turn.coach)} /> : undefined} onAddContext={onAddContext} feedbackContext={turn.feedbackContext} conversationFeedback={turn.conversationFeedback} onRetry={onRetryHelp} analysis={<AnalysisSentence label={tr("Your message")} text={turn.user} translation={userTranslation} gloss={turn.userSavedGloss} tokens={assistant?.user_tokens} />} skills={<MessageSkillAnalysis graphTarget={graphTarget('attribution')} messageId={turn.id} source={turn.user} />} id={turn.id} text={turn.user} feedback={turn.coach} decision={turn.coachDecision} onControl={onCoachControl ? control => onCoachControl(turn, control) : undefined} error={turn.coachError} reviewing={coachingPending} onEdit={!editDisabled && onEditUser ? () => onEditUser(turn) : undefined} onAsk={onAskCoach}
            reward={<>{(turn.fixes ?? 0) > 0 && <FixCount count={turn.fixes!} />}<MessageXpButton messageId={turn.id} source={turn.user} /></>}
            bubble={coachTool => (
              <div
                {...bubbleSelection(onSelectMessage ? () => onSelectMessage(turn.id, 'user') : undefined, selectedSide === 'user', tr("Your message"))}
                className={`msg chat-message me${selectedSide === 'user' ? ' focused' : ''}${userSegments.length ? '' : ' plain'}${userAidsReserved ? ' aids-reserved' : ''}${rtl ? ' rtl' : ''}${inspectable && inspectorOpen ? ' inspecting' : ''} with-actions`}
                style={aidSpace}
              >
                <span data-phrase-source className={coachingPending ? 'message-coaching-pending' : undefined}>
                {userSegments.length > 0
                  ? <SavedGlossText graphTarget={graphTarget('learner.gloss')} revealAids={userWordsOverride === true} showAids={userWordsOpen} showSound={userSoundOverride ?? undefined} key={turn.userSavedGloss?.attemptId ?? 'tokens'} text={turn.user!} segments={userSegments} decorateSegment={decorateMarks} />
                  : plainMarked}
                </span>
                {showUserTranslation && userTranslation && <div className="trans" dir="auto" {...graphTarget('learner.translation')}>{userTranslation}</div>}
                <TranslationStatus state={turn.userTranslationState} shown={showUserTranslation && !userTranslation} />
                {userReading.status}
                <GlossAssistance assistant={{ savedGloss: turn.userSavedGloss, glossState: turn.userGlossState, glossError: turn.userGlossError, glossOperationId: turn.userGlossOperationId }} onRetryGloss={onRetryGloss} />
                <EvidenceMappingNotice snapshot={snapshot} chatId={practice?.chatId ?? null} messageId={turn.id} />
                {inspectable && inspectorOpen && <CompactInspection inspection={inspectable.result.inspection} playback={recordingPlayback}
                  enabled={inspectable.enabled} onExpand={inspectable.onExpand} />}
                <MessageTools play={learnerPlay} tools={learnerTools} more={learnerMore(coachTool)}
                  // BACKEND: only the latest recording is kept, in memory, so earlier messages have no audio to inspect (Deferred A16).
                  inspect={inspectable && { kind: 'available', open: inspectorOpen, onToggle: () => setInspectorOpen(!inspectorOpen) }}
                  actions={<>
                    {/* With errors flagged, Fix it under the bubble is the edit action. */}
                    {onEditUser && flags.length === 0 && <button type="button" className="message-tools-icon" disabled={editDisabled} aria-label={tr("Edit message")} title={tr("Edit message")}
                      onClick={event => { event.stopPropagation(); onEditUser(turn) }}><ToolbarIcon name="edit" /></button>}
                    <AddToDrillButton text={turn.user!} />
                    {eligible && turn.userMessageId && onStartPhrase && <StartPhraseButton messageId={turn.userMessageId} text={turn.user!} onStart={onStartPhrase} />}
                  </>} />
                {recordingFailure != null && <ErrorNotice as="p" error={recordingFailure}>{tr("Audio playback failed.")}</ErrorNotice>}
                {(onSelectMessage || selectedSide === 'user') && <SelectionRing />}
              </div>
            )} />
          </>}
        </div>
      )}
      {onlySide !== 'user' && assistant && !pendingEdit && (
        <div className="partner-turn">
          <span className="partner-reaction-slot">
            {turn.user && <PersonaReaction userGloss={turn.userSavedGloss} replyGloss={assistant.savedGloss} reaction={turn.reaction} error={turn.reactionError} message={turn.user} reply={assistant.reply} onEdit={!editDisabled && onEditUser ? () => onEditUser(turn) : undefined} />}
          </span>
          <TargetMessage provenance={null} graphTargets={{ text: graphTarget('text'), translation: graphTarget('reply.translation'), gloss: graphTarget('reply.gloss'), audio: turn.execution?.speech?.state === 'succeeded' ? graphTarget('audio') : undefined }}
            additionalActions={eligible && assistant.messageId && onStartPhrase && <StartPhraseButton messageId={assistant.messageId} text={assistant.reply} onStart={onStartPhrase} />}
            layout="bubble"
            text={assistant.reply}
            sourcePresentation={reveal.revealing ? <ReceivedText text={assistant.reply} visibleText={reveal.text} streaming rtl={rtl} /> : undefined}
            segments={assistant.savedGloss?.segments ?? anchoredTokenGlosses(assistant.reply, assistant.tokens)}
            segmentsKey={assistant.savedGloss ? `${assistant.savedGloss.operationId}:${assistant.savedGloss.attemptId}` : 'tokens'}
            segmentsPending={['ready', 'waiting_dependencies', 'running'].includes(assistant.glossState ?? '')}
            lookupWords={false}
            onRequestWords={eligible && assistant.messageId ? () => partnerReading.request('word_gloss') : undefined}
            onRequestTranslation={eligible && assistant.messageId ? () => partnerReading.request('translation') : undefined}
            translation={assistant.translation || null}
            translateLabel={tr("Translate partner message")}
            romanization={null}
            pronunciation={null}
            annotation={null}
            translationState={assistant.translationState}
            status={<><GlossAssistance assistant={assistant} onRetryGloss={onRetryGloss} />{partnerReading.status}</>}
            speech={ttsReady && onSpeak ? { speaking, preparing: partnerSpeech?.preparing, disabled: partnerSpeech && !partnerSpeech.enabled, onToggle: () => onSpeak(assistant.reply, turn.id), error: speechError ?? null } : null}
            analysis={{ pending: false, onOpen: () => onBubbleTap(turn.id) }}
            inspect={partnerSpeech ? { kind: 'available', open: partnerInspectorOpen, disabled: !partnerSpeech.enabled, onToggle: () => {
              setPartnerInspectorOpen(!partnerInspectorOpen)
              if (!partnerInspectorOpen && !partnerSpeech.retained && !partnerSpeech.playing && !partnerSpeech.preparing) partnerSpeech.toggle()
            } } : null}
            inspector={partnerSpeech && <MessageSpeechInspection open={partnerInspectorOpen} speech={partnerSpeech} text={assistant.reply} />}
            onSelect={onSelectMessage ? () => onSelectMessage(turn.id, 'assistant') : undefined}
            focused={selectedSide === 'assistant' || (!onSelectMessage && focused)}
            rtl={rtl}
          />
        </div>
      )}
      {onlySide !== 'user' && (pendingEdit ? (
        pendingEdit.phase === 'sending' && <ReplyStatus reply={WAITING_REPLY} rtl={rtl} />
      ) : assistant === null && (
        <ReplyStatus reply={turn.replyState} stream={replyStream} visibleText={reveal.text} retainedText={retainedReplyText(turn.execution)} rtl={rtl} onControl={onReplyControl} onActivity={onActivity} />
      ))}
    </>
  )
}
