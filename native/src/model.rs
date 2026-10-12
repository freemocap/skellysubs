use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    AbsoluteZero,
    Beginner,
    Intermediate,
    Advanced,
    Fluent,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HelpAmount {
    Minimal,
    Balanced,
    Generous,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CoachProactivity {
    OnRequest,
    Occasional,
    Frequent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PracticeSettings {
    pub difficulty: Difficulty,
    pub direction: crate::conversations::direction::ConversationDirection,
    pub explanation_language: String,
    pub variety_id: String,
    pub explanation_variety_id: String,
    pub composing_help: HelpAmount,
    pub coach_proactivity: CoachProactivity,
    pub translation: bool,
    pub pronunciation: bool,
    pub romanization: bool,
    pub auto_send: bool,
    pub read_aloud: bool,
    pub speech_voice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechUnavailableReason {
    NotRequested,
    Cancelled,
    Failed,
    UnknownOutcome,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SpeechAudioState {
    Streaming {
        operation_id: String,
        message_id: String,
        execution_id: String,
        sample_offset: u32,
        audio_base64: String,
        alignment: Option<crate::speech::alignment::SpeechAlignment>,
    },
    Pending {
        operation_id: String,
        message_id: String,
    },
    Ready {
        operation_id: String,
        attempt_id: String,
        message_id: String,
        mime: String,
        audio_base64: String,
        alignment: Option<crate::speech::alignment::SpeechAlignment>,
    },
    Unavailable {
        operation_id: String,
        message_id: String,
        reason: SpeechUnavailableReason,
        message: String,
        attempt_id: Option<String>,
        #[ts(type = "unknown")]
        diagnostics: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStatus {
    NotStarted,
    InProgress,
    Skipped,
    Completed,
}
/// Reading size, in percent. Rust owns these; the frontend reads them from the
/// generated contracts.
pub const TEXT_SIZE_DEFAULT: u16 = 85;
pub const TEXT_SIZE_MIN: u16 = 75;
pub const TEXT_SIZE_MAX: u16 = 160;
pub const TEXT_SIZE_STEP: u16 = 5;

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum PracticeView {
    #[default]
    Chat,
    Drill,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub execution: crate::configuration::execution::ExecutionPreferences,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub practice_view: Option<PracticeView>,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub appearance: crate::configuration::appearance::AppearancePreferences,
    pub explanation_language: String,
    pub explanation_variety_id: String,
    pub interface_locale: String,
    pub target_varieties: std::collections::BTreeMap<String, String>,
    // Per-language learner overrides; absent entries use bundled script defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub script_scales: Option<std::collections::BTreeMap<String, f64>>,
    // Learner-owned picker shortcuts; independent of conversations and history.
    #[serde(default)]
    pub my_languages: Vec<String>,
    pub text_size: u16,
    pub text_spacing: u8,
    pub high_contrast: bool,
    pub onboarding: OnboardingStatus,
    // Explicit new-workspace marker: old not_started values are not proof of first use.
    #[serde(default)]
    pub onboarding_required: bool,
    #[serde(default)]
    pub onboarding_language: Option<String>,
    // Optional guidance has an independent, durable dismissal state.
    #[serde(default)]
    pub onboarding_help: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Learner {
    pub id: String,
    pub name: String,
    pub revision: i32,
    pub preferences: Preferences,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LanguageProfile {
    pub id: String,
    pub learner_id: String,
    pub language_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonaDetails {
    pub name: String,
    /// The name in Latin letters, present exactly when the persona's language has
    /// a romanization system.
    pub romanized_name: Option<String>,
    /// Left blank until someone chooses one.
    pub age: Option<u8>,
    pub location: String,
    pub occupation: String,
    pub background: String,
    pub current_situation: String,
    pub interests: Vec<String>,
    pub opinions: Vec<String>,
    pub interesting_facts: Vec<String>,
    pub favorite_books: Vec<String>,
    pub favorite_movies: Vec<String>,
    pub manner: String,
    pub quirks: Vec<String>,
    pub vibe: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: String,
    pub learner_id: String,
    pub language_id: String,
    pub revision: i32,
    pub details: PersonaDetails,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    pub learner_id: String,
    pub persona_id: String,
    pub archived: bool,
    pub revision: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub contact_id: String,
    pub language_id: String,
    pub title: String,
    pub archived: bool,
    pub revision: i32,
    pub settings_revision: i32,
    pub settings: PracticeSettings,
    pub created_at: String,
    #[ts(type = "number")]
    pub last_used: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Variety {
    pub transcription_language: Option<String>,
    pub direction: String,
    pub font_scale: f64,
    pub romanization: Option<String>,
    pub id: String,
    pub name: String,
    pub description: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub transcription_language: Option<String>,
    pub language_tag: Option<String>,
    pub font_scale: f64,
    pub direction: String,
    pub romanization: Option<String>,
    pub id: String,
    pub name: String,
    pub native_name: String,
    pub varieties: Vec<Variety>,
    pub default_variety: String,
    /// The authored greeting, at this language's default variety. Carried on the
    /// catalog so choosing a language can show what saying it sounds like.
    pub greeting: StarterGreeting,
    /// Who the learner meets first in this language. Enough of the bundled
    /// starter persona to introduce them, not the whole profile.
    pub partner: LanguagePartner,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePartner {
    pub name: String,
    pub romanized_name: Option<String>,
    pub vibe: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub session_id: String,
    pub revision: i32,
    pub learner: Learner,
    pub languages: Vec<Language>,
    pub language_profiles: Vec<LanguageProfile>,
    pub personas: Vec<Persona>,
    pub contacts: Vec<Contact>,
    pub conversations: Vec<Conversation>,
    pub saved_topics: Vec<crate::conversations::direction::SavedTopic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    AskGuideCoach {
        conversation_id: String,
        text: String,
        guide: crate::configuration::guide_translation::GuideReference,
        #[serde(default)]
        #[ts(optional, inline)]
        focus: Option<crate::configuration::guide_translation::GuideCoachFocus>,
        expected_revision: i32,
    },
    StartGuideConversation {
        source_conversation_id: String,
        guide: crate::configuration::guide_translation::GuideReference,
        example: usize,
        #[serde(default)]
        #[ts(optional)]
        phrase: Option<String>,
        expected_revision: i32,
    },
    StartSkillConversation {
        source_conversation_id: String,
        language: String,
        variety: String,
        skill_id: String,
        subskill_id: Option<String>,
        expected_revision: i32,
    },
    StartPhraseConversation {
        source_message_id: String,
        phrase: String,
        contact_id: String,
        expected_revision: i32,
    },
    StartConversation {
        conversation_id: String,
        configuration: crate::conversations::direction::ConversationStartConfig,
        message: Option<String>,
        input: Option<crate::learning::coaching::InputEvidence>,
        expected_revision: i32,
    },
    UpdateConversationPrompt {
        conversation_id: String,
        configuration: crate::conversations::direction::ConversationStartConfig,
        additions: Vec<String>,
        deletions: Vec<String>,
        expected_revision: i32,
        expected_settings_revision: i32,
    },
    SaveTopics {
        additions: Vec<String>,
        deletions: Vec<String>,
        expected_revision: i32,
    },
    CoachControl {
        turn_id: String,
        control: crate::learning::coaching::CoachControl,
    },
    ReviseTurn {
        conversation_id: String,
        turn_id: String,
        text: String,
        input: crate::learning::coaching::InputEvidence,
        expected_revision: i32,
    },
    AskCoach {
        conversation_id: String,
        text: String,
        expected_revision: i32,
    },
    StartChat {
        language_id: String,
    },
    SendMessage {
        input: crate::learning::coaching::InputEvidence,
        conversation_id: String,
        text: String,
        expected_revision: i32,
    },
    RequestMessageSpeech {
        message_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        regenerate: Option<bool>,
    },
    CancelMessageSpeech {
        operation_id: String,
    },
    RequestSuggestions {
        message_id: String,
    },
    RequestMessageHelp {
        message_id: String,
        help: crate::conversations::execution::MessageHelp,
        retry: bool,
    },
    RequestExplanations {
        message_id: String,
    },
    ReassessFeedback {
        turn_id: String,
        note: String,
    },
    RetryReplyHelp {
        message_id: String,
        help_kind: crate::learning::coaching::conversation_support::ReplyHelpKind,
    },
    RetryGloss {
        operation_id: String,
    },
    ControlTurn {
        turn_id: String,
        control: TurnControl,
    },
    SetPaused {
        paused: bool,
    },
    CreateContact {
        language_id: String,
        details: PersonaDetails,
    },
    UpdatePersona {
        persona_id: String,
        expected_revision: i32,
        details: PersonaDetails,
    },
    SetContactArchived {
        contact_id: String,
        expected_revision: i32,
        archived: bool,
    },
    DeleteContact {
        contact_id: String,
        expected_revision: i32,
    },
    CreateConversation {
        contact_id: String,
        title: String,
    },
    OpenConversation {
        conversation_id: String,
    },
    UpdateConversation {
        conversation_id: String,
        expected_revision: i32,
        title: String,
        archived: bool,
    },
    UpdateSettings {
        conversation_id: String,
        expected_revision: i32,
        settings: PracticeSettings,
    },
    DeleteConversation {
        conversation_id: String,
        expected_revision: i32,
    },
    UpdateLearner {
        expected_revision: i32,
        name: String,
        preferences: Preferences,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    pub session_id: String,
    pub action_id: String,
    pub action: Action,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub action_id: String,
    pub entity_id: String,
    pub revision: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Validation,
    Conflict,
    NotFound,
    SessionExpired,
    Storage,
    Provider,
    AdmissionHeld,
    PendingTurn,
    ConfigLoad,
    UnknownOutcome,
    Credential,
    Internal,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub diagnostics: Option<serde_json::Value>,
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub refusal: Option<Refusal>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    pub reason: RefusalReason,
    pub service_wide: bool,
    pub retry_at: Option<f64>,
    pub request_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    RateLimit,
    DailyLimit,
    SpendingPaused,
    Unknown,
}
impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            refusal: None,
            diagnostics: None,
        }
    }
    pub fn with_diagnostics(mut self, value: serde_json::Value) -> Self {
        let limited = crate::ai::policy::rate_limit::detected(&value);
        self.diagnostics = Some(value);
        if limited
            && !self.message.starts_with("Sorry, rate limited.")
            && matches!(
                self.code,
                ErrorCode::Provider | ErrorCode::UnknownOutcome | ErrorCode::AdmissionHeld
            )
        {
            self.message = format!("Sorry, rate limited. Try again shortly. {}", self.message);
        }
        self
    }
    pub fn with_refusal(mut self, refusal: Refusal) -> Self {
        if matches!(refusal.reason, RefusalReason::RateLimit)
            && !self.message.starts_with("Sorry, rate limited.")
        {
            self.message = format!("Sorry, rate limited. Try again shortly. {}", self.message);
        }
        self.refusal = Some(refusal);
        self
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
impl From<rusqlite::Error> for AppError {
    #[track_caller]
    fn from(error: rusqlite::Error) -> Self {
        crate::diagnostics::failures::sqlite(&error)
    }
}
impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        crate::diagnostics::response::json_context(
            &error,
            "stored_json_decode",
            Self::new(ErrorCode::Storage, "Stored JSON could not be decoded."),
        )
    }
}
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStarted {
    pub device_label: Option<String>,
    pub recording_id: String,
    pub samples_per_second: f64,
    pub browser_capture: bool,
    // The browser device id to record from when `browser_capture` is set;
    // `None` means the system default. Desktop capture opens its device natively.
    pub browser_device_id: Option<String>,
}

/// What the window needs before it can mount: whether the workspace opened, and
/// whether a cleanup an earlier reset recorded still could not be finished.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartupState {
    /// Set when the workspace could not be opened, so the shell cannot mount.
    pub refusal: Option<AppError>,
    /// Set when directories a previous reset recorded still could not be cleared.
    /// The app is usable; that leftover data is not cleared.
    pub cleanup: Option<AppError>,
    // A stale keychain entry still needs deletion. The healthy workspace remains usable.
    pub credential_cleanup: Option<AppError>,
}

/// The limits the persona editor validates against, generated so the frontend
/// cannot drift from the rules the store enforces.
fn persona_limits() -> String {
    use crate::partners::persona::*;
    format!(
        "export const PERSONA_LIMITS = {{ nameMax: {NAME_MAX}, ageMin: {AGE_MIN}, ageMax: {AGE_MAX}, locationMax: {LOCATION_MAX}, occupationMax: {OCCUPATION_MAX}, backgroundMax: {BACKGROUND_MAX}, currentSituationMax: {CURRENT_SITUATION_MAX}, mannerMax: {MANNER_MAX}, itemMax: {ITEM_MAX}, interestsMax: {INTERESTS_MAX}, opinionsMax: {OPINIONS_MAX}, factsMax: {FACTS_MAX}, booksMax: {BOOKS_MAX}, moviesMax: {MOVIES_MAX}, quirksMax: {QUIRKS_MAX}, vibeMin: {VIBE_MIN}, vibeMax: {VIBE_MAX}, briefMax: {BRIEF_MAX} }} as const"
    )
}

/// The reading-size range and default, generated so the shortcut, the reset and
/// the store agree.
fn text_size_limits() -> String {
    format!(
        "export const TEXT_SIZE = {{ default: {TEXT_SIZE_DEFAULT}, min: {TEXT_SIZE_MIN}, max: {TEXT_SIZE_MAX}, step: {TEXT_SIZE_STEP} }} as const"
    )
}

pub fn bindings() -> String {
    let config = ts_rs::Config::default();
    let declarations = [
        RecordingStarted::decl(&config),
        crate::speech::recording::microphone_test::MicrophoneTestStarted::decl(&config),
        crate::speech::recording::continuous_policy::ContinuousRecordingPolicy::decl(&config),
        crate::speech::recording::continuous_policy::ListeningSettings::decl(&config),
        crate::speech::recording::continuous_policy::ListeningMode::decl(&config),
        crate::speech::recording::continuous::ListeningStatus::decl(&config),
        crate::speech::recording::continuous::ListeningTake::decl(&config),
        crate::speech::analysis::spectrogram::LiveSpectrogram::decl(&config),
        crate::speech::recording::continuous::ListeningTakeState::decl(&config),
        Theme::decl(&config),
        crate::learning::rewards::RewardEvent::decl(&config),
        crate::learning::rewards::skill_level_events::SkillLevelEventKind::decl(&config),
        crate::learning::rewards::skill_level_events::SkillLevelEvent::decl(&config),
        crate::learning::learner::skill_levels::SkillLevelProgress::decl(&config),
        crate::learning::learner::skill_levels::SkillLevelSummary::decl(&config),
        crate::learning::effort::EffortDimension::decl(&config),
        crate::learning::effort::EffortAward::decl(&config),
        crate::learning::effort::EffortProgress::decl(&config),
        crate::learning::streaks::commands::StreakCounter::decl(&config),
        crate::learning::streaks::commands::StreakSnapshot::decl(&config),
        crate::learning::learner::progression::LanguageTotals::decl(&config),
        crate::learning::effort::report::EffortActivity::decl(&config),
        crate::learning::effort::report::EffortHistoryEntry::decl(&config),
        crate::learning::effort::report::EffortReport::decl(&config),
        crate::learning::learner::learner_state::LearnerState::decl(&config),
        crate::learning::learner::learner_state::ConstructState::decl(&config),
        ConnectionRoute::decl(&config),
        crate::configuration::appearance::SurfacePalette::decl(&config),
        crate::configuration::appearance::MotionPreference::decl(&config),
        crate::configuration::appearance::AppearancePreferences::decl(&config),
        crate::configuration::execution::ExecutionMode::decl(&config),
        crate::configuration::execution::ExecutionPreferences::decl(&config),
        crate::conversations::execution::MessageHelp::decl(&config),
        AccessSettings::decl(&config),
        ProviderCredentialCheck::decl(&config),
        AccessCheck::decl(&config),
        CustomEndpoint::decl(&config),
        HostedAccount::decl(&config),
        UsageSummary::decl(&config),
        ProfileSnapshot::decl(&config),
        PersonaGenerationAttempt::decl(&config),
        PersonaGenerationUsage::decl(&config),
        PersonaGenerationActivity::decl(&config),
        AudioModelSettings::decl(&config),
        AudioSettings::decl(&config),
        AssessmentAdapter::decl(&config),
        ConnectionConfig::decl(&config),
        TurnControl::decl(&config),
        GlossSegmentKind::decl(&config),
        GlossCoverage::decl(&config),
        GlossSegment::decl(&config),
        WordGlossView::decl(&config),
        crate::diagnostics::DiagnosticCommand::decl(&config),
        crate::learning::rewards::reward_settings::RewardSettings::decl(&config),
        crate::speech::recording::microphone::MicrophoneSource::decl(&config),
        crate::speech::recording::microphone::MicrophoneDevice::decl(&config),
        crate::speech::recording::microphone::MicrophoneList::decl(&config),
        crate::learning::coaching::InputEvidence::decl(&config),
        crate::learning::coaching::Outcome::decl(&config),
        RevisionSuffixCount::decl(&config),
        crate::learning::coaching::MeaningLevel::decl(&config),
        crate::learning::coaching::ErrorOp::decl(&config),
        crate::learning::coaching::ErrorSource::decl(&config),
        crate::learning::coaching::ErrorTag::decl(&config),
        crate::learning::coaching::ObservedItem::decl(&config),
        crate::learning::coaching::CoachObservation::decl(&config),
        crate::learning::coaching::ObservedItemSummary::decl(&config),
        crate::learning::coaching::CoachObservationView::decl(&config),
        crate::learning::coaching::CoachIssue::decl(&config),
        crate::learning::coaching::CoachIssueSeverity::decl(&config),
        crate::learning::coaching::CoachMove::decl(&config),
        crate::learning::coaching::Correction::decl(&config),
        crate::learning::coaching::CoachDecision::decl(&config),
        crate::learning::coaching::CoachControl::decl(&config),
        crate::partners::partner_reaction::PartnerReaction::decl(&config),
        crate::partners::partner_reaction::ReactionKind::decl(&config),
        Opening::decl(&config),
        crate::conversations::direction::TopicCard::decl(&config),
        crate::conversations::direction::SavedTopic::decl(&config),
        crate::conversations::direction::TimeReference::decl(&config),
        crate::learning::recommendations::RecommendationMode::decl(&config),
        crate::conversations::direction::TopicChoice::decl(&config),
        crate::conversations::direction::ConversationDirection::decl(&config),
        crate::conversations::direction::ConversationStartConfig::decl(&config),
        crate::conversations::direction::PromptPreview::decl(&config),
        crate::conversations::direction::CoachFocusPreview::decl(&config),
        crate::learning::coaching::SuggestedReply::decl(&config),
        crate::learning::coaching::conversation_support::ConversationFeedback::decl(&config),
        crate::learning::coaching::message_assessment::ChoiceAssessment::decl(&config),
        crate::learning::coaching::conversation_support::AssistedReply::decl(&config),
        crate::learning::coaching::conversation_support::ReplyAssistance::decl(&config),
        crate::learning::coaching::conversation_support::ReplyBrief::decl(&config),
        crate::learning::coaching::conversation_support::ReplyHelpKind::decl(&config),
        crate::language::reading::ReadingScope::decl(&config),
        crate::learning::coaching::conversation_support::ReplyExplanation::decl(&config),
        crate::learning::coaching::conversation_support::ReplyExplanations::decl(&config),
        ChatMessage::decl(&config),
        SpeechRequestView::decl(&config),
        TurnView::decl(&config),
        TurnHistoryPage::decl(&config),
        RecordedMessage::decl(&config),
        AiViewSelection::decl(&config),
        AiElsewhereSelection::decl(&config),
        AiRevisionSelection::decl(&config),
        crate::ai::inspection::NativeAttemptInspection::decl(&config),
        crate::ai::content::ContentRecord::decl(&config),
        crate::ai::content::ContentPayload::decl(&config),
        crate::ai::inspection::NativeRunEntry::decl(&config),
        crate::ai::inspection::NativeRunPage::decl(&config),
        AiWindowState::decl(&config),
        ConversationSnapshot::decl(&config),
        crate::conversations::message_history::MessageHistory::decl(&config),
        crate::conversations::message_history::MessageVersionView::decl(&config),
        crate::conversations::message_history::VersionAssessmentState::decl(&config),
        Difficulty::decl(&config),
        HelpAmount::decl(&config),
        CoachProactivity::decl(&config),
        PracticeSettings::decl(&config),
        SpeechUnavailableReason::decl(&config),
        SpeechAudioState::decl(&config),
        OnboardingStatus::decl(&config),
        PracticeView::decl(&config),
        Preferences::decl(&config),
        Learner::decl(&config),
        LanguageProfile::decl(&config),
        PersonaDetails::decl(&config),
        Persona::decl(&config),
        Contact::decl(&config),
        Conversation::decl(&config),
        Variety::decl(&config),
        Language::decl(&config),
        crate::configuration::LanguageInspection::decl(&config),
        crate::configuration::guides::GuideInspection::decl(&config),
        crate::configuration::guides::TeachingGuide::decl(&config),
        crate::configuration::guides::GuideSection::decl(&config),
        crate::configuration::guides::GuideExample::decl(&config),
        crate::configuration::guides::GuideVariant::decl(&config),
        crate::configuration::ContentSource::decl(&config),
        crate::configuration::ContentRule::decl(&config),
        crate::configuration::ContentValue::decl(&config),
        crate::configuration::SchemeInspection::decl(&config),
        Snapshot::decl(&config),
        StartupState::decl(&config),
        Action::decl(&config),
        Command::decl(&config),
        Receipt::decl(&config),
        ErrorCode::decl(&config),
        Refusal::decl(&config),
        StarterGreeting::decl(&config),
        LanguagePartner::decl(&config),
        TranscriptionAttempt::decl(&config),
        crate::speech::analysis::audio_inspection::TranscriptionInspectionResult::decl(&config),
        crate::speech::analysis::audio_inspection::AudioInspection::decl(&config),
        crate::speech::recording::owner::RecordingOwner::decl(&config),
        crate::drill::conversation_source::ConversationDrillInput::decl(&config),
        crate::drill::conversation_source::ConversationDrillPage::decl(&config),
        crate::drill::generation::DrillLength::decl(&config),
        crate::drill::generation::DrillGenerationInput::decl(&config),
        crate::drill::skill_focus::DrillSkillTarget::decl(&config),
        crate::drill::skill_focus::DrillSkillFocus::decl(&config),
        crate::learning::practice_assessment::SkillPrompt::decl(&config),
        crate::learning::recommendations::Candidate::decl(&config),
        crate::learning::recommendations::Recommendation::decl(&config),
        crate::drill::previews::DrillSource::decl(&config),
        crate::drill::previews::DrillConversationRef::decl(&config),
        crate::drill::previews::DrillReportedLabels::decl(&config),
        crate::drill::previews::DrillVerifiedProperties::decl(&config),
        crate::drill::previews::DrillCandidate::decl(&config),
        crate::drill::previews::DrillShortfall::decl(&config),
        crate::drill::previews::DrillGenerationPreview::decl(&config),
        crate::drill::DrillItemInput::decl(&config),
        crate::configuration::practice::PracticeSet::decl(&config),
        crate::configuration::practice::PracticeSetSummary::decl(&config),
        crate::drill::retention::DrillStorageView::decl(&config),
        crate::drill::sessions::DrillSessionView::decl(&config),
        crate::drill::sessions::DrillVisitView::decl(&config),
        crate::drill::DrillItemView::decl(&config),
        crate::drill::history::DrillAttemptPage::decl(&config),
        crate::drill::DrillAttemptView::decl(&config),
        crate::drill::comparison::DrillComparison::decl(&config),
        crate::drill::reliability::DrillReliability::decl(&config),
        crate::drill::comparison::WordComparison::decl(&config),
        crate::drill::comparison::WordOutcome::decl(&config),
        crate::drill::comparison::ScriptNote::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionWaveform::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionSpectrogram::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionMelBand::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionActivity::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionRegion::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionPause::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionWordTiming::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionWord::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionUnsupportedWord::decl(&config),
        crate::speech::analysis::audio_inspection::InspectionTimingStatus::decl(&config),
        RefusalReason::decl(&config),
        crate::language::reading::ReadingAid::decl(&config),
        crate::language::reading::ReadingInput::decl(&config),
        crate::language::reading::ReadingResult::decl(&config),
        crate::configuration::guide_translation::SkillGuideResult::decl(&config),
        crate::configuration::guide_translation::GuideReference::decl(&config),
        crate::configuration::guide_translation::GuideContext::decl(&config),
        crate::speech::alignment::SpeechAudio::decl(&config),
        crate::ai::audio::TranscriptionResult::decl(&config),
        crate::speech::analysis::fluency::TranscriptTiming::decl(&config),
        crate::speech::analysis::fluency::Word::decl(&config),
        crate::speech::alignment::SpeechAlignment::decl(&config),
        crate::speech::alignment::CharacterAlignment::decl(&config),
        crate::language::reading::saved::SavedGlossQuery::decl(&config),
        crate::language::reading::saved::SavedGlossSource::decl(&config),
        crate::language::reading::saved::SavedGlossProvenance::decl(&config),
        crate::ai::results::CacheSettings::decl(&config),
        AppError::decl(&config),
    ];
    format!(
        "// Generated from Rust contracts. Run npm run contracts.\n{}\n{}\n{}\n{}\n",
        format_args!(
            "export const diagnosticCommands = {} as const;",
            serde_json::to_string(crate::diagnostics::DIAGNOSTIC_COMMAND_NAMES)
                .expect("command names")
        ),
        declarations
            .map(|line| format!(
                "export {}",
                line.lines()
                    .map(str::trim_end)
                    .collect::<Vec<_>>()
                    .join("\n")
            ))
            .join("\n"),
        format_args!(
            "{}\nexport const SKILL_CATALOG_VERSION = {} as const",
            persona_limits(),
            crate::learning::coaching::catalog_version()
        ),
        format_args!(
            "{}\nexport const DEFAULT_EXECUTION: ExecutionPreferences = {}\nexport const DEFAULT_APPEARANCE: AppearancePreferences = {}\nexport const DIFFICULTY_LEVELS: readonly Difficulty[] = {} as const\nexport const DRILL_RECORDING_MAX_MB = {} as const\nexport const DRILL_LENGTHS: readonly DrillLength[] = {} as const\nexport const CONTINUOUS_RECORDING_POLICY: ContinuousRecordingPolicy = {} as const",
            text_size_limits(),
            serde_json::to_string(
                &crate::configuration::execution::ExecutionPreferences::default()
            )
            .expect("execution defaults serialize"),
            serde_json::to_string(
                &crate::configuration::appearance::AppearancePreferences::default()
            )
            .expect("appearance defaults serialize"),
            serde_json::to_string(&crate::configuration::difficulty::LEVELS)
                .expect("difficulty levels serialize"),
            crate::drill::retention::MAX_LIMIT_MB,
            serde_json::to_string(&crate::drill::generation::LENGTHS)
                .expect("drill lengths serialize"),
            serde_json::to_string(&crate::speech::recording::continuous_policy::POLICY)
                .expect("continuous policy serializes")
        )
    )
}

/// Audio models share the single connection access route.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioModelSettings {
    pub model: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioSettings {
    pub transcription: AudioModelSettings,
    pub speech: AudioModelSettings,
}
/// Assessment strategy, independent of the access route and chat model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentAdapter {
    JevChoice,
    ChatModel,
}
impl AssessmentAdapter {
    pub fn label(self) -> &'static str {
        match self {
            Self::JevChoice => "jev_choice",
            Self::ChatModel => "chat_model",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub assessment_adapter: AssessmentAdapter,
    pub route: ConnectionRoute,
    pub signed_in: bool,
    pub email: String,
    pub revision: i32,
    pub configured: bool,
    pub standard_model: String,
    pub fast_model: String,
    pub audio: AudioSettings,
    pub paused: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TurnControl {
    Pause,
    Resume,
    Step,
    Cancel,
    Retry,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GlossSegmentKind {
    Gloss,
    Literal,
    Unresolved,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GlossCoverage {
    Complete,
    Partial,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GlossSegment {
    pub start: u32,
    pub end: u32,
    pub kind: GlossSegmentKind,
    pub gloss: Option<String>,
    #[ts(optional)]
    pub romanization: Option<String>,
    #[ts(optional)]
    pub pronunciation: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WordGlossView {
    pub source_message_id: String,
    pub target_language_id: String,
    pub explanation_language_id: String,
    pub format_version: String,
    pub template_version: String,
    pub boundary_policy: String,
    pub operation_id: String,
    pub attempt_id: String,
    pub coverage: GlossCoverage,
    pub segments: Vec<GlossSegment>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    #[ts(optional)]
    pub assessment_state: Option<String>,
    #[ts(optional)]
    pub assessment_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub guide_context: Option<serde_json::Value>,
    #[ts(optional)]
    pub feedback_context: Option<String>,
    #[ts(optional)]
    pub conversation_feedback:
        Option<crate::learning::coaching::conversation_support::ConversationFeedback>,
    #[ts(optional)]
    pub reply_brief: Option<crate::learning::coaching::conversation_support::ReplyBrief>,
    #[ts(optional)]
    pub brief_state: Option<String>,
    #[ts(optional)]
    pub brief_error: Option<String>,
    #[ts(optional)]
    pub reading_scope: Option<crate::language::reading::ReadingScope>,
    #[ts(optional)]
    pub reply_assistance: Option<crate::learning::coaching::conversation_support::ReplyAssistance>,
    #[ts(optional)]
    pub reply_explanations:
        Option<crate::learning::coaching::conversation_support::ReplyExplanations>,
    #[ts(optional)]
    pub explanations_state: Option<String>,
    #[ts(optional)]
    pub explanations_error: Option<String>,
    #[ts(optional)]
    pub reaction: Option<crate::partners::partner_reaction::PartnerReaction>,
    #[ts(optional)]
    pub reaction_error: Option<String>,
    pub coach_decision: Option<crate::learning::coaching::CoachDecision>,
    pub turn_id: String,
    pub replaces_turn_id: Option<String>,
    pub replaced_by: Option<String>,
    #[ts(optional)]
    pub feedback: Option<crate::learning::coaching::CoachObservationView>,
    #[ts(optional)]
    pub feedback_state: Option<String>,
    #[ts(optional)]
    pub feedback_error: Option<String>,
    #[ts(optional)]
    pub suggested_replies: Option<Vec<crate::learning::coaching::SuggestedReply>>,
    #[ts(optional)]
    pub suggestions_state: Option<String>,
    #[ts(optional)]
    pub suggestions_error: Option<String>,
    pub word_gloss: Option<WordGlossView>,
    pub gloss_state: Option<String>,
    pub gloss_error: Option<String>,
    pub gloss_operation_id: Option<String>,
    pub translation_state: Option<String>,
    pub translation: Option<String>,
    pub id: String,
    pub sequence: i32,
    pub role: String,
    pub text: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SpeechRequestView {
    pub source_message_id: String,
    pub id: String,
    pub state: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TurnView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recording_ids: Option<Vec<String>>,
    /// Structural product-field associations, not value/attempt provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native_surfaces: Option<std::collections::BTreeMap<String, Vec<String>>>,
    #[ts(optional)]
    pub award_sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub native_response: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "import('./graph-contracts').AttemptPreview")]
    pub native_preview: Option<crate::ai::graph::AttemptPreview>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "import('./graph-contracts').InspectionSnapshot")]
    pub native_graph: Option<crate::ai::graph::InspectionSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native_execution_available: Option<bool>,
    pub replaces_turn_id: Option<String>,
    pub replaced_by: Option<String>,
    pub route: ConnectionRoute,
    pub id: String,
    pub state: String,
    pub paused: bool,
    pub hold: Option<AppError>,
    #[ts(optional)]
    pub speech: Option<SpeechRequestView>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AiWindowState {
    pub supported: bool,
    pub open: bool,
}
/// What the AI View is looking at, handed between the docked panel and the
/// popped-out window so neither loses the learner's place. `turn_id` is None
/// while the view follows the newest turn. `operation_kind` retains its wire
/// name but carries the executable graph's operation identity.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiViewSelection {
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub operation_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub elsewhere: Option<AiElsewhereSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub revision: Option<AiRevisionSelection>,
}
/// Presentation selection only; retained in application memory across windows.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AiElsewhereSelection {
    Workspace {
        entry: crate::ai::inspection::NativeRunEntry,
    },
    Transcription {
        id: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AiRevisionSelection {
    Live,
    Structure,
    Revision { revision: String },
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecordedMessage {
    pub role: String,
    pub content: String,
}
/// A page of a conversation's turns, newest first, keyed by turn rather than
/// by message so every turn stays reachable, including coach turns and turns
/// that never produced a message.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TurnHistoryPage {
    pub turns: Vec<TurnView>,
    pub has_older: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSnapshot {
    /// Ephemeral display identity, scoped by session, conversation and page.
    /// Navigation may advance revision while retaining identical display fields.
    pub content_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub phrase_seed: Option<String>,
    pub topic_choices: Vec<crate::conversations::direction::TopicCard>,
    /// The greeting the start surface offers as a first thing to say, resolved
    /// for this conversation's target language and variety.
    pub starter_greeting: StarterGreeting,
    pub opening: Option<Opening>,
    pub revision_suffix_counts: Vec<RevisionSuffixCount>,
    pub transcription_attempts: Vec<TranscriptionAttempt>,
    pub coach_messages: Vec<ChatMessage>,
    pub conversation_id: String,
    pub session_id: String,
    pub revision: i32,
    pub messages: Vec<ChatMessage>,
    pub turns: Vec<TurnView>,
    pub connection: ConnectionConfig,
    pub has_older: bool,
}
/// Authored display content: one canonical greeting per language, with the
/// transliteration for the variety in force.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StarterGreeting {
    pub text: String,
    pub romanized: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionAttempt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub diagnostics: Option<serde_json::Value>,
    pub id: String,
    pub route: ConnectionRoute,
    pub model: String,
    pub state: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionRoute {
    Hosted,
    Custom,
}
impl ConnectionRoute {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hosted => "hosted",
            Self::Custom => "custom",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "hosted" => Ok(Self::Hosted),
            "custom" => Ok(Self::Custom),
            _ => Err(AppError::new(ErrorCode::Storage, "Unknown AI route.")),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
#[ts(rename_all = "camelCase")]
pub struct HostedAccount {
    pub email: String,
    pub name: String,
    pub used_usd: f64,
    pub limit_usd: f64,
    pub remaining_usd: f64,
    pub tokens_today: u32,
    pub requests_today: u32,
    #[serde(rename(
        deserialize = "estimated_turns_remaining",
        serialize = "estimatedRequestsRemaining"
    ))]
    #[ts(rename = "estimatedRequestsRemaining")]
    pub estimated_requests_remaining: u32,
    pub estimated_tokens_remaining: u32,
    pub custom_limit: bool,
    pub resets: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub id: String,
    pub label: String,
    pub conversations: i32,
    pub learner_messages: i32,
    pub persona_messages: i32,
    pub attempts: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub unknown_usage: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSnapshot {
    pub revision: i32,
    pub global: UsageSummary,
    pub languages: Vec<UsageSummary>,
    pub personas: Vec<UsageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonaGenerationAttempt {
    #[serde(default)]
    #[ts(optional)]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub diagnostics: Option<serde_json::Value>,
    pub id: String,
    pub attempt_id: String,
    pub operation_id: String,
    pub language_id: String,
    pub route: ConnectionRoute,
    pub requested_model: String,
    pub profile_revision: i32,
    pub state: String,
    pub created_at: String,
    pub dispatched_at: Option<String>,
    pub finished_at: Option<String>,
    pub actual_model: Option<String>,
    pub provider_id: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersonaGenerationUsage {
    pub attempts: i32,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub unknown_usage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonaGenerationActivity {
    pub revision: i32,
    pub attempts: Vec<PersonaGenerationAttempt>,
    pub usage: PersonaGenerationUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomEndpoint {
    pub base_url: String,
    pub bearer_auth: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccessSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub credential_previews: Option<std::collections::BTreeMap<String, String>>,
    pub custom_url_is_unsaved_default: bool,
    pub revision: i32,
    pub custom_key_configured: bool,
    pub custom: CustomEndpoint,
}

#[cfg(test)]
mod difficulty_tests {
    use super::*;

    #[test]
    fn five_levels_round_trip_and_previous_values_are_rejected() {
        for (level, wire) in [
            (Difficulty::AbsoluteZero, "absolute_zero"),
            (Difficulty::Beginner, "beginner"),
            (Difficulty::Intermediate, "intermediate"),
            (Difficulty::Advanced, "advanced"),
            (Difficulty::Fluent, "fluent"),
        ] {
            let encoded = serde_json::to_value(&level).unwrap();
            assert_eq!(encoded, serde_json::json!(wire));
            assert_eq!(
                serde_json::from_value::<Difficulty>(encoded).unwrap(),
                level
            );
        }
        for previous in ["gentle", "balanced", "challenging", "zero"] {
            let mut settings = serde_json::to_value(
                crate::language::languages::defaults("spanish", "english").unwrap(),
            )
            .unwrap();
            settings["difficulty"] = serde_json::json!(previous);
            assert!(serde_json::from_value::<PracticeSettings>(settings).is_err());
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSuffixCount {
    pub turn_id: String,
    pub exchange_count: i32,
    pub coach_turn_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Opening {
    Learner,
    Partner,
}
#[cfg(test)]
mod appearance_tests {
    use super::*;
    #[test]
    fn appearance_defaults_and_rejects_unknown_values() {
        let original = serde_json::json!({"execution":crate::configuration::execution::ExecutionPreferences::default(),"explanationLanguage":"english","explanationVarietyId":"english-united-states","interfaceLocale":"english","targetVarieties":{},"textSize":100,"textSpacing":0,"highContrast":false,"onboarding":"completed"});
        let preferences: Preferences = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(preferences.theme, Theme::Light);
        assert_eq!(
            preferences.appearance,
            crate::configuration::appearance::AppearancePreferences::default()
        );
        for theme in ["light", "dark", "system"] {
            let mut value = original.clone();
            value["theme"] = theme.into();
            let decoded: Preferences = serde_json::from_value(value.clone()).unwrap();
            value["myLanguages"] = serde_json::json!([]);
            value["onboardingRequired"] = false.into();
            value["onboardingLanguage"] = serde_json::Value::Null;
            value["onboardingHelp"] = false.into();
            value["appearance"] = serde_json::to_value(
                crate::configuration::appearance::AppearancePreferences::default(),
            )
            .unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), value);
        }
        let mut invalid = original;
        invalid["theme"] = "unknown".into();
        assert!(serde_json::from_value::<Preferences>(invalid).is_err());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialCheck {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub diagnostics: Option<serde_json::Value>,
    pub provider: String,
    pub state: String,
    pub status: Option<u16>,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccessCheck {
    pub providers: Vec<ProviderCredentialCheck>,
}
