import type { RunGuideAction } from '../../components/learning/GuideActions'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { ConversationErrorScope } from './reading/ConversationErrorScope'
import { ConversationReadingProvider } from './reading/ConversationReadingProvider'
import { interruptSpeech } from '../../platform/audio/speech'
import { ConversationDirectionSettings } from './session/ConversationDirectionSettings'
import type { CoachControl, ConversationStartConfig } from '../../generated/contracts'
import { AskCoachContext } from '../../components/learning/AskCoachButton'
import { useI18n } from '../../components/localization/i18n'
import { TranscriptionInspector } from './speech/TranscriptionInspector'
import { ConversationExport } from './session/ConversationExport'
import { PracticeDivider } from './messages/PracticeDivider'
import { LiveCoachReview } from './coaching/LiveCoachReview'
import { OpeningStatus } from './session/OpeningStatus'
import { useNavigationStore } from '../../state/navigation/navigation'
import { ConversationStart } from './session/ConversationStart'
import { PersonaProfileDialog } from './partners/PersonaProfileDialog'
import { DifficultySelect } from '../../components/controls/DifficultySelect'
import { ConversationHeader, type DifficultyPlace } from './session/ConversationHeader'
import { ConversationSettings } from './session/ConversationSettings'
import { ToolbarIcon } from '../../components/controls/ToolbarIcon'
import { NewPersonaDialog } from './partners/NewPersonaDialog'
import { PersonaPicker } from './partners/PersonaPicker'
import { useConversationDetails } from './session/useConversationDetails'
import { ComposerInput } from './composer/ComposerInput'
import { ErrorDetails } from '../../components/feedback/ErrorDetails'
import { RequestFailure } from './messages/RequestFailure'
import { PendingTurn } from './messages/PendingTurn'
import { useTurnDisplayKeys } from './messages/useTurnDisplayKeys'
import { usePendingMessage } from './session/usePendingMessage'
import { ReadingPreferencesProvider } from '../../components/reading/ReadingPreferences'
import { configureRewardSounds, stopRewardSounds } from '../../platform/audio/reward-sounds'
import { RewardPresentationProvider } from './progress/RewardPresentation'
import { AiStatus } from './composer/AiStatus'
import { TurnReplyHelp } from './composer/TurnReplyHelp'
import { useSkillNavigationStore } from '../../state/navigation/skill-navigation'
import { aiTraySlot } from '../../state/navigation/ai-tray'
import { useCallback, useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { openOverlay } from '../../domain/input/back'
import { createContact as createContactRequest, executeAction, readWorkspace, nativeError } from '../../platform/ipc/workspace'
import type { Settings } from '../../types'
import type { PersonaDetails } from '../../generated/contracts'
import { unreportedInput, type InputEvidence } from '../../domain/learning/evidence/skills'
import { PracticeContext } from './session/PracticeContext'
import { SkillRewards } from './progress/SkillRewards'
import { XpChip, type ConversationProgressSection } from './progress/XpChip'
import { translatedName } from '../../domain/localization'
import { conversationProgressSectionId } from './progress/ConversationProgress'
import { isTauri, languageFor } from '../../platform/ipc/tauri'
import { languageLabel } from '../../domain/language/language-label'
import { personaName } from './partners/personaLimits'
import { useSettingsStore } from '../../state/settings/settings'
import { useSessionStore } from '../../state/session/session'
// Where the narrow-window layout puts the learner: the shell's navigation state
// decides it, so the type lives with that state.
import type { MobileLocation } from '../../state/navigation/navigation'
import { useMessageSpeech } from './speech/useMessageSpeech'
import { comboFromEvent } from '../../domain/input/keyboard'
import { LiveRecording } from '../../components/media/LiveRecording'
import { useRecorderLayout } from '../../components/media/useRecorderLayout'
import { ResizeHandle, useStoredSize } from '../../components/layout/ResizeHandle'
import { useUiDirection } from '../../components/localization/useUiDirection'
import { MicrophoneSelector } from '../../components/media/MicrophoneSelector'
import { MicrophoneCheck } from '../../components/media/MicrophoneCheck'
import { EditFeedback } from './coaching/EditFeedback'
import { TurnView, type TurnShape } from './messages/TurnView'
import { DetailDialog } from '../../components/dialogs/DetailDialog'
import { AnalysisContent } from './reading/AnalysisContent'
import { PartnerMessageAnalysis } from './reading/PartnerMessageAnalysis'
import { CoachAnalysisPanel } from './coaching/CoachAnalysisPanel'
import { logInfo, logWarn } from '../../platform/diagnostics/log'
import { ChatHistory } from './session/ChatHistory'
import { latestAnswered } from '../../domain/conversation/turns'
import { revisionChanges } from '../../domain/conversation/revision-changes'
import { useConversation } from './session/useConversation'
import { useConversationScroll } from './messages/useConversationScroll'
import { useMicRecorder } from '../../platform/audio/useMicRecorder'
import { usePersistentToggle } from '../../components/persistence/usePersistentToggle'
import { useWidthTier } from '../../components/layout/useWidthTier'
import { reportFault } from '../../platform/diagnostics/faults'
import { needsProviderSetup } from '../../domain/access/providers'

/// Number of conversation stripe hues: the .chat[data-stripe] rules in
/// conversation/message styles and the --chat-stripe-* tokens in tokens.css.
const CHAT_STRIPES = 5

export default function ConversationPage({
  active,
  nativePicker,
  mobileSurface,
  historyOpen = false,
  onHistoryOpenChange,
  onOpenSettings,
  onNewChatReady,
  onSkillStartReady,
  onGuideActionReady,
}: {
  active: boolean
  /// The explanation-language picker, kept in the conversation settings panel.
  nativePicker: ReactNode
  mobileSurface: MobileLocation
  historyOpen?: boolean
  onHistoryOpenChange?: (open: boolean) => void
  /// Open the Settings modal. It lands on the AI provider section, which is
  /// where every "configure a provider" failure is asking the learner to go.
  onNewChatReady?: (action: (() => void) | null) => void
  onGuideActionReady?: (action: RunGuideAction | null) => void
  onSkillStartReady?: (action: ((language: string, variety: string, skillId: string, subskillId?: string | null) => Promise<void>) | null) => void
  onOpenSettings?: () => void
}) {
  const tr = useI18n()
  const workspace = useRef<HTMLDivElement>(null)
  const composer = useRef<HTMLDivElement>(null)
  const stopSpeechRef = useRef<() => void>(() => {})
  const selectionVersion = useSkillNavigationStore((state) => state.sequence)
  const skillSelection = useSkillNavigationStore((state) => state.selected)
  const selectSkill = useSkillNavigationStore((state) => state.select)
  const [selectedMessage, setSelectedMessage] = useState<{ id: number; side: 'user' | 'assistant' } | null>(null)
  const [pinnedId, setPinnedId] = useState<number | null>(null)
  const [sending, setSending] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const errorRef = useRef(error)
  errorRef.current = error
  const [input, setInputState] = useState('')
  const inputRevision = useRef(0)
  const setInput = useCallback((value: string | ((previous: string) => string)) => { inputRevision.current += 1; setInputState(value) }, [])
  const inputEvidence = useRef<InputEvidence>(unreportedInput())
  const draftRecording = useRef<string | null>(null)
  const [sentRecording, setSentRecording] = useState<{ recordingId: string; turnId: string } | null>(null)
  // A repair retains its source and explicitly confirms removal of dependent turns.
  const [editingTurnId, setEditingTurnId] = useState<number | null>(null)
  const [acceptedEditSource, setAcceptedEditSource] = useState<string | null>(null)
  const [editRevision, setEditRevision] = useState<number | null>(null)
  const [revisionConfirmation, setRevisionConfirmation] = useState<{ text: string; input: InputEvidence; revision: number; exchangeCount: number } | null>(null)
  const connection = useSessionStore((state) => state.connection)
  const signingIn = useSessionStore((state) => state.signingIn)
  const startHostedSignIn = useSessionStore((state) => state.startHostedSignIn)
  const settings = useSettingsStore((state) => state.settings)
  // Bumped when a settings **write** lands. Switching conversations reloads the
  // record without bumping it: a different scope is not a settings change.
  const settingsVersion = useSettingsStore((state) => state.revision)
  useEffect(() => {
    if (settings) configureRewardSounds(settings.xp_effects === false ? 'no' : settings.reward_sounds, settings.auto_speak)
    if (!active) stopRewardSounds()
  }, [settings?.xp_effects, settings?.reward_sounds, settings?.auto_speak, active])
  useEffect(() => () => stopRewardSounds(), [])
  const [panelTab, setPanelTab] = useState<'coaching' | 'progress'>('coaching')
  const [coachDraft, setCoachDraft] = useState('')
  const [coachRevealRequest, setCoachRevealRequest] = useState(0)
  const mode = useNavigationStore(state => state.mode)
  const [reviewing, setReviewing] = useState<Set<number>>(new Set())
  const consumeCoachDraft = useCallback(() => setCoachDraft(''), [])
  const { open: breakOpen, toggle: toggleBreak } = usePersistentToggle('skellyspeak_break', true)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [difficultyPlace, setDifficultyPlace] = useState<DifficultyPlace>('header')
  // Panel reload counter: bumped when the coach thread is reset externally.
  const [threadReload, setThreadReload] = useState(0)


  const streamRef = useRef<HTMLDivElement | null>(null)
  const breakRef = useRef<HTMLDivElement | null>(null)

  const settingsRef = useRef<Settings | null>(null)
  settingsRef.current = settings
  const sendRef = useRef<(text: string) => Promise<void>>(async () => {})
  const toggleMicRef = useRef<() => void>(() => {})
  const toggleBreakRef = useRef<() => void>(() => {})

  const setHistoryOpen = useCallback(
    (open: boolean) => onHistoryOpenChange?.(open),
    [onHistoryOpenChange]
  )

  /// Everything tied to the conversation leaving the screen. The turns
  /// themselves are set by whoever swapped them.
  const resetView = useCallback(() => {
    setPinnedId(null)
    setSelectedMessage(null)
    setCoachDraft('')
    setCoachRevealRequest(0)
    setReviewing(new Set())
    setError(null)
    setSending(false)
    setEditingTurnId(null)
    setAcceptedEditSource(null)
    setRevisionConfirmation(null)
    stopSpeechRef.current()
    setThreadReload((v) => v + 1)
  }, [])

  const {
    turns,
    chats,
    currentChatId,
    openChat,
    startNew: startNewConversation,
    startFromPhrase,
    startFromSkill,
    runGuideAction,
    removeChat,
    sendMessage,
    pendingReply,
    snapshotRevision,
    readError, retryRead, olderError, loadingOlder, loadOlder,
    snapshot,
  } = useConversation({
    settings,
    setHistoryOpen,
    resetView,
  })

  useEffect(() => {
    onSkillStartReady?.(settings && !sending ? startFromSkill : null)
    return () => onSkillStartReady?.(null)
  }, [onSkillStartReady, settings, sending, startFromSkill])

  // A sent message holds its place until native storage has its turn.
  const pending = usePendingMessage(turns)
  const pendingMessage = pending.message
  const releasePending = pending.release
  useEffect(() => releasePending(), [currentChatId, releasePending])
  const selectedChatRef = useRef(currentChatId)
  selectedChatRef.current = currentChatId
  const details = useConversationDetails(currentChatId, snapshotRevision)
  const [creatingConversation, setCreatingConversation] = useState(false)
  const creatingContactConversation = useRef(false)
  const [contactError, setContactError] = useState<string | null>(null)
  // A contact is the container; its entry shows the persona it speaks through.
  const contactChoices = (details.directory?.contacts ?? []).flatMap(contact => {
    const persona = details.directory?.personas.find(item => item.id === contact.personaId)
    return !contact.archived && persona && persona.languageId === settings?.target_language
      ? [{ id: contact.id, name: personaName(persona.details), symbol: persona.details.vibe[0] }]
      : []
  })
  const activeContactId = details.contact?.id ?? ''
  const contactChats = chats.filter(chat => details.directory?.conversations.some(item => item.id === chat.id && item.contactId === activeContactId))
  async function createContactConversation(contactId: string) {
    if (creatingContactConversation.current) return
    creatingContactConversation.current = true; setCreatingConversation(true); setContactError(null)
    try { await details.beforeSend(); await openChat(await details.createConversation(contactId)) }
    catch (reason) { setContactError(nativeError(reason)) }
    finally { creatingContactConversation.current = false; setCreatingConversation(false) }
  }
  // Choosing a persona means choosing the contact it speaks through: open that
  // contact's most recent conversation, or give them one if they have none.
  async function chooseContact(contactId: string) {
    if (contactId === activeContactId || creatingContactConversation.current) return
    const conversations = (details.directory?.conversations ?? []).filter(item => item.contactId === contactId && !item.archived)
    const recent = conversations.sort((a, b) => b.lastUsed - a.lastUsed)[0]
    if (recent) {
      creatingContactConversation.current = true; setCreatingConversation(true); setContactError(null)
      try { await details.beforeSend(); await openChat(recent.id) }
      catch (reason) { setContactError(nativeError(reason)) }
      finally { creatingContactConversation.current = false; setCreatingConversation(false) }
      return
    }
    await createContactConversation(contactId)
  }
  async function createPersona(next: PersonaDetails) {
    if (!settings || creatingContactConversation.current) return
    creatingContactConversation.current = true; setCreatingConversation(true); setContactError(null)
    try {
      await details.beforeSend()
      const conversation = await createContactRequest(settings.target_language, next)
      setNewPersonaOpen(false)
      await openChat(conversation)
    } catch (reason) { setContactError(nativeError(reason)); throw reason }
    finally { creatingContactConversation.current = false; setCreatingConversation(false) }
  }
  const [editingPersonaId, setEditingPersonaId] = useState<string | null>(null)
  const editingPersona = details.directory?.personas.find(item => item.id === editingPersonaId)
  const [newPersonaOpen, setNewPersonaOpen] = useState(false)
  // The tab shows the persona being talked to. Creating another belongs to the
  // header picker, so nothing here can overwrite this one by accident.
  function targetLanguageLabel(id: string) { return details.directory?.languages.find(item => item.id === id)?.name ?? id }



  useEffect(() => {
    logInfo('[guided] page mounted, isTauri =', isTauri)
    // A settings **save** supersedes the missing-provider banner. The revision is
    // zero until a write lands, so a first read never clears a real failure.
    if (settingsVersion > 0 && errorRef.current && needsProviderSetup(errorRef.current)) {
      errorRef.current = null
      setError(null)
    }
  }, [settingsVersion, currentChatId])

  // The record describes the active conversation's scope, so switching chats
  // re-reads it. This does not bump the revision: nothing was written.
  useEffect(() => {
    void useSettingsStore.getState().load().catch((error: unknown) => reportFault('Loading settings', error))
  }, [currentChatId])

  const streamTail = pendingMessage ? `sending:${pendingMessage.key}:${pendingMessage.phase}`
    : (() => { const last = turns.filter(turn => !turn.replacedBy).at(-1); return last ? `${last.id}:${last.assistant ? 'reply' : 'pending'}` : null })()
  const streamScroll = useConversationScroll(streamRef, currentChatId, snapshot?.messages[0]?.sequence, turns, streamTail, active)
  // New messages go to the tail; revisions stay at the message being edited.
  const jumpToLatest = useRef(streamScroll.jumpToLatest)
  jumpToLatest.current = streamScroll.jumpToLatest
  const pendingKey = pendingMessage?.key
  useEffect(() => { if (pendingKey && !pendingMessage?.editing) jumpToLatest.current() }, [pendingKey])

  const tier = useWidthTier()
  const isMobile = tier !== 'full'
  function openCoach(id?: number) {
    if (id !== undefined) setPinnedId(id)
    setPanelTab('coaching')
    if (!breakOpen) toggleBreak()
    if (isMobile) useNavigationStore.getState().openConversation('panel')
    // Phones: opening moves no focus, so nothing scrolls to reveal the field and
    // no keyboard opens over the popover. The learner taps the field to ask.
    if (!isMobile) requestAnimationFrame(() => breakRef.current?.querySelector<HTMLTextAreaElement>('.coach-input')?.focus({ preventScroll: true }))
  }
  /** A header counter: the learning panel's Progress tab, scrolled to the section that leads with the counter's number. */
  function openProgress(section: ConversationProgressSection) {
    setPanelTab('progress')
    if (!breakOpen) toggleBreak()
    if (isMobile) useNavigationStore.getState().openConversation('panel')
    requestAnimationFrame(() => document.getElementById(conversationProgressSectionId(section))?.scrollIntoView({ block: 'start', behavior: 'smooth' }))
  }
  const [exportOpen, setExportOpen] = useState(false)
  const [inspectionOpen, setInspectionOpen] = useState(false)
  useEffect(() => setInspectionOpen(false), [currentChatId, active])
  useEffect(() => { draftRecording.current = null; setSentRecording(null) }, [currentChatId, active])
  const [analysisOpen, setAnalysisOpen] = useState(false)
  const guideCoachOpen = useRef(openCoach)
  guideCoachOpen.current = openCoach
  useEffect(() => {
    onGuideActionReady?.(async action => {
      await runGuideAction(action)
      if (action.kind === 'coach') {
        setCoachRevealRequest(request => request + 1)
        guideCoachOpen.current()
      }
    })
    return () => onGuideActionReady?.(null)
  }, [onGuideActionReady, runGuideAction])
  function askCoach(question: string) {
    setAnalysisOpen(false)
    setCoachDraft(question)
    openCoach()
  }
  useEffect(() => { setAnalysisOpen(false) }, [currentChatId, settingsVersion])
  const readingSettingsBusy = useNavigationStore(state => state.settingsBusy)
  const readingOverlay = useNavigationStore(state => state.overlay)
  const readingQuestion = useNavigationStore(state => state.readingQuestion)
  useEffect(() => {
    if (!readingQuestion || !active || !currentChatId || readingSettingsBusy || readingOverlay) return
    askCoach(readingQuestion)
    useNavigationStore.getState().draftReadingQuestion(null)
  }, [readingQuestion, active, currentChatId, readingSettingsBusy, readingOverlay])


  const onBubbleTap = useCallback(
    (id: number) => {
      setPinnedId(id)
      setAnalysisOpen(true)
    },
    []
  )
  const [startDraft, setStartDraft] = useState<{ id: string; value: ConversationStartConfig } | null>(null)
  // The header's partner menu, which the start card's Change partner also opens.
  const [partnerMenuOpen, setPartnerMenuOpen] = useState(false)
  const startConfiguration = startDraft?.id === currentChatId ? startDraft.value : details.conversation ? {
    difficulty: details.conversation.settings.difficulty, varietyId: details.conversation.settings.varietyId, direction: details.conversation.settings.direction,
  } : null
  const acceptingSend = useRef(false)
  useEffect(() => setSending(pendingReply || acceptedEditSource !== null), [pendingReply, snapshotRevision, acceptedEditSource])
  useEffect(() => {
    if (acceptedEditSource && turns.some(turn => turn.replacesTurnId === acceptedEditSource)) {
      setAcceptedEditSource(null)
      setEditingTurnId(null)
    }
  }, [acceptedEditSource, turns])
  /** Sends `text` as a new message, or as a revision of `editing`. It holds its
   * place from the first moment; a new message's text moves out of the draft
   * into its bubble, and if it is rejected the bubble keeps it with Retry. */
  async function submitText(text: string, provenance: InputEvidence, revision?: number,
    recording: string | null = provenance.modality === 'speech_transcript' ? draftRecording.current : null, editing: number | null = editingTurnId, fromDraft = true) {
    if (acceptingSend.current) return
    acceptingSend.current = true
    const submittedChatId = currentChatId
    const submittedRecording = recording
    const edited = editing !== null ? turns.find(item => item.id === editing) : undefined
    // A recording's text fills the bubble that has held its place since it stopped.
    const key = submittedRecording && pendingMessage?.phase === 'transcribing' ? pendingMessage.key : submittedRecording ?? crypto.randomUUID()
    pending.hold({ key, editing: edited?.turnId ? { id: edited.id, turnId: edited.turnId } : null, text, phase: 'sending' })
    if (editing === null && fromDraft) {
      setInput('')
      inputEvidence.current = unreportedInput()
    }
    const submittedDraftRevision = inputRevision.current
    let acceptedTurnId: string | null = null
    let editedSource: string | null = null
    setSending(true)
    setError(null)
    try {
      await details.beforeSend()
      if (selectedChatRef.current !== submittedChatId) return
      if (editing !== null) {
        const turn = edited
        if (!turn?.turnId || !snapshot || revision === undefined) throw new Error('Revision source is unavailable. Reopen the message to edit it.')
        acceptedTurnId = (await executeAction(snapshot, { kind: 'reviseTurn', conversationId: snapshot.conversationId, turnId: turn.turnId, text, input: provenance, expectedRevision: revision })).entityId
        editedSource = turn.turnId
      } else if (snapshot && !snapshot.opening && turns.length === 0) {
        if (!startConfiguration) throw new Error('Conversation settings are unavailable.')
        const latest = await readWorkspace()
        if (selectedChatRef.current !== submittedChatId) return
        acceptedTurnId = (await executeAction(latest, { kind: 'startConversation', conversationId: snapshot.conversationId, configuration: startConfiguration, message: text, input: provenance, expectedRevision: latest.revision })).entityId
      } else acceptedTurnId = (await sendMessage(text, currentChatId, provenance)).entityId
      if (selectedChatRef.current !== submittedChatId) return
      if (submittedRecording && acceptedTurnId) {
        setSentRecording({ recordingId: submittedRecording, turnId: acceptedTurnId })
        if (draftRecording.current === submittedRecording) draftRecording.current = null
      }
      if (fromDraft && inputRevision.current === submittedDraftRevision) {
        setInput('')
        inputEvidence.current = unreportedInput()
      }
      if (editedSource) setAcceptedEditSource(editedSource)
      setRevisionConfirmation(null)
    } catch (reason) {
      if (selectedChatRef.current !== submittedChatId) return
      if (editing !== null) {
        // An edit stays in the composer, so the learner can adjust, resend or cancel it.
        pending.release(key)
        setError(nativeError(reason))
        if (inputRevision.current === submittedDraftRevision) setInput(text)
      } else pending.fail(key, nativeError(reason), () => submitRef.current(text, provenance, undefined, submittedRecording, null, false))
      setRevisionConfirmation(null)
      setEditRevision(null)
      setSending(false)
    } finally { acceptingSend.current = false }
  }
  const submitRef = useRef(submitText)
  submitRef.current = submitText

  async function startConversation(configuration: ConversationStartConfig) {
    if (acceptingSend.current || sending) throw new Error('A conversation action is already pending.')
    if (!snapshot || snapshot.conversationId !== currentChatId) throw new Error('The conversation is not ready.')
    const reviewed = snapshot
    const owner = currentChatId
    acceptingSend.current = true
    setSending(true)
    try {
      await details.beforeSend()
      if (selectedChatRef.current !== owner) throw new Error('The conversation changed before starting.')
      const latest = await readWorkspace()
      if (selectedChatRef.current !== owner) throw new Error('The conversation changed before starting.')
      await executeAction(latest, { kind: 'startConversation', conversationId: reviewed.conversationId, configuration, message: null, input: null, expectedRevision: latest.revision })
    } catch (reason) {
      if (selectedChatRef.current === owner) setSending(false)
      throw reason
    } finally { acceptingSend.current = false }
  }

  async function send(text: string) {
    const message = text.trim()
    if (!message || acceptingSend.current || (sending && editingTurnId === null)) return
    const provenance = { ...inputEvidence.current, revision: editingTurnId !== null }
    stopSpeechRef.current()
    if (editingTurnId !== null) {
      const turn = turns.find(item => item.id === editingTurnId)
      const scope = snapshot?.revisionSuffixCounts.find(item => item.turnId === turn?.turnId)
      if (!scope || !snapshot) { setError('Revision source is unavailable. Reopen the message to edit it.'); return }
      // A failed admission requires a fresh native preview; the draft stays intact.
      const revision = editRevision ?? snapshot.revision
      if (scope.exchangeCount) {
        setRevisionConfirmation({ text: message, input: provenance, revision, exchangeCount: scope.exchangeCount })
        return
      }
      await submitText(message, provenance, revision)
    } else await submitText(message, provenance)
  }
  sendRef.current = send

  const cancelEdit = useCallback(() => {
    inputEvidence.current = unreportedInput()
    setEditingTurnId(null)
    setInput('')
  }, [])
  toggleBreakRef.current = toggleBreak
  // Configurable keyboard shortcuts. Modifier combos work while typing;
  // the handler ignores repeat events and the shortcut-capture inputs.
  useEffect(() => {
    const shortcuts = settings?.shortcuts
    if (!shortcuts) return
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return
      const target = e.target as HTMLElement | null
      if (target?.closest?.('[data-shortcut-capture]')) return
      const combo = comboFromEvent(e)
      const inField = /^(input|textarea|select)$/i.test(target?.tagName ?? '')
      const hasMod = e.ctrlKey || e.altKey || e.metaKey
      if (!hasMod && inField) return
      if (combo === shortcuts.mic) {
        e.preventDefault()
        toggleMicRef.current()
      } else if (combo === shortcuts.panel) {
        e.preventDefault()
        toggleBreakRef.current()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [settings?.shortcuts])

  const activeTurns = turns.filter(turn => !turn.replacedBy)
  const turnDisplayKeys = useTurnDisplayKeys(currentChatId, turns)
  const editingTurn = turns.find((turn) => turn.id === editingTurnId)
  /** Coach controls act on the turn's decision as it is when they run. */
  const coachControl = async (turnId: string, control: CoachControl): Promise<void> => {
    if (!snapshot) throw new Error('Coaching is unavailable.')
    await executeAction(snapshot, { kind: 'coachControl', turnId, control })
  }
  const editChanges = editingTurn?.user != null ? revisionChanges(editingTurn.user, input) : 0
  const explicitlySelectedTurn = activeTurns.find(turn => turn.id === selectedMessage?.id)
  const selectedTurn = explicitlySelectedTurn ?? activeTurns.at(-1)
  const selectedSide = explicitlySelectedTurn ? selectedMessage!.side : selectedTurn?.assistant ? 'assistant' : 'user'
  useEffect(() => { if (selectedMessage && !explicitlySelectedTurn) setSelectedMessage(null) }, [selectedMessage, explicitlySelectedTurn])
  const selectMessage = (id: number, side: 'user' | 'assistant'): void => {
    setSelectedMessage({ id, side })
    setPanelTab('coaching')
    if (!breakOpen) toggleBreak()
    if (isMobile) useNavigationStore.getState().openConversation('panel')
  }
  const coachedTurn = selectedTurn ?? activeTurns.find(turn => turn.id === pinnedId) ?? activeTurns.at(-1)
  const editBlocked = acceptingSend.current || acceptedEditSource !== null
  const canEdit = (turn: TurnShape): boolean => Boolean(turn.turnId) && turn.user !== null
  /** Puts a sent message in the composer; sending replaces it. */
  const startEdit = (selected: TurnShape): void => {
    setEditingTurnId(selected.id)
    setEditRevision(snapshot?.revision ?? null)
    setInput(selected.user ?? '')
    setError(null)
    inputEvidence.current = { ...unreportedInput(), recording_ids: snapshot?.turns.find(turn => turn.id === selected.turnId)?.recordingIds }
    // On phones, leave the microphone visible until the learner taps to type.
    if (!isMobile) composer.current?.querySelector('textarea')?.focus()
  }
  const latestAssistantId = latestAnswered(activeTurns)?.id ?? null

  // Romanization shows for targets whose script needs it (Arabic → ALA-LC).
  const showRomanization =
    settings != null && languageFor(settings.target_language, settings.target_variety)?.romanization != null

  // RTL targets render message lines right-to-left.
  const rtl =
    settings != null && languageFor(settings.target_language, settings.target_variety)?.direction === 'rtl'

  useEffect(() => {
    onNewChatReady?.(settings && !sending ? () => { void startNewConversation() } : null)
    return () => onNewChatReady?.(null)
  }, [onNewChatReady, settings, sending, startNewConversation])

  const savingReading = useSettingsStore((state) => state.savingPreference)
  const toggleSetting = useSettingsStore((state) => state.setPreference)

  const targetLanguage = settings ? languageFor(settings.target_language, settings.target_variety) : null
  const nativeLanguage = settings ? languageFor(settings.native_language, settings.native_variety) : null
  const targetLanguageName = targetLanguage ? languageLabel(targetLanguage, tr.locale) : ''
  const nativeLanguageName = nativeLanguage ? languageLabel(nativeLanguage, tr.locale) : ''
  const romanized = Boolean(targetLanguage?.romanization)
  const pinnedTurn = activeTurns.find(t => t.id === (pinnedId ?? latestAssistantId) && t.assistant) ?? null


  const mic = useMicRecorder({
    owner: active && currentChatId ? { kind: 'conversation', id: currentChatId } : null,
    onTranscribe: (text, result) => {
      if (text) {
        draftRecording.current = result.inspection.recordingId
        inputEvidence.current.modality = 'speech_transcript'
        const autoSend = settingsRef.current?.auto_send && (!sending || editingTurnId !== null)
        inputEvidence.current.recording_ids = autoSend
          ? [result.inspection.recordingId]
          : [...new Set([...(inputEvidence.current.recording_ids ?? []), result.inspection.recordingId])]
        if (autoSend) {
          logInfo('[mic] auto-send enabled — sending transcription')
          void sendRef.current(text)
        } else {
          setInput((prev) => (prev ? `${prev} ${text}` : text))
        }
      } else logWarn('[mic] transcription was empty (silence?)')
    },
  })
  // With Auto-send a recording is sent the moment it stops, so its message holds
  // its place while it is transcribed. A failed take keeps it, with Retry sending
  // the same audio again; a take whose text went to the draft, or that held no
  // speech, lets it go.
  const take = mic.pendingRecordings.at(-1) ?? null
  const takeKey = take ? `${take.recordingId}:${take.state}` : null
  const autoSends = Boolean(settings?.auto_send) && (!sending || editingTurnId !== null)
  const followedTake = useRef<string | null>(null)
  useEffect(() => {
    if (!take || followedTake.current === takeKey) return
    followedTake.current = takeKey
    if (take.state === 'processing') {
      if (autoSends) pending.hold({ key: take.recordingId, editing: editingTurn?.turnId ? { id: editingTurn.id, turnId: editingTurn.turnId } : null, text: null, phase: 'transcribing' })
    } else if (take.state === 'failed') pending.fail(take.recordingId, nativeError(take.failure), () => mic.retry(take.recordingId))
    else pending.release(take.recordingId, 'transcribing')
  }, [takeKey])
  const takeFailed = take?.state === 'failed' ? take : null
  const speech = useMessageSpeech(snapshot, currentChatId, Boolean(settings?.auto_speak), active, settings?.tts_rate ?? 1, (settings?.master_volume ?? 100) * (settings?.voice_volume ?? 100) / 10000, mic.recording || mic.transcribing)
  stopSpeechRef.current = () => { speech.stop(); interruptSpeech() }
  const toggleMic = () => { stopSpeechRef.current(); void mic.toggleMic() }
  // Tap or Hold: in Hold the microphone records while the pad is held down.
  const [voiceMode, setVoiceMode] = useState<'tap' | 'hold'>('tap')
  const holdSession = useRef<{ ready: Promise<void>; released: boolean } | null>(null)
  const holdStart = () => {
    if (holdSession.current || mic.recording || mic.transcribing) return
    stopSpeechRef.current()
    holdSession.current = { ready: mic.toggleMic(), released: false }
  }
  const holdEnd = () => {
    const session = holdSession.current
    if (!session || session.released) return
    session.released = true
    const stopped = mic.stopMic()
    void Promise.all([session.ready, stopped]).then(() => {
      if (holdSession.current === session) holdSession.current = null
    })
  }
  toggleMicRef.current = toggleMic
  // The recording panel keeps the height the learner drags it to; the stream takes the rest.
  const [voiceHeight, setVoiceHeight] = useStoredSize('chat-voice')
  // Opened reply help keeps a dragged height too, so the learner decides how much it takes.
  const [replyHelpHeight, setReplyHelpHeight] = useStoredSize('reply-help')
  const uiDirection = useUiDirection()
  const recorder = useRecorderLayout('chat', uiDirection)




  // Where the docked coach panel does not fit (compact and narrow), the coach
  // covers the conversation as a sheet: Back, Escape, the scrim and its close
  // control fold it away.
  const coachCovers = isMobile && mobileSurface === 'panel'
  const closeCoach = useCallback(() => useNavigationStore.getState().openConversation('chat'), [])
  useEffect(() => {
    if (!active || !coachCovers) return
    const release = openOverlay(closeCoach)
    const onKey = (event: KeyboardEvent) => { if (event.key === 'Escape') closeCoach() }
    window.addEventListener('keydown', onKey)
    return () => { release(); window.removeEventListener('keydown', onKey) }
  }, [active, coachCovers, closeCoach])
  // The coach slides in only after the learner opens it, not on first open of the page.
  const [surfaceSwitched, setSurfaceSwitched] = useState(false)
  const shownSurface = useRef(mobileSurface)
  useEffect(() => {
    if (shownSurface.current === mobileSurface) return
    shownSurface.current = mobileSurface
    setSurfaceSwitched(true)
  }, [mobileSurface])
  const latestTurn = activeTurns.at(-1)
  // Bind the recording to the accepted send receipt, never to matching text.
  const recordingTurnId = sentRecording?.recordingId === mic.lastTranscription?.inspection.recordingId
    ? activeTurns.find(turn => turn.turnId === sentRecording?.turnId)?.id ?? null : null
  const inspectLatest = () => useNavigationStore.getState().inspectAi({ conversationId: snapshot?.conversationId ?? null, turnId: latestTurn?.turnId ?? null, operationKind: null })
  const replyHelp = (turn = latestTurn, inline = false) => (
    <ConversationErrorScope conversationId={snapshot?.conversationId} turn={turn?.execution}>
      <TurnReplyHelp inline={inline} turn={turn} conversationId={snapshot?.conversationId} onAsk={askCoach} busy={sending}
        onUse={(text, source) => {
          inputEvidence.current = { ...inputEvidence.current, [source]: true }
          setInput(previous => previous.trim() ? `${previous.trimEnd()} ${text}` : text)
          composer.current?.querySelector<HTMLTextAreaElement>('.field')?.focus()
        }} />
    </ConversationErrorScope>
  )
  // A new conversation's voice panel suggests the authored greeting to say.
  const greeting = turns.length === 0 && snapshot && !snapshot.opening ? snapshot.starterGreeting : null
  const greetingPrompt = greeting ? tr.rich('Say {greeting} to start', { greeting: <>
    <b className="target-word" lang={targetLanguage?.languageTag}><bdi dir={rtl ? 'rtl' : 'ltr'}>{greeting.text}</bdi></b>
    {greeting.romanized && <> (<bdi dir="ltr">{greeting.romanized}</bdi>)</>}
  </> }) : undefined
  const composerActivity = (
    <div className="composer-activity">
      <AiStatus transcribing={mic.transcribing} scheduling={sending && !pendingReply} turns={snapshot?.turns ?? []}
        synthesizing={speech.phase === 'preparing'} buffering={speech.phase === 'buffering'} latest={latestTurn?.assistant ? latestTurn.execution ?? undefined : undefined} onInspectLatest={inspectLatest}
        beside={null} />
    </div>
  )
  const chatComposer = (
        <div className="composer" data-editing={editingTurnId !== null ? "" : undefined} ref={composer}
          data-voice-sized={voiceHeight === null ? undefined : ""} style={voiceHeight === null ? undefined : { '--chat-voice-height': `${Math.round(voiceHeight)}px` } as CSSProperties}>
          {editingTurnId !== null && (
            <div className="edit-banner">
              <ToolbarIcon name="edit" size={15} />
              <strong>{acceptedEditSource ? tr("Edit saved — updating conversation…") : tr("Fix your message")}</strong>
              {!acceptedEditSource && editingTurn?.user != null && <span className="edit-banner-changes">{editChanges === 0
                ? tr("Sending replaces your message") : tr("Changes from your original", { count: editChanges })}</span>}
              <button type="button" disabled={acceptedEditSource !== null} onClick={cancelEdit}>
                {tr("Cancel")}</button>
            </div>
          )}
          {editingTurn && !acceptedEditSource && <EditFeedback onControl={editingTurn.turnId ? control => coachControl(editingTurn.turnId!, control) : undefined} key={editingTurn.id} decision={editingTurn.coachDecision} feedback={editingTurn.coach} error={editingTurn.coachError} reviewing={reviewing.has(editingTurn.id)} />}
          {!isMobile && composerActivity}
          {/* A failed take offers Retry, which sends its audio again; its message's
              bubble shows it when Auto-send had sent it. */}
          {mic.failure != null && !(takeFailed && pendingMessage?.key === takeFailed.recordingId) && <ErrorNotice error={mic.failure}
            onRetry={takeFailed ? () => mic.retry(takeFailed.recordingId) : undefined}>
            <p>{nativeError(mic.failure)}</p>
            {!takeFailed && <button type="button" className="btn" disabled={mic.recording || mic.transcribing} onClick={toggleMic}>{tr('Record again')}</button>}
          </ErrorNotice>}
          {/* Stacked, one row above the answer holds reply help, the status line and
              the coach, instead of a row each. */}
          {isMobile && <div className="composer-assist" data-help-sized={replyHelpHeight === null ? undefined : ''}
            style={replyHelpHeight === null ? undefined : { '--reply-help-height': `${Math.round(replyHelpHeight)}px` } as CSSProperties}>
            {/* Opened reply help's top edge; the handle hides while help is folded. */}
            <ResizeHandle className="reply-help-resize" label={tr("Resize the reply help panel")} axis="y" grow={-1} size={replyHelpHeight}
              min={120} max={Math.round(window.innerHeight * 0.8)} onResize={setReplyHelpHeight}
              measure={() => composer.current?.querySelector('.composer-assist .reply-help')?.getBoundingClientRect().height ?? 0} />
            {/* The AI View's tray rises here, right above the pill that opens it,
                while this conversation shows and the coach does not cover it. */}
            {replyHelp()}<div className="ai-tray-slot" ref={active && !coachCovers ? aiTraySlot : undefined} />{composerActivity}<button type="button" className="chat-coach" aria-expanded={coachCovers} onClick={() => openCoach()}>
            <ToolbarIcon name="idea" size={15} /><span>{tr("Coach")}</span></button></div>}
          {/* The divider is the recording panel's own top edge; everything above it
              stays with the conversation. */}
          <ResizeHandle className="composer-voice-resize" label={tr("Resize the recording panel")} axis="y" grow={-1} size={voiceHeight} min={150} max={640}
            measure={() => composer.current?.querySelector('.composer-voice')?.getBoundingClientRect().height ?? 0} onResize={setVoiceHeight} />
          <ComposerInput stream={mic.recording ? <LiveRecording source={mic.waveSource} spectrum={mic.spectrum} time={recorder.time} /> : null} prompt={greetingPrompt}
            layout={recorder} mode={voiceMode} onMode={setVoiceMode} onHoldStart={holdStart} onHoldEnd={holdEnd} onAutoSend={() => void toggleSetting('auto_send')}
            microphoneSelector={<MicrophoneSelector value={settings?.microphone_device_id ?? null} disabled={!settings || mic.starting || mic.recording || mic.transcribing}
              onChange={microphone_device_id => { void useSettingsStore.getState().update(current => ({ ...current, microphone_device_id }), 'Changing microphone') }} />}
            microphoneCheck={<MicrophoneCheck device={settings?.microphone_device_id ?? null} disabled={!settings || mic.starting || mic.recording || mic.transcribing} />}
            device={settings ? settings.microphone_device_id : undefined} silentTake={mic.silentTake !== null} onDismissSilentTake={mic.dismissSilentTake} micShortcut={settings?.shortcuts.mic} input={input} available={isTauri && connection?.configured === true} sending={editingTurnId !== null ? acceptingSend.current || acceptedEditSource !== null : sending}
            health={mic.health} deviceLabel={mic.deviceLabel} starting={mic.starting} recording={mic.recording} transcribing={mic.transcribing} autoSend={settings?.auto_send ?? false}
            targetLanguageTag={targetLanguage?.languageTag} targetLanguageName={targetLanguage?.endonym ?? ''}
            onInput={setInput} onSend={text => { void send(text) }}
            onDiscardRecording={mic.cancel} onToggleRecording={toggleMic} />
        </div>
  )

  // Both surfaces share message capabilities, availability and action handlers.
  const renderTurn = (turn: typeof activeTurns[number], onlySide?: 'user' | 'assistant') => (
    <ConversationErrorScope key={onlySide ? `message:${onlySide}:${turn.turnId ?? turn.id}` : turn.turnId ? turnDisplayKeys.get(turn.turnId) : turn.id} conversationId={snapshot?.conversationId} turn={turn.execution}><TurnView
      onlySide={onlySide}
      onStartPhrase={startFromPhrase}
      turn={turn}
      pendingEdit={pendingMessage?.editing?.id === turn.id ? pendingMessage : undefined}
      editing={turn.id === editingTurnId}
      onActivity={() => useNavigationStore.getState().inspectAi({ conversationId: snapshot?.conversationId ?? null, turnId: turn.turnId ?? null, operationKind: null })}
      onReplyControl={turn.turnId ? async control => { await executeAction(await readWorkspace(), { kind: 'controlTurn', turnId: turn.turnId!, control }) } : undefined}
      onRetryGloss={async operationId => { await executeAction(await readWorkspace(), { kind: 'retryGloss', operationId }) }}
      reviewing={turn.analysisState === 'pending' || reviewing.has(turn.id)}
      onAskCoach={askCoach}
      onSelectMessage={selectMessage}
      selectedSide={selectedTurn?.id === turn.id ? selectedSide : undefined}
      focused={(pinnedId ?? latestAssistantId) === turn.id}
      ttsReady={isTauri && Boolean(turn.assistant?.messageId)}
      speaking={Boolean(turn.assistant?.messageId && speech.messageId === turn.assistant.messageId && speech.phase === 'playing')}
      speechError={speech.failure?.messageId === turn.assistant?.messageId ? speech.failure ?? undefined : undefined}
      onSpeak={() => { if (turn.assistant?.messageId) speech.resume(turn.assistant.messageId) }}
      partnerSpeech={isTauri && turn.assistant?.messageId ? {
        retained: speech.retained?.audio.messageId === turn.assistant.messageId ? speech.retained : null,
        time: speech.retained?.audio.messageId === turn.assistant.messageId ? speech.time : 0,
        playing: speech.messageId === turn.assistant.messageId && speech.phase === 'playing',
        preparing: speech.messageId === turn.assistant.messageId && (speech.phase === 'preparing' || speech.phase === 'buffering'),
        enabled: active && !mic.recording && !mic.transcribing,
        rate: settings?.tts_rate ?? 1, volume: (settings?.master_volume ?? 100) * (settings?.voice_volume ?? 100) / 10000,
        seek: seconds => speech.seek(turn.assistant!.messageId!, seconds), stop: speech.stop,
        toggle: () => speech.resume(turn.assistant!.messageId!),
      } : undefined}
      recording={turn.id === recordingTurnId && mic.lastTranscription ? { result: mic.lastTranscription, rate: settings?.tts_rate ?? 1,
        volume: (settings?.master_volume ?? 100) * (settings?.voice_volume ?? 100) / 10000, enabled: !mic.recording && !mic.transcribing,
        onExpand: () => setInspectionOpen(true) } : undefined}
      rtl={rtl}
      onBubbleTap={onBubbleTap}
      onAddContext={turn.turnId && !turn.replacedBy ? async note => { await executeAction(await readWorkspace(), {kind:'reassessFeedback', turnId:turn.turnId!, note}) } : undefined}
      onRetryHelp={turn.turnId ? async () => { await executeAction(await readWorkspace(), {kind:'controlTurn', turnId:turn.turnId!, control:'retry'}) } : undefined}
      onCoachControl={turn.turnId ? async (selected, control) => {
        if (!selected.turnId) throw new Error('Coaching source is unavailable.')
        await coachControl(selected.turnId, control)
      } : undefined}
      editDisabled={editBlocked}
      onEditUser={canEdit(turn) ? startEdit : undefined}
    />
    </ConversationErrorScope>
  )

  // The coach: beside the conversation at full width; a full-screen modal where
  // the docked panel does not fit (compact and narrow).
  const coachPanel = (
      <section
        className={`break ${breakOpen || isMobile ? '' : 'collapsed'}`}
        ref={breakRef}
      >
        {!breakOpen && !isMobile && <button type="button" className="break-head" onClick={toggleBreak} aria-expanded={false}><ToolbarIcon name="idea" size={16} /><span>{tr("Coach")}</span></button>}

        {/* Private coaching and message assessment. */}
        {currentChatId && <CoachAnalysisPanel
          targetLanguageName={targetLanguage ? translatedName(tr.locale, targetLanguage.name) : ''}
          key={`${currentChatId}:${settings?.target_language}:${settings?.native_language}:${threadReload}`}
          coachingContent={<>{selectedTurn && renderTurn(selectedTurn, selectedSide)}{selectedTurn && selectedSide === 'assistant' ? <AnalysisContent showMessages={false} replyHelp={replyHelp(selectedTurn, true)} partnerOnly requestOnOpen={false} key={selectedTurn.turnId} turn={selectedTurn} conversationId={snapshot?.conversationId} onAsk={askCoach} nativeLanguageName={nativeLanguageName} showRomanization={showRomanization} rtl={rtl} /> : <>
          <LiveCoachReview key={coachedTurn?.id} revealOnView={!selectedTurn} turn={coachedTurn} onEdit={coachedTurn && canEdit(coachedTurn) && !editBlocked && editingTurnId !== coachedTurn.id ? () => startEdit(coachedTurn) : undefined} visible={active && mode === 'practice' && panelTab === 'coaching' && (isMobile || breakOpen)} nativeLanguageName={nativeLanguageName} rtl={rtl} onControl={async control => {
            if (!coachedTurn?.turnId) throw new Error('Coaching is unavailable.')
            await coachControl(coachedTurn.turnId, control)
          }} /></>}</>}
          chatId={currentChatId}
          conversationBusy={sending || details.saving}
          onCollapse={isMobile ? closeCoach : toggleBreak}
          tab={panelTab}
          onTab={setPanelTab}
          autoSendDraft
          draftQuestion={coachDraft}
          revealRequest={coachRevealRequest}
          onDraftConsumed={consumeCoachDraft}
          pinnedTurn={pinnedTurn}
          nativeLanguageName={nativeLanguageName}
          showRomanization={showRomanization}
          rtl={rtl}
        />}

      </section>
  )

  // The difficulty select sits beside the partner whenever the header has room
  // for it. The header measures its own room and says when the select has to
  // lead the settings sheet instead; it is never wrapped onto a second row.
  const difficultyControl = details.conversation && <DifficultySelect value={!snapshot?.opening && startConfiguration ? startConfiguration.difficulty : details.conversation.settings.difficulty} saving={details.saving} onChange={async difficulty => { if (!snapshot?.opening && startConfiguration && currentChatId) setStartDraft({ id: currentChatId, value: { ...startConfiguration, difficulty } }); else await details.saveDifficulty(difficulty) }} />

  return (
    <ConversationReadingProvider snapshot={snapshot} conversation={details.conversation}><AskCoachContext value={askCoach}><ReadingPreferencesProvider settings={settings}><RewardPresentationProvider enabled={settings?.xp_effects !== false} chatId={currentChatId} active={active}><PracticeContext value={{ chatId: currentChatId, selectionVersion, selected: skillSelection && skillSelection.target === settings?.target_language ? skillSelection.skillId : null, select: skillId => { if (!settings) throw new Error('Settings are not loaded'); selectSkill({ target: settings.target_language, skillId }) } }}>
    <div className="guided-workspace">
    <div
      ref={workspace}
      className={`split ${isMobile ? 'mobile-conversation' : ''} ${coachCovers ? 'mobile-coach' : ''} ${surfaceSwitched ? 'surface-switched' : ''}`}
    >
      <ChatHistory
        open={historyOpen}
        chats={contactChats}

        currentId={currentChatId}
        languageName={targetLanguageName}
        onClose={() => setHistoryOpen(false)}
        onOpenChat={(id) => void openChat(id)}
        onNewChat={() => void startNewConversation()}
        onDeleteChat={(id) => void removeChat(id)}
      />
      {/* ── Chat half (paper) ─────────────────────────────────────────── */}
      <section className="chat" data-stripe={Array.from(currentChatId ?? '').reduce((sum, char) => sum + char.charCodeAt(0), 0) % CHAT_STRIPES}>
        <ConversationHeader leading={<button type="button" className="chat-conversations" aria-label={tr("Conversations")} title={tr("Conversations")}
            aria-expanded={historyOpen} onClick={() => setHistoryOpen(!historyOpen)}><ToolbarIcon name="menu" size={17} /></button>}
          persona={<PersonaPicker choices={contactChoices} currentId={activeContactId} open={partnerMenuOpen} onOpenChange={setPartnerMenuOpen}
          busy={creatingConversation} onSelect={id => { void chooseContact(id) }} onEdit={() => setEditingPersonaId(details.persona?.id ?? null)} onCreate={() => setNewPersonaOpen(true)} />} error={details.error}
          difficulty={difficultyControl} onDifficultyPlace={setDifficultyPlace}>
          <div className="chat-heading-actions">
          <XpChip chatId={currentChatId} onOpen={openProgress} />
          <ConversationSettings summary={settings?.auto_speak ? tr("Reading aloud") : undefined} open={settingsOpen} onOpenChange={setSettingsOpen} settings={settings} saving={savingReading} onToggle={toggleSetting}
            nativePicker={nativePicker} difficulty={difficultyPlace === 'settings' ? difficultyControl : undefined} showRomanization={showRomanization} exportDisabled={!currentChatId} onExport={() => setExportOpen(true)}
            promptControls={snapshot?.opening && details.conversation && <ConversationDirectionSettings conversationId={snapshot.conversationId} topics={snapshot.topicChoices} direction={details.conversation.settings.direction} />} />
          <button type="button" className="chat-new" aria-label={tr("New conversation")} title={tr("New conversation")} disabled={creatingConversation || !currentChatId} onClick={() => void startNewConversation()}><ToolbarIcon name="plus" size={17} /></button>
          </div>
        </ConversationHeader>
        <SkillRewards chatId={currentChatId} active={active} />
        <div className="stream" data-editing={editingTurnId !== null ? '' : undefined} ref={streamRef} onScroll={streamScroll.onScroll}>
          {readError && <ErrorNotice as="div" error={readError}><p>{tr("Conversation updates stopped.")} {readError}</p><button type="button" onClick={retryRead}>{tr("Retry reading conversation")}</button></ErrorNotice>}
          {snapshot?.hasOlder && <button type="button" disabled={loadingOlder} onClick={() => void loadOlder()}>{loadingOlder ? tr("Loading older messages…") : tr("Load older messages")}</button>}
          {olderError && <ErrorNotice as="p" error={olderError}>{olderError}</ErrorNotice>}
          {turns.length === 0 && !error && !sending && connection?.configured === false ? (
            <div className="access-start">
              <p>{tr("Choose how to connect to AI.")}</p>
              <button type="button" className="btn primary" disabled={signingIn} onClick={() => void startHostedSignIn()}>
                {signingIn && <span className="activity-spinner" aria-hidden="true" />}{signingIn ? tr("Signing in…") : tr("Sign in with Google")}
              </button>
              <button type="button" className="access-alternative" onClick={onOpenSettings ?? (() => useNavigationStore.getState().showOverlay('settings'))}>
                {tr("Or set up AI access in another way")}
              </button>
            </div>
          ) : turns.length === 0 && !readError && !error && !pendingMessage && (
            snapshot && (snapshot.opening ? <OpeningStatus snapshot={snapshot} onActivity={() => useNavigationStore.getState().showOverlay('activity')} /> : startConfiguration && <ConversationStart conversationId={snapshot.conversationId} value={startConfiguration} onChange={value => setStartDraft({ id: snapshot.conversationId, value })} partnerSymbol={contactChoices.find(choice => choice.id === activeContactId)?.symbol} partnerName={details.persona ? personaName(details.persona.details) : undefined} key={snapshot.conversationId} topics={snapshot.topicChoices} busy={sending || pendingReply} onStart={startConversation} targetTag={targetLanguage?.languageTag ?? undefined} targetDir={rtl ? 'rtl' : 'ltr'} recording={mic.recording} transcribing={mic.transcribing} canPartnerStart={!input.trim() && !mic.recording && !mic.transcribing} onChangePartner={() => setPartnerMenuOpen(true)} onAboutPartner={details.persona ? () => setEditingPersonaId(details.persona!.id) : undefined} />)
          )}
          {snapshot?.phraseSeed && <details><summary>{tr('Starting phrase')}</summary><p dir="auto">{snapshot.phraseSeed}</p></details>}
          {activeTurns.map(turn => renderTurn(turn))}
          {pendingMessage && !pendingMessage.editing && <PendingTurn key={pendingMessage.key} message={pendingMessage} rtl={rtl} onOpenSettings={onOpenSettings}
            onDismiss={() => releasePending(pendingMessage.key)} />}
          {streamScroll.unseen && <button type="button" className="stream-jump" onClick={streamScroll.jumpToLatest}>{tr("New messages")}</button>}
          {error && <RequestFailure error={error} onOpenSettings={onOpenSettings} />}
        </div>

        {!isMobile && chatComposer}
      </section>

      {!isMobile && breakOpen && <PracticeDivider workspace={workspace} />}

      {isMobile && <div className="chat mobile-composer">{chatComposer}</div>}
      {coachCovers && <div className="coach-scrim" aria-hidden="true" onClick={closeCoach} />}

      {coachPanel}

    </div>

      {contactError && <ErrorDetails label={tr("Partner")} errorKey={contactError}>{contactError}</ErrorDetails>}
      {newPersonaOpen && settings && <NewPersonaDialog key="new-persona" language={settings.target_language} romanized={romanized} busy={creatingConversation}
        onCreate={createPersona} onClose={() => setNewPersonaOpen(false)} />}
      {editingPersona && <PersonaProfileDialog key={editingPersona.id} persona={editingPersona} language={targetLanguageLabel(editingPersona.languageId)} romanized={Boolean(languageFor(editingPersona.languageId)?.romanization)} onSave={details.savePersona} onNewPersona={() => { setEditingPersonaId(null); setNewPersonaOpen(true) }} onClose={() => setEditingPersonaId(null)} />}
      {inspectionOpen && mic.lastTranscription && <TranscriptionInspector key={mic.lastTranscription.inspection.recordingId} result={mic.lastTranscription}
        rate={settings?.tts_rate ?? 1} volume={(settings?.master_volume ?? 100) * (settings?.voice_volume ?? 100) / 10000} enabled={!mic.recording && !mic.transcribing}
        onClose={() => setInspectionOpen(false)} />}
      {revisionConfirmation && <DetailDialog title={tr("Revise earlier message")} onClose={() => setRevisionConfirmation(null)}>
        <p>{tr("This revision removes ")}{revisionConfirmation.exchangeCount} {tr(" later conversation turns. Your edited message replaces the original; private coach history is kept.")}</p>
        <div className="detail-actions">
          <button type="button" onClick={() => setRevisionConfirmation(null)}>{tr("Cancel")}</button>
          <button type="button" disabled={acceptingSend.current || acceptedEditSource !== null} onClick={() => void submitText(revisionConfirmation.text, revisionConfirmation.input, revisionConfirmation.revision)}>{tr("Revise and remove later turns")}</button>
        </div>
      </DetailDialog>}
      {exportOpen && currentChatId && <ConversationExport key={currentChatId} conversationId={currentChatId} onClose={() => setExportOpen(false)} />}
      {analysisOpen && (pinnedTurn?.assistant ? <PartnerMessageAnalysis key={pinnedTurn.turnId} assistant={pinnedTurn.assistant} conversationId={snapshot?.conversationId} onClose={() => setAnalysisOpen(false)} /> : <DetailDialog title={tr("Message analysis")} onClose={() => setAnalysisOpen(false)}>
        <h2>{tr("Message analysis")}</h2>
        <p>{tr("Select Analysis on a conversation reply to inspect that message.")}</p>
      </DetailDialog>)}

    </div>
    </PracticeContext></RewardPresentationProvider></ReadingPreferencesProvider></AskCoachContext></ConversationReadingProvider>
  )
}
