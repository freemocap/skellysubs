// Generated from Rust contracts. Run npm run contracts.
export const diagnosticCommands = ["workflow_control","start_drill_session","end_drill_session","enter_drill_visit","leave_drill_visit","get_drill_sessions","get_drill_storage","set_drill_storage","conversation_drill_candidates","get_drill_generation_activity","begin_drill_preview","cancel_drill_preview","preview_drill_items","get_drill_preview","accept_drill_items","discard_drill_preview","create_drill_item","get_practice_sets","preview_practice_set","accept_practice_phrases","get_drill_items","get_last_drill_item","drill_attempts","delete_drill_item","delete_drill_attempt","clear_drill_attempts","get_drill_attempt_audio","inspect_drill_audio","begin_reading","run_reading","get_skill_guide","get_cached_reading_audio","cancel_reading","get_reading_activity","get_saved_gloss_sources","get_update_channel","latest_github_release","read_speech_audio","inspect_message_speech","get_inference_cache_settings","save_inference_cache_settings","record_frontend_diagnostic","share_diagnostic_logs","save_diagnostic_logs","read_frontend_diagnostics","list_microphones","get_microphone","save_microphone","microphone_test_start","microphone_test_samples","microphone_test_stop","mic_start","mic_listen_start","mic_listen_push","mic_listen_status","mic_listen_spectrogram","mic_listen_stop","mic_listen_discard","mic_listen_tune","mic_wave","mic_spectrogram","mic_push","mic_cancel","mic_transcribe","mic_retry_transcription","get_transcription_result","factory_reset","export_workspace","get_startup_state","retry_credential_cleanup","get_snapshot","preferred_languages","preview_conversation_prompt","inspect_language","execute_command","get_access_settings","save_access_settings","check_access","local_server_available","connect_local_server","open_local_admin","get_connection","save_models","watch_conversation","get_message_history","begin_persona_generation","run_persona_generation","cancel_persona_generation","hosted_sign_in","hosted_account","hosted_diagnostics","hosted_sign_out","cancel_sign_in","select_route","get_profile","get_persona_generation_activity","get_skill_evidence","get_learner_state","get_learner_profile","claim_reward_events","initialize_skill_level_events","claim_skill_level_events","get_effort_progress","refresh_streaks","record_bot_inspection","get_effort_report","claim_effort_awards","export_learner_state","save_learner_state","view_conversation_yaml","save_conversation_yaml","get_reward_settings","get_playback_rate","save_playback_rate","save_reward_settings","save_skill_profile","get_practice_overview","get_language_totals","open_ai_window","get_practice_view","set_practice_view","list_turn_history","read_graph_history","read_native_graph_attempt","list_native_workspace_runs","read_native_run_history","read_native_workspace_graph","ai_window_state","dock_ai_window","set_ai_view_selection","get_ai_view_selection"] as const;
export type RecordingStarted = { deviceLabel: string | null, recordingId: string, samplesPerSecond: number, browserCapture: boolean, browserDeviceId: string | null, };
export type MicrophoneTestStarted = { browserCapture: boolean, deviceLabel: string | null, };
export type ContinuousRecordingPolicy = { version: number, pauseOptionsMs: [number, number, number, number, number], defaultPauseMs: number, silenceTimeoutOptionsMs: [number, number, number, number, number], defaultSilenceTimeoutMs: number,
/**
 * The level (dBFS) a frame must exceed to count as speech. The learner sets
 * it directly; it does not follow the measured room noise, so it stays where
 * it was put while listening.
 */
minThresholdDb: number, maxThresholdDb: number, defaultThresholdDb: number,
/**
 * Voiced time a take needs before it is kept; shorter bursts are ignored.
 */
minTakeOptionsMs: [number, number, number, number], defaultMinTakeMs: number, maxPendingTakes: number, maxTakeSeconds: number, maxSessionSeconds: number, maxTakes: number, };
export type ListeningSettings = { pauseMs: number, silenceTimeoutMs: number, thresholdDb: number, minTakeMs: number, };
export type ListeningMode = "auto" | "monitor" | "manual";
export type ListeningStatus = { recordingId: string, listening: boolean, speaking: boolean, queued: number, processing: boolean, completed: number, takes: Array<ListeningTake>, failure: AppError | null,
/**
 * The boundary choices in force now.
 */
settings: ListeningSettings,
/**
 * Loudest analysed frame in the latest drain, in dBFS.
 */
levelDb: number,
/**
 * The measured room noise (absent until a quiet frame has been heard) and
 * the level a frame must exceed to count as speech.
 */
noiseFloorDb: number | null, thresholdDb: number,
/**
 * Bursts that opened a take but ended shorter than the shortest take allowed.
 */
ignoredTakes: number, };
export type ListeningTake = { recordingId: string, number: number, startSeconds: number, endSeconds: number, cutSeconds: number, state: ListeningTakeState, failure: AppError | null, };
export type LiveSpectrogram = { data: InspectionSpectrogram, endSeconds: number, };
export type ListeningTakeState = "queued" | "processing" | "completed" | "failed";
export type Theme = "light" | "dark" | "system";
export type RewardEvent = { id: string, attemptId: string, constructId: string, kind: string, tier: number, xp: number, experience: number, effort: number, quote: string, support: string, difficulty: string, novelty: string, policyHash: string, atSecs: bigint, claimed: boolean, };
export type SkillLevelEventKind = "skill_level" | "language_level";
export type SkillLevelEvent = { id: string, sequence: number, kind: SkillLevelEventKind, skillId?: string, fromLevel: number, toLevel: number, sourceAttemptId?: string, chatId?: string, messageId?: number, };
export type SkillLevelProgress = { skillId: string, points: number, level: number, currentThreshold: number, nextThreshold: number, };
export type SkillLevelSummary = { policyId: string, skills: Array<SkillLevelProgress>,
/**
 * Minimum skill level within this evidence scope, not a proficiency rating.
 */
level: number, currentThreshold: number, nextThreshold: number,
/**
 * Thresholds for levels 1 through the scope's next level, inclusive.
 */
bands: Array<number>, };
export type EffortDimension = "partner_understood" | "revisions_sent" | "practice_attempts" | "explorations" | "bot";
export type EffortAward = { id: string, dimension: EffortDimension, sourceId: string, language: string, variety: string, conversationId: string | null, policy: string, createdAt: string, claimed: boolean, };
export type EffortProgress = { target: string, partnerUnderstood: number, revisionsSent: number, practiceAttempts: number, explorations: number, bot: number, recent: Array<EffortAward>, };
export type StreakCounter = { count: number, day: number | null, celebration: string | null, };
export type StreakSnapshot = { sessionId: string, today: number, app: StreakCounter, };
export type LanguageTotals = { target: string, name: string, nativeName: string, languageTag: string | null, xp: number, conversations: number, partnerUnderstood: number, revisionsSent: number, practiceAttempts: number, explorations: number, bot: number, };
export type EffortActivity = { dimension: EffortDimension, total: number, lastSevenDays: number, activeDays: number, firstAt: string, lastAt: string, };
export type EffortHistoryEntry = { id: string, dimension: EffortDimension, createdAt: string,
/**
 * Current retained source text, never another persisted copy.
 */
sourceText: string | null, };
export type EffortReport = { target: string, activity: Array<EffortActivity>, entries: Array<EffortHistoryEntry>, next: string | null, };
export type LearnerState = { learnerId: string, languageId: string, asOfSecs: number, configHash: string, constructRegistryHash: string, estimatorHash: string, estimatorVersion: number, calibration: string, choices: unknown, observations: unknown[], constructs: Array<ConstructState>, };
export type ConstructState = { constructId: string, varietyId: string, rating: number, uncertainty: number, lastSeen: number, halfLifeDays: number, n: number, independentN: number, effectiveN: number, recall: number, dueAt: number, due: boolean, insufficientEvidence: boolean, evidenceAttemptIds: Array<string>, };
export type ConnectionRoute = "hosted" | "custom";
export type SurfacePalette = "cool" | "warm";
export type MotionPreference = "system" | "full" | "reduced";
export type AppearancePreferences = { palette: SurfacePalette, motion: MotionPreference, };
export type ExecutionMode = "automatic" | "on_demand";
export type ExecutionPreferences = { assessment: ExecutionMode, replyBrief: ExecutionMode, reading: ExecutionMode, };
export type MessageHelp = "assessment" | "coaching" | "reply_brief" | "translation" | "word_gloss";
export type AccessSettings = { credentialPreviews?: { [key in string]: string }, customUrlIsUnsavedDefault: boolean, revision: number, customKeyConfigured: boolean, custom: CustomEndpoint, };
export type ProviderCredentialCheck = { diagnostics?: unknown, provider: string, state: string, status: number | null, durationMs: number, };
export type AccessCheck = { providers: Array<ProviderCredentialCheck>, };
export type CustomEndpoint = { baseUrl: string, bearerAuth: boolean, };
export type HostedAccount = { email: string, name: string, usedUsd: number, limitUsd: number, remainingUsd: number, tokensToday: number, requestsToday: number, estimatedRequestsRemaining: number, estimatedTokensRemaining: number, customLimit: boolean, resets: string, };
export type UsageSummary = { id: string, label: string, conversations: number, learnerMessages: number, personaMessages: number, attempts: number, inputTokens: number, outputTokens: number, unknownUsage: number, };
export type ProfileSnapshot = { revision: number, global: UsageSummary, languages: Array<UsageSummary>, personas: Array<UsageSummary>, };
export type PersonaGenerationAttempt = { finishReason?: string, diagnostics?: unknown, id: string, attemptId: string, operationId: string, languageId: string, route: ConnectionRoute, requestedModel: string, profileRevision: number, state: string, createdAt: string, dispatchedAt: string | null, finishedAt: string | null, actualModel: string | null, providerId: string | null, inputTokens: number | null, outputTokens: number | null, error: string | null, };
export type PersonaGenerationUsage = { attempts: number, inputTokens: number, outputTokens: number, unknownUsage: number, };
export type PersonaGenerationActivity = { revision: number, attempts: Array<PersonaGenerationAttempt>, usage: PersonaGenerationUsage, };
export type AudioModelSettings = { model: string, };
export type AudioSettings = { transcription: AudioModelSettings, speech: AudioModelSettings, };
export type AssessmentAdapter = "jev_choice" | "chat_model";
export type ConnectionConfig = { assessmentAdapter: AssessmentAdapter, route: ConnectionRoute, signedIn: boolean, email: string, revision: number, configured: boolean, standardModel: string, fastModel: string, audio: AudioSettings, paused: boolean, };
export type TurnControl = "pause" | "resume" | "step" | "cancel" | "retry";
export type GlossSegmentKind = "gloss" | "literal" | "unresolved";
export type GlossCoverage = "complete" | "partial";
export type GlossSegment = { start: number, end: number, kind: GlossSegmentKind, gloss: string | null, romanization?: string, pronunciation?: string, };
export type WordGlossView = { sourceMessageId: string, targetLanguageId: string, explanationLanguageId: string, formatVersion: string, templateVersion: string, boundaryPolicy: string, operationId: string, attemptId: string, coverage: GlossCoverage, segments: Array<GlossSegment>, };
export type DiagnosticCommand = "workflow_control" | "start_drill_session" | "end_drill_session" | "enter_drill_visit" | "leave_drill_visit" | "get_drill_sessions" | "get_drill_storage" | "set_drill_storage" | "conversation_drill_candidates" | "get_drill_generation_activity" | "begin_drill_preview" | "cancel_drill_preview" | "preview_drill_items" | "get_drill_preview" | "accept_drill_items" | "discard_drill_preview" | "create_drill_item" | "get_practice_sets" | "preview_practice_set" | "accept_practice_phrases" | "get_drill_items" | "get_last_drill_item" | "drill_attempts" | "delete_drill_item" | "delete_drill_attempt" | "clear_drill_attempts" | "get_drill_attempt_audio" | "inspect_drill_audio" | "begin_reading" | "run_reading" | "get_skill_guide" | "get_cached_reading_audio" | "cancel_reading" | "get_reading_activity" | "get_saved_gloss_sources" | "get_update_channel" | "latest_github_release" | "read_speech_audio" | "inspect_message_speech" | "get_inference_cache_settings" | "save_inference_cache_settings" | "record_frontend_diagnostic" | "share_diagnostic_logs" | "save_diagnostic_logs" | "read_frontend_diagnostics" | "list_microphones" | "get_microphone" | "save_microphone" | "microphone_test_start" | "microphone_test_samples" | "microphone_test_stop" | "mic_start" | "mic_listen_start" | "mic_listen_push" | "mic_listen_status" | "mic_listen_spectrogram" | "mic_listen_stop" | "mic_listen_discard" | "mic_listen_tune" | "mic_wave" | "mic_spectrogram" | "mic_push" | "mic_cancel" | "mic_transcribe" | "mic_retry_transcription" | "get_transcription_result" | "factory_reset" | "export_workspace" | "get_startup_state" | "retry_credential_cleanup" | "get_snapshot" | "preferred_languages" | "preview_conversation_prompt" | "inspect_language" | "execute_command" | "get_access_settings" | "save_access_settings" | "check_access" | "local_server_available" | "connect_local_server" | "open_local_admin" | "get_connection" | "save_models" | "watch_conversation" | "get_message_history" | "begin_persona_generation" | "run_persona_generation" | "cancel_persona_generation" | "hosted_sign_in" | "hosted_account" | "hosted_diagnostics" | "hosted_sign_out" | "cancel_sign_in" | "select_route" | "get_profile" | "get_persona_generation_activity" | "get_skill_evidence" | "get_learner_state" | "get_learner_profile" | "claim_reward_events" | "initialize_skill_level_events" | "claim_skill_level_events" | "get_effort_progress" | "refresh_streaks" | "record_bot_inspection" | "get_effort_report" | "claim_effort_awards" | "export_learner_state" | "save_learner_state" | "view_conversation_yaml" | "save_conversation_yaml" | "get_reward_settings" | "get_playback_rate" | "save_playback_rate" | "save_reward_settings" | "save_skill_profile" | "get_practice_overview" | "get_language_totals" | "open_ai_window" | "get_practice_view" | "set_practice_view" | "list_turn_history" | "read_graph_history" | "read_native_graph_attempt" | "list_native_workspace_runs" | "read_native_run_history" | "read_native_workspace_graph" | "ai_window_state" | "dock_ai_window" | "set_ai_view_selection" | "get_ai_view_selection";
export type RewardSettings = { revision: number, fastMode: boolean, effectsEnabled: boolean, rewardSounds: string, masterVolume: number, voiceVolume: number, effectsVolume: number, };
export type MicrophoneSource = "native" | "browser";
export type MicrophoneDevice = { id: string, label: string, isDefault: boolean, channels: number | null, sampleRate: number | null, unavailable: string | null, };
export type MicrophoneList = { source: MicrophoneSource, devices: Array<MicrophoneDevice>, };
export type InputEvidence = { modality: string, suggestion: boolean, scaffold: boolean, revision: boolean, };
export type Outcome = "demonstrated" | "partial" | "not_demonstrated" | "not_observed" | "uncertain";
export type RevisionSuffixCount = { turnId: string, exchangeCount: number, coachTurnCount: number, };
export type MeaningLevel = "full" | "partial" | "none";
export type ErrorOp = "missing" | "replace" | "unnecessary";
export type ErrorSource = "transfer" | "developmental" | "slip" | "unknown";
export type ErrorTag = { op: ErrorOp, category: string, source: ErrorSource, blocks_meaning: boolean, target_hypothesis: string, hint: string, elicitation: string, metalinguistic: string, };
export type ObservedItem = { construct: string, quote: string, outcome: Outcome, error: ErrorTag | null, rationale: string, };
export type CoachObservation = { meaning_recovered: MeaningLevel, items: Array<ObservedItem>, };
export type ObservedItemSummary = { construct: string, quote: string, outcome: Outcome, rationale: string, };
export type CoachObservationView = {
/**
 * Correctable source spans, independent of skill evidence and disclosure.
 */
issues: Array<CoachIssue>, corrections: Array<Correction>, notes: Array<string>, meaningRecovered: MeaningLevel, items: Array<ObservedItemSummary>, candidatesSent: number, itemsReturned: number, };
export type CoachIssue = { quote: string, severity: CoachIssueSeverity, };
export type CoachIssueSeverity = "error" | "partial";
export type CoachMove = "partner_clarify" | "hint" | "elicit" | "metalinguistic" | "explicit";
export type Correction = { construct: string, quote: string, move: CoachMove, text: string, explanation?: string, };
export type CoachDecision = { exposedMove: CoachMove | null, shown: Correction | null, retryInvited: boolean, alsoNoticed: Array<ObservedItemSummary>, keptGoing: boolean, };
export type CoachControl = "open_card" | "show_answer" | "keep_going";
export type PartnerReaction = { kind: ReactionKind, answer: ChoiceAssessment, };
export type ReactionKind = "understood" | "confused";
export type Opening = { "kind": "learner" } | { "kind": "partner" };
export type TopicCard = { id: string,
/**
 * Decorative; the labels carry the meaning.
 */
glyph: string,
/**
 * The scene name in the conversation's target language.
 */
target: string,
/**
 * The target name transliterated, when the conversation's variety resolves
 * to a romanization scheme. `None` for Latin-script languages.
 */
romanized: string | null,
/**
 * The same scene name in the conversation's explanation language. Equal to
 * `target` when the two languages match; the surface decides whether to
 * draw it twice.
 */
translation: string, };
export type SavedTopic = { id: string, text: string, };
export type TimeReference = "any" | "past" | "future";
export type RecommendationMode = "explore" | "continuePracticing" | "coachChoice";
export type TopicChoice = { "kind": "skill", skillId: string, subskillId: string | null, } | { "kind": "builtin", id: string, } | { "kind": "coach", mode: RecommendationMode, } | { "kind": "custom", text: string, };
export type ConversationDirection = { topic: TopicChoice | null, timeReference: TimeReference, usePersonaDetails: boolean, };
export type ConversationStartConfig = {
/**
 * The learner applied the prompt editor before starting this conversation.
 */
promptEditor?: boolean, difficulty: Difficulty, varietyId: string, direction: ConversationDirection, };
export type PromptPreview = { coachFocus: CoachFocusPreview | null, configuration: ConversationStartConfig, yaml: string, systemPrompt: string, difficultyPrompts: Array<[Difficulty, string]>, };
export type CoachFocusPreview = { skillId: string, name: string, mode: RecommendationMode, experience: number, effort: number, };
export type SuggestedReply = { text: string, segments: Array<GlossSegment>, };
export type ConversationFeedback = { grammar: number | null, conversation: number | null, answers: { [key in string]: ChoiceAssessment }, };
export type ChoiceAssessment = { choice: string, probabilities: { [key in string]: number }, confidence: number, };
export type AssistedReply = { text: string, translation: string, romanization: string, pronunciation: string, };
export type ReplyAssistance = { replies: Array<AssistedReply>, frames: Array<string>, starters: Array<string>, };
export type ReplyBrief = { explanation: string, };
export type ReplyHelpKind = "brief" | "grammar" | "replies";
export type ReadingScope = { language: string, variety: string | null, explanation: string, explanationVariety: string | null, };
export type ReplyExplanation = { quote: string, title: string, body: string, example: string, contrast: string, };
export type ReplyExplanations = { cards: Array<ReplyExplanation>, };
export type ChatMessage = { assessmentState?: string, assessmentError?: string, guideContext?: unknown, feedbackContext?: string, conversationFeedback?: ConversationFeedback, replyBrief?: ReplyBrief, briefState?: string, briefError?: string, readingScope?: ReadingScope, replyAssistance?: ReplyAssistance, replyExplanations?: ReplyExplanations, explanationsState?: string, explanationsError?: string, reaction?: PartnerReaction, reactionError?: string, coachDecision: CoachDecision | null, turnId: string, replacesTurnId: string | null, replacedBy: string | null, feedback?: CoachObservationView, feedbackState?: string, feedbackError?: string, suggestedReplies?: Array<SuggestedReply>, suggestionsState?: string, suggestionsError?: string, wordGloss: WordGlossView | null, glossState: string | null, glossError: string | null, glossOperationId: string | null, translationState: string | null, translation: string | null, id: string, sequence: number, role: string, text: string, createdAt: string, };
export type SpeechRequestView = { sourceMessageId: string, id: string, state: string, };
export type TurnView = {
/**
 * Structural product-field associations, not value/attempt provenance.
 */
nativeSurfaces?: { [key in string]: Array<string> }, awardSources?: Array<string>, nativeResponse?: unknown, channel?: string, nativePreview?: import('./graph-contracts').AttemptPreview, nativeGraph?: import('./graph-contracts').InspectionSnapshot, nativeExecutionAvailable?: boolean, replacesTurnId: string | null, replacedBy: string | null, route: ConnectionRoute, id: string, state: string, paused: boolean, hold: AppError | null, speech?: SpeechRequestView, };
export type TurnHistoryPage = { turns: Array<TurnView>, hasOlder: boolean, };
export type RecordedMessage = { role: string, content: string, };
export type AiViewSelection = { conversationId: string | null, turnId: string | null, operationKind: string | null, elsewhere?: AiElsewhereSelection, revision?: AiRevisionSelection, };
export type AiElsewhereSelection = { "kind": "workspace", entry: NativeRunEntry, } | { "kind": "transcription", id: string, };
export type AiRevisionSelection = { "kind": "live" } | { "kind": "structure" } | { "kind": "revision", revision: string, };
export type NativeAttemptInspection = { engine: string, run: string, artifact: string, revision: string, node: string, attempt: string, execution: string, owner: string, evidence: unknown, retainedText: string | null, };
export type NativeRunEntry = { engine: string, run: string, artifact: string, kind: string, };
export type NativeRunPage = { runs: Array<NativeRunEntry>, before: string | null, };
export type AiWindowState = { supported: boolean, open: boolean, };
export type ConversationSnapshot = { phraseSeed?: string, topicChoices: Array<TopicCard>,
/**
 * The greeting the start surface offers as a first thing to say, resolved
 * for this conversation's target language and variety.
 */
starterGreeting: StarterGreeting, opening: Opening | null, revisionSuffixCounts: Array<RevisionSuffixCount>, transcriptionAttempts: Array<TranscriptionAttempt>, coachMessages: Array<ChatMessage>, conversationId: string, sessionId: string, revision: number, messages: Array<ChatMessage>, turns: Array<TurnView>, connection: ConnectionConfig, hasOlder: boolean, };
export type MessageHistory = { conversationId: string, rootMessageId: string, currentMessageId: string, versions: Array<MessageVersionView>, };
export type MessageVersionView = { messageId: string, turnId: string, previousMessageId: string | null, text: string, createdAt: string, assessments: Array<VersionAssessmentState>, feedback: CoachObservationView | null, coachDecision: CoachDecision | null, conversationFeedback: ConversationFeedback | null, };
export type VersionAssessmentState = { kind: string, state: string, attemptId: string | null, error: string | null, };
export type Difficulty = "absolute_zero" | "beginner" | "intermediate" | "advanced" | "fluent";
export type HelpAmount = "minimal" | "balanced" | "generous";
export type CoachProactivity = "on_request" | "occasional" | "frequent";
export type PracticeSettings = { difficulty: Difficulty, direction: ConversationDirection, explanationLanguage: string, varietyId: string, explanationVarietyId: string, composingHelp: HelpAmount, coachProactivity: CoachProactivity, translation: boolean, pronunciation: boolean, romanization: boolean, autoSend: boolean, readAloud: boolean, speechVoice: string, };
export type SpeechUnavailableReason = "notRequested" | "cancelled" | "failed" | "unknownOutcome" | "expired";
export type SpeechAudioState = { "status": "streaming", operationId: string, messageId: string, executionId: string, sampleOffset: number, audioBase64: string, alignment: SpeechAlignment | null, } | { "status": "pending", operationId: string, messageId: string, } | { "status": "ready", operationId: string, attemptId: string, messageId: string, mime: string, audioBase64: string, alignment: SpeechAlignment | null, } | { "status": "unavailable", operationId: string, messageId: string, reason: SpeechUnavailableReason, message: string, attemptId: string | null, diagnostics: unknown, };
export type OnboardingStatus = "not_started" | "in_progress" | "skipped" | "completed";
export type PracticeView = "chat" | "drill";
export type Preferences = { execution: ExecutionPreferences, practiceView?: PracticeView, theme: Theme, appearance: AppearancePreferences, explanationLanguage: string, explanationVarietyId: string, interfaceLocale: string, targetVarieties: { [key in string]: string }, scriptScales?: { [key in string]: number }, myLanguages: Array<string>, textSize: number, textSpacing: number, highContrast: boolean, onboarding: OnboardingStatus, onboardingRequired: boolean, onboardingLanguage: string | null, onboardingHelp: boolean, };
export type Learner = { id: string, name: string, revision: number, preferences: Preferences, };
export type LanguageProfile = { id: string, learnerId: string, languageId: string, };
export type PersonaDetails = { name: string,
/**
 * The name in Latin letters, present exactly when the persona's language has
 * a romanization system.
 */
romanizedName: string | null,
/**
 * Left blank until someone chooses one.
 */
age: number | null, location: string, occupation: string, background: string, currentSituation: string, interests: Array<string>, opinions: Array<string>, interestingFacts: Array<string>, favoriteBooks: Array<string>, favoriteMovies: Array<string>, manner: string, quirks: Array<string>, vibe: Array<string>, };
export type Persona = { id: string, learnerId: string, languageId: string, revision: number, details: PersonaDetails, };
export type Contact = { id: string, learnerId: string, personaId: string, archived: boolean, revision: number, };
export type Conversation = { id: string, contactId: string, languageId: string, title: string, archived: boolean, revision: number, settingsRevision: number, settings: PracticeSettings, createdAt: string, lastUsed: number, };
export type Variety = { transcriptionLanguage: string | null, direction: string, fontScale: number, romanization: string | null, id: string, name: string, description: string, };
export type Language = { transcriptionLanguage: string | null, languageTag: string | null, fontScale: number, direction: string, romanization: string | null, id: string, name: string, nativeName: string, varieties: Array<Variety>, defaultVariety: string,
/**
 * The authored greeting, at this language's default variety. Carried on the
 * catalog so choosing a language can show what saying it sounds like.
 */
greeting: StarterGreeting,
/**
 * Who the learner meets first in this language. Enough of the bundled
 * starter persona to introduce them, not the whole profile.
 */
partner: LanguagePartner, };
export type LanguageInspection = { guides: Array<GuideInspection>, fingerprint: string, language: Language, varietyId: string, review: string, family: string, values: Array<ContentValue>, rules: Array<ContentRule>, schemes: Array<SchemeInspection>, sources: Array<ContentSource>, partner: PersonaDetails, schemaJson: string, resolvedJson: string, learningJson: string, conversationJson: string, };
export type GuideInspection = { guide: TeachingGuide, fingerprint: string,
/**
 * The UI and later assessment projection compose this supplement with the core.
 */
selectedVariety: string | null, source: string, explanationName: string, explanationTag: string | null, explanationDirection: string, };
export type TeachingGuide = { id: string, target: { "kind": "script", script: string, } | { "kind": "reading", language: string, } | { "kind": "skill", language: string, skill: string, }, explanation_language: string, title: string, summary: string, sections: Array<GuideSection>, examples: Array<GuideExample>,
/**
 * Compact authored guidance; not automatically added to existing prompts.
 */
guidance: string, shared_guides: Array<string>, variants: Array<GuideVariant>, sources: Array<string>, review: "needs_review" | "reviewed", origin: "human" | "ai" | "mixed", authorship: string, };
export type GuideSection = { title: string, text: string, };
export type GuideExample = { text: string, meaning: string, note: string, };
export type GuideVariant = { variety: string, summary: string, sections: Array<GuideSection>, examples: Array<GuideExample>, guidance: string, sources: Array<string>, review: "needs_review" | "reviewed", origin: "human" | "ai" | "mixed", authorship: string, };
export type ContentSource = { path: string, yaml: string, };
export type ContentRule = { scope: string, text: string, source: string, };
export type ContentValue = { field: string, value: string, source: string, };
export type SchemeInspection = { id: string, label: string, instructions: string, examples: Array<[string, string]>, sources: Array<string>, review: string, source: string, selected: boolean, usedBy: Array<string>, };
export type Snapshot = { sessionId: string, revision: number, learner: Learner, languages: Array<Language>, languageProfiles: Array<LanguageProfile>, personas: Array<Persona>, contacts: Array<Contact>, conversations: Array<Conversation>, savedTopics: Array<SavedTopic>, };
export type StartupState = {
/**
 * Set when the workspace could not be opened, so the shell cannot mount.
 */
refusal: AppError | null,
/**
 * Set when directories a previous reset recorded still could not be cleared.
 * The app is usable; that leftover data is not cleared.
 */
cleanup: AppError | null, credentialCleanup: AppError | null, };
export type Action = { "kind": "askGuideCoach", conversationId: string, text: string, guide: GuideReference, focus?: { "kind": "subskill", subskillId: string, } | { "kind": "example", index: number, }, expectedRevision: number, } | { "kind": "startGuideConversation", sourceConversationId: string, guide: GuideReference, example: number, phrase?: string, expectedRevision: number, } | { "kind": "startSkillConversation", sourceConversationId: string, language: string, variety: string, skillId: string, subskillId: string | null, expectedRevision: number, } | { "kind": "startPhraseConversation", sourceMessageId: string, phrase: string, contactId: string, expectedRevision: number, } | { "kind": "startConversation", conversationId: string, configuration: ConversationStartConfig, message: string | null, input: InputEvidence | null, expectedRevision: number, } | { "kind": "updateConversationPrompt", conversationId: string, configuration: ConversationStartConfig, additions: Array<string>, deletions: Array<string>, expectedRevision: number, expectedSettingsRevision: number, } | { "kind": "saveTopics", additions: Array<string>, deletions: Array<string>, expectedRevision: number, } | { "kind": "coachControl", turnId: string, control: CoachControl, } | { "kind": "reviseTurn", conversationId: string, turnId: string, text: string, input: InputEvidence, expectedRevision: number, } | { "kind": "askCoach", conversationId: string, text: string, expectedRevision: number, } | { "kind": "startChat", languageId: string, } | { "kind": "sendMessage", input: InputEvidence, conversationId: string, text: string, expectedRevision: number, } | { "kind": "requestMessageSpeech", messageId: string, regenerate?: boolean, } | { "kind": "cancelMessageSpeech", operationId: string, } | { "kind": "requestSuggestions", messageId: string, } | { "kind": "requestMessageHelp", messageId: string, help: MessageHelp, retry: boolean, } | { "kind": "requestExplanations", messageId: string, } | { "kind": "reassessFeedback", turnId: string, note: string, } | { "kind": "retryReplyHelp", messageId: string, helpKind: ReplyHelpKind, } | { "kind": "retryGloss", operationId: string, } | { "kind": "controlTurn", turnId: string, control: TurnControl, } | { "kind": "setPaused", paused: boolean, } | { "kind": "createContact", languageId: string, details: PersonaDetails, } | { "kind": "updatePersona", personaId: string, expectedRevision: number, details: PersonaDetails, } | { "kind": "setContactArchived", contactId: string, expectedRevision: number, archived: boolean, } | { "kind": "deleteContact", contactId: string, expectedRevision: number, } | { "kind": "createConversation", contactId: string, title: string, } | { "kind": "openConversation", conversationId: string, } | { "kind": "updateConversation", conversationId: string, expectedRevision: number, title: string, archived: boolean, } | { "kind": "updateSettings", conversationId: string, expectedRevision: number, settings: PracticeSettings, } | { "kind": "deleteConversation", conversationId: string, expectedRevision: number, } | { "kind": "updateLearner", expectedRevision: number, name: string, preferences: Preferences, };
export type Command = { sessionId: string, actionId: string, action: Action, };
export type Receipt = { actionId: string, entityId: string, revision: number, };
export type ErrorCode = "validation" | "conflict" | "not_found" | "session_expired" | "storage" | "provider" | "admission_held" | "pending_turn" | "config_load" | "unknown_outcome" | "credential" | "internal";
export type Refusal = { reason: RefusalReason, serviceWide: boolean, retryAt: number | null, requestId: string | null, };
export type StarterGreeting = { text: string, romanized: string | null, };
export type LanguagePartner = { name: string, romanizedName: string | null, vibe: Array<string>, };
export type TranscriptionAttempt = { diagnostics?: unknown, id: string, route: ConnectionRoute, model: string, state: string, startedAt: string, finishedAt: string | null, error: string | null, };
export type TranscriptionInspectionResult = { text: string, inspection: AudioInspection, audioBase64: string, diagnostics: unknown | null, };
export type AudioInspection = { recordingId: string, owner: RecordingOwner, duration: number, sampleRate: number, waveform: InspectionWaveform, spectrogram: InspectionSpectrogram, activity: InspectionActivity, wordTiming: InspectionWordTiming, };
export type RecordingOwner = { "kind": "conversation", "id": string } | { "kind": "drillItem", "id": string };
export type ConversationDrillInput = { scope: ReadingScope, cursor: string | null, limit: number, };
export type ConversationDrillPage = { preview: DrillGenerationPreview, nextCursor: string | null, };
export type DrillLength = "word" | "shortPhrase" | "sentence" | "severalSentences";
export type DrillGenerationInput = { skillTarget?: DrillSkillTarget, language: string, variety: string | null, explanation: string, explanationVariety: string | null, topic: string | null, count: number, difficulty: Difficulty, length: DrillLength, };
export type DrillSkillTarget = { "kind": "skill", skillId: string, } | { "kind": "coach", mode: RecommendationMode, };
export type DrillSkillFocus = { skill: SkillPrompt, recommendation: Recommendation | null, };
export type SkillPrompt = { id: string, name: string, overview: string, boundary: string, language_guidance: string, };
export type Candidate = { skillId: string, experience: number, effort: number, };
export type Recommendation = { policy: string, requested: RecommendationMode, selected: RecommendationMode, skill: Candidate, eligibleSkills: number, };
export type DrillSource = { "kind": "own" } | { "kind": "bundled", set: PracticeSet, contentHash: string, } | { "kind": "generated", skillFocus?: DrillSkillFocus, requestId: string, candidateId: string, topic: string | null, difficulty: Difficulty, length: DrillLength, } | { "kind": "conversation", sourceRef: DrillConversationRef, revision: string, };
export type DrillConversationRef = { conversationId: string, messageId: string, turnId: string, role: string, startByte: number, endByte: number, };
export type DrillReportedLabels = { difficulty: string | null, tags: Array<string>, };
export type DrillVerifiedProperties = { scopeMatchesRequest: boolean, lengthOk: boolean, nonEmpty: boolean, duplicate: boolean, };
export type DrillCandidate = { candidateId: string, text: string, translation: string | null, reported: DrillReportedLabels, verified: DrillVerifiedProperties, source: DrillSource, };
export type DrillShortfall = { requested: number, produced: number, reason: string, };
export type DrillGenerationPreview = { requested: DrillGenerationInput | null, requestId: string, candidates: Array<DrillCandidate>, shortfall: DrillShortfall | null, receiptId: string | null, };
export type DrillItemInput = { text: string, language: string, variety: string | null, explanation: string, explanationVariety: string | null, };
export type PracticeSet = "absolute_zero" | "beginner" | "intermediate" | "advanced" | "social" | "idiomatic";
export type PracticeSetSummary = { set: PracticeSet, count: number, sample: string, };
export type DrillStorageView = { limitMb: number, recordingBytes: number, pendingRemovalBytes: number, };
export type DrillSessionView = { id: string, language: string, startedAt: string, endedAt: string | null, endReason: string | null, visits: Array<DrillVisitView>, };
export type DrillVisitView = { id: string, itemId: string, enteredAt: string, leftAt: string | null, attempts: bigint, exactMatches: bigint, };
export type DrillItemView = { source: DrillSource, id: string, text: string, language: string, variety: string, explanation: string, explanationVariety: string, createdAt: string, attemptCount: number, bestMatchRatio: number | null, lastAttemptAt: string | null, attempts: Array<DrillAttemptView>, };
export type DrillAttemptPage = { attempts: Array<DrillAttemptView>, nextCursor: string | null, };
export type DrillAttemptView = { id: string, sequence: bigint, visitId: string | null, transcript: string, comparison: DrillComparison, audioBytes: bigint | null, audioPrunedAt: string | null, transcriptionAttemptId: string | null, countedAsPractice: boolean, createdAt: string, };
export type DrillComparison = { policy: string, reliability?: DrillReliability, target: string, transcript: string, normalizations: Array<string>, normalizedTarget: string, normalizedTranscript: string, edits: number, referenceGraphemes: number, characterErrorRate: number | null, matchRatio: number | null, words: Array<WordComparison>, scriptNote: ScriptNote, };
export type DrillReliability = { policy: number, accepted: boolean, confidence: number | null, minimumConfidence: number, speechSeconds: number, noSpeechProbability: number | null, source: string, reason: string, };
export type WordComparison = { kind: WordOutcome, target: string | null, transcript: string | null, similarity: number | null, };
export type WordOutcome = "same" | "substituted" | "missing" | "extra";
export type ScriptNote = "matches" | "mismatch" | "unknown";
export type InspectionWaveform = { binSeconds: number, min: Array<number>, max: Array<number>, };
export type InspectionSpectrogram = { frameSeconds: number, frameStartSeconds: Array<number>, windowSeconds: number, fftSize: number, bands: Array<InspectionMelBand>, minFrequencyHz: number, maxFrequencyHz: number, measuredMaxFrequencyHz: number, melScale: string, normalization: string, dbReference: string, dbMin: number, dbMax: number, bins: Array<Array<number | null>>, };
export type InspectionMelBand = { lowHz: number, centerHz: number, highHz: number, };
export type InspectionActivity = { algorithm: string, noiseFloorDbfs: number, thresholdDbfs: number, regions: Array<InspectionRegion>, pauses: Array<InspectionPause>, limitations: Array<string>, };
export type InspectionRegion = { start: number, end: number, };
export type InspectionPause = { start: number, duration: number, };
export type InspectionWordTiming = { status: InspectionTimingStatus, reason: string | null, words: Array<InspectionWord>, unsupported: Array<InspectionUnsupportedWord>, };
export type InspectionWord = { index: number, word: string, providerStart: number, providerEnd: number, start: number, end: number, clipped: boolean, };
export type InspectionUnsupportedWord = { index: number, word: string, providerStart: number, providerEnd: number, reason: string, };
export type InspectionTimingStatus = "available" | "unavailable";
export type RefusalReason = "rate_limit" | "daily_limit" | "spending_paused" | "unknown";
export type ReadingAid = "word_gloss" | "speech" | "translation" | "explanations" | "completions";
export type ReadingInput = {
/**
 * Optional conversation attribution captured by the reading surface.
 */
conversationId?: string, referenceItem?: string, text: string, language: string, variety: string | null, explanation: string, explanationVariety: string | null, aid: ReadingAid, };
export type ReadingResult = { gloss: WordGlossView | null, audioBase64: string | null, audioAlignment: SpeechAlignment | null, translation: string | null, explanations: ReplyExplanations | null, receipt: unknown, };
export type SkillGuideResult = { markdown: string, explanationLanguage: string, generated: boolean, context?: GuideContext, provenance: unknown, };
export type GuideReference = { language: string, variety: string, skillId: string, editionLanguage: string, fingerprint: string, };
export type GuideContext = { reference: GuideReference, subskills: Array<string>, examples: Array<string>, };
export type SpeechAudio = { audioBase64: string, alignment: SpeechAlignment | null, };
export type TranscriptionResult = { text: string, timing: TranscriptTiming | null, };
export type TranscriptTiming = { text: string, duration: number, words: Array<Word>, };
export type Word = { word: string, start: number, end: number, };
export type SpeechAlignment = { sourceText: string, original: CharacterAlignment | null, normalized: CharacterAlignment | null, };
export type CharacterAlignment = { characters: Array<string>, starts: Array<number>, ends: Array<number>, };
export type SavedGlossQuery = { scope: ReadingScope,
/**
 * Exact surfaces from the shared reading segmenter, including the passage.
 */
surfaces: Array<string>, };
export type SavedGlossSource = { sourceId: string, dictionary?: boolean, provenance?: SavedGlossProvenance, operationId: string | null, attemptId: string | null, scope: ReadingScope, text: string, segments: Array<GlossSegment>, };
export type SavedGlossProvenance = { packageId: string, version: string, sense: string, sources: Array<string>, review: string, };
export type CacheSettings = { capacityBytes: number, usedBytes: number, resultCount: number, };
export type AppError = { diagnostics?: unknown, code: ErrorCode, message: string, refusal: Refusal | null, };
export const PERSONA_LIMITS = { nameMax: 80, ageMin: 18, ageMax: 100, locationMax: 120, occupationMax: 120, backgroundMax: 2000, currentSituationMax: 600, mannerMax: 600, itemMax: 120, interestsMax: 12, opinionsMax: 12, factsMax: 12, booksMax: 8, moviesMax: 8, quirksMax: 8, vibeMin: 2, vibeMax: 4, briefMax: 200 } as const
export const SKILL_CATALOG_VERSION = 1646978996 as const
export const TEXT_SIZE = { default: 85, min: 75, max: 160, step: 5 } as const
export const DEFAULT_EXECUTION: ExecutionPreferences = {"assessment":"automatic","replyBrief":"on_demand","reading":"automatic"}
export const DEFAULT_APPEARANCE: AppearancePreferences = {"palette":"cool","motion":"system"}
export const DIFFICULTY_LEVELS: readonly Difficulty[] = ["absolute_zero","beginner","intermediate","advanced","fluent"] as const
export const DRILL_RECORDING_MAX_MB = 100000 as const
export const DRILL_LENGTHS: readonly DrillLength[] = ["word","shortPhrase","sentence","severalSentences"] as const
export const CONTINUOUS_RECORDING_POLICY: ContinuousRecordingPolicy = {"version":4,"pauseOptionsMs":[600,1000,1500,2000,2500],"defaultPauseMs":1000,"silenceTimeoutOptionsMs":[5000,10000,15000,30000,60000],"defaultSilenceTimeoutMs":10000,"minThresholdDb":-80.0,"maxThresholdDb":-10.0,"defaultThresholdDb":-45.0,"minTakeOptionsMs":[160,300,600,1000],"defaultMinTakeMs":300,"maxPendingTakes":3,"maxTakeSeconds":30,"maxSessionSeconds":600,"maxTakes":100} as const
