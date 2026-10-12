export interface Shortcuts {
  mic: string
  speak: string
  panel: string
  settings: string
}

export interface Settings {
  execution?: import('./generated/contracts').ExecutionPreferences
  appearance?: import('./generated/contracts').AppearancePreferences
  theme?: 'light' | 'dark' | 'system'
  scope?: { sessionId: string; conversationId: string; settingsRevision: number; learnerRevision: number; rewardRevision: number }
  provider_mode: import('./generated/contracts').ConnectionRoute
  hosted_email: string
  custom_base_url: string
  custom_model: string
  standard_model: string
  observer_model: string | null
  my_languages: string[]
  target_varieties: Record<string, string>
  script_scales?: Record<string, number>
  target_language: string
  target_variety: string
  native_variety: string
  interface_locale: string
  native_language: string
  microphone_device_id: string | null
  auto_speak: boolean
  auto_send: boolean
  always_romanize: boolean
  auto_translate: boolean
  text_size: number
  text_spacing: number
  always_pronunciation: boolean
  xp_effects?: boolean
  fast_mode: boolean
  reward_sounds: import('./platform/audio/reward-sounds').RewardSoundMode
  master_volume: number
  voice_volume: number
  effects_volume: number
  tts_rate: number
  shortcuts: Shortcuts
}

/// One row in the chat history sidebar. Mirrors `ChatSummary` in
/// `conversation.rs`.
export interface ChatSummary {
  id: string
  /// Derived from the first thing said. Empty for a chat with no turns yet.
  title: string
  updated_at: number
  turn_count?: number
}

/// A conversation as opened: which one it is, and its turns.
export interface OpenedConversation {
  id: string
  turns: StoredTurn[]
}

export type AnalysisState = 'pending' | 'done' | null

export type PersonaReaction = import('./generated/contracts').PartnerReaction

/// One exchange, as stored. This is the canonical turn shape: the live turn in
/// ConversationPage is this plus `pendingText`, the streaming buffer, which is
/// transient by definition and never written to disk.
export interface StoredTurn {
  userMessageId?: string
  assessmentState?: string | null
  assessmentError?: string | null
  feedbackState?: string | null
  replyState?: import('./domain/conversation/reply-state').ReplyState
  /// The turn's recorded operations and attempts, for live activity.
  execution?: import('./generated/contracts').TurnView
  turnId?: string
  replacesTurnId?: string | null
  replacedBy?: string | null
  /// Revisions sent in place of this message's earlier versions.
  fixes?: number
  userSavedGloss?: import('./generated/contracts').WordGlossView | null
  userGlossOperationId?: string | null
  userTranslation?: string | null
  userTranslationState?: string | null
  userGlossError?: string | null
  userGlossState?: string | null

  id: number
  user: string | null
  assistant: GuidedTurnResult | null
  /// Saved feedback determines completion; operation errors remain independent
  /// in coachError and replyState, including when no reply exists.
  analysisState: AnalysisState
  feedbackContext?: string
  conversationFeedback?: import('./generated/contracts').ConversationFeedback
  coach?: import('./generated/contracts').CoachObservationView
  coachDecision?: import('./generated/contracts').CoachDecision
  reaction?: PersonaReaction
  reactionError?: string
  coachError?: string
}

/// Identity and remaining allowance on the hosted service. Mirrors
/// `GET /v1/me` and the `Account` struct in `hosted.rs`.
/// Mirrors `GET /v1/me`. The allowance is money — the service meters what the
/// provider actually charged, because a token's price varies about a
/// hundredfold across models. Tokens and turns are ESTIMATES derived from this
/// account's own usage, for readability, and gate nothing.
export interface HostedAccount {
  email: string
  name: string
  used_usd: number
  limit_usd: number
  remaining_usd: number
  tokens_today: number
  requests_today: number
  estimated_requests_remaining: number
  estimated_tokens_remaining: number
  /// This account has its own daily limit instead of the service default.
  custom_limit: boolean
  resets: string
}

export interface GuidedToken {
  pronunciation: string | null
  text: string
  gloss: string | null
  pos: string | null
  notable: boolean
  romanization: string | null
}

export interface Mechanic {
  quote?: string
  title: string
  cefr: string | null
  body: string
  example: string | null
  contrast: string | null
}

export interface Scaffolds {
  replies: import('./generated/contracts').SuggestedReply[]
  frames: string[]
  starters: string[]
}

export interface GuidedTurnResult {
  help?: import('./domain/conversation/reply-help').ReplyHelpView
  assistance?: import('./generated/contracts').ReplyAssistance
  explanationsState?: string | null
  explanationsError?: string | null
  /// State of the reply-suggestions job for this persona message.
  suggestionsState?: string | null
  translationState?: string | null
  messageId?: string
  savedGloss?: import('./generated/contracts').WordGlossView | null
  glossError?: string | null
  glossState?: string | null
  glossOperationId?: string | null
  reply: string
  translation: string | null
  tokens: GuidedToken[]
  user_tokens: GuidedToken[]
  user_translation: string | null
  mechanics: Mechanic[]
  scaffolds: Scaffolds
  errors: string[]
}


export interface RecurringError {
  error: string
  correction: string
  seen_count: number
}

export interface TaughtMechanic {
  mechanic: string
  last_seen_turn: number
}

export interface TeachingPlan {
  session_focus: string[]
  recurring_errors: RecurringError[]
  vocab_recycle: string[]
  avoid: string[]
  learner_interests: string[]
  energy_read: string
  correction_budget: number
  taught_ledger: TaughtMechanic[]
}

export interface Profile {
  about: string
  level_notes: string
  strengths: string[]
  weaknesses: string[]
  interests: string[]
  long_term_errors: RecurringError[]
}

export interface ObserverDocuments {
  plan: TeachingPlan
  profile: Profile
}

export type Level = 'beginner' | 'intermediate' | 'advanced' | 'fluent'


// ─── Observability: one Run per agent execution ──────────────────────────
// Mirrors native/src/trace.rs. See docs/docs-site/docs/observability.md.

/// Who a unit of work belongs to. Mirrors ontology.rs::Actor.
/// Only two agents exist — `chat` and `coach`. Everything else is the Runner.
export type Actor =
  | { type: 'agent'; id: 'chat' | 'coach' }
  | { type: 'runner' }

export type AttemptKind =
  | 'ok'
  | 'rate_limited'
  | 'unparseable'
  | 'invalid'
  | 'failed'

export type RunOutcome = 'ok' | 'retried_then_ok' | 'failed'

export interface Usage {
  prompt_tokens: number | null
  completion_tokens: number | null
  total_tokens: number | null
  cost: number | null
}

export interface RequestContext {
  chat_id: string; message_id: number | null; replaces_message_id: number | null; trigger: string
  target: string; native: string; variety: string; provider_mode: string; difficulty: 'zero' | 'beginner' | 'intermediate' | 'advanced' | 'fluent'
  inferred_level_notes: string; topic: string | null; persona: unknown; history_messages: number; history_available: number
}
export interface PromptBlock { id: string; source: string; content: string }
export interface RecordedRequest {
  messages: { role: string; content: string; truncated: boolean }[]
  parameters: Record<string, unknown>; route: string; blocks: PromptBlock[]; truncated: boolean
}
export interface LengthCheck { status: string; sentences: number; words: number | null; violations: string[]; method: string }
export interface Attempt {
  request: RecordedRequest | null
  response: string | null
  response_truncated: boolean
  index: number
  kind: AttemptKind
  duration_ms: number
  /** The provider's or parser's own message, verbatim. */
  error: string | null
  usage: Usage | null
}

export interface Run {
  session_id: string
  app_version: string
  context: RequestContext | null
  length_checks: LengthCheck[]
  application_status: string
  id: number
  /** Groups every run fired by one conversational turn. */
  turn_id: number | null
  /// Which ontology Operation ran.
  operation: string
  actor: Actor
  label: string
  model: string
  temperature: number | null
  reasoning: boolean
  max_tokens: number | null
  streamed: boolean
  schema: string | null
  started_at_ms: number
  first_token_ms: number | null
  duration_ms: number
  usage: Usage | null
  attempts: Attempt[]
  outcome: RunOutcome
  error: string | null
  /** The messages actually sent, as `role: content` blocks. The *content*
   *  of the node — payload only, never headers. */
  prompt: string | null
  /** The model's raw response, before parsing. */
  output: string | null
}

// ─── The declared graph (native/src/graph.rs) ─────────────────────────
// The frontend draws ONLY what get_graph returns. No second copy.

export type NodeKind = 'input' | 'agent_step' | 'tool' | 'faculty' | 'barrier'

export type EdgeKind =
  | 'sequential'
  | 'fan_out'
  /** Node → your screen, the instant it finishes. Nothing waits for siblings. */
  | 'hydrate'
  /** Reconciliation only — never a gate. */
  | 'fan_in'
  | 'conditional'
  | 'background'

export interface GraphNode {
  id: string
  label: string
  kind: NodeKind
  operation: string | null
  purpose: string
  x: number
  y: number
}

export interface GraphEdge {
  from: string
  to: string
  kind: EdgeKind
  condition: string | null
}

export interface Graph {
  id: string
  label: string
  description: string
  nodes: GraphNode[]
  edges: GraphEdge[]
  shared_state: string[]
}

// ─── Reconciliation: the graph's own fidelity (native/src/trace.rs) ───
// The observability layer reporting on itself. A declaration that cannot
// tell you when it is wrong is a claim, not an observation.

export interface EdgeVerdict {
  from: string
  to: string
  verdict: 'observed' | 'contradicted' | 'unobserved'
  detail: string | null
}

export interface Reconciliation {
  turns_observed: number
  /** Ran, but no node declares it — the map is missing something. */
  undeclared_operations: string[]
  /** Declared but never seen to run this session. Unproven, not wrong. */
  unobserved_operations: string[]
  edges: EdgeVerdict[]
  consistent: boolean
}

/** Announced when an operation begins — see trace.rs. A completed `Run`
 *  arrives far too late to show that something is working right now. */
export interface RunStarted {
  context: RequestContext | null
  id: number
  turn_id: number | null
  operation: string
  actor: Actor
  label: string
  model: string
  started_at_ms: number
}
