import { reducedMotion } from '../../../platform/appearance/motion'
import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { ActivityIndicator } from '../../../components/feedback/ActivityIndicator'
import { PersonaAvatar } from '../../../components/media/PersonaAvatar'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { useI18n } from '../../../components/localization/i18n'
import type { ConversationStartConfig, SavedTopic, Snapshot, TopicCard, TopicChoice } from '../../../generated/contracts'
import { useContext, useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { executeAction, nativeError, readWorkspace } from '../../../platform/ipc/workspace'
import { SkillEvidenceContext } from '../../../state/learning/useSkillEvidence'
import { ConversationPromptCreator } from './ConversationPromptCreator'
import { validTopicText } from './CustomTopicDialog'
import { StartOptions, startOptionsSummary } from './StartChoices'

/// The empty conversation. Every start is one press: the partner starts, a topic
/// starts, or the learner's own topic starts; or the learner sends a first
/// message from the composer below. The options come first, folded to one line,
/// and apply to every one of them.
///
/// It renders inside the ordinary message stream, above the ordinary composer.
/// This component owns display and callbacks only; recording, the draft and
/// submission stay with the page.
export function ConversationStart({ topics, busy, onStart, partnerName, partnerSymbol, conversationId, value, onChange, targetTag, targetDir, recording, transcribing, canPartnerStart, onAboutPartner, onChangePartner }: {
  partnerName?: string; partnerSymbol?: string; conversationId: string
  topics: TopicCard[]; busy: boolean; value: ConversationStartConfig
  targetTag?: string; targetDir?: string
  recording: boolean; transcribing: boolean
  /// False while the composer holds a draft or the microphone is in use: a
  /// partner-first opening would either talk over an unsent first message or
  /// land in the middle of one recording.
  canPartnerStart: boolean
  /// The partner's profile, and the picker that changes partner.
  onAboutPartner?: () => void
  onChangePartner?: () => void
  onChange: (value: ConversationStartConfig) => void; onStart: (configuration: ConversationStartConfig) => Promise<void>
}) {
  const tr = useI18n()
  const ids = useId()
  const { snapshot } = useContext(SkillEvidenceContext)
  // The start in flight: 'partner', a topic's id, 'saved:' and a saved topic's id, or 'own'.
  const [starting, setStarting] = useState<string | null>(null)
  const pending = useRef(false)
  const [error, setError] = useState<string | null>(null)
  const [workspace, setWorkspace] = useState<Snapshot | null>(null)
  const [saved, setSaved] = useState<SavedTopic[]>([])
  const [own, setOwn] = useState('')
  const [saveOwn, setSaveOwn] = useState(false)
  // Saved here already, so a retried start does not save it twice.
  const [savedOwn, setSavedOwn] = useState<string | null>(null)
  const [optionsOpen, setOptionsOpen] = useState(false)
  const options = useRef<HTMLDivElement>(null)
  // Opened after the stream has scrolled, the options may reach past its edge:
  // once open, the stream scrolls just far enough to show them.
  const stillMotion = () => reducedMotion()
  const revealOptions = () => options.current?.scrollIntoView({ block: 'nearest', behavior: stillMotion() ? 'auto' : 'smooth' })
  useEffect(() => {
    let current = true
    // Topics saved for later belong to the learner across languages; each starts like any other.
    readWorkspace().then(result => { if (current) setSaved(result.savedTopics) }, reason => { if (current) setError(nativeError(reason)) })
    return () => { current = false }
  }, [])
  async function start(key: string, topic?: TopicChoice, before?: () => Promise<void>) {
    if (busy || pending.current || !canPartnerStart || recording || transcribing) return
    pending.current = true; setStarting(key); setError(null)
    try {
      await before?.()
      // A topic started here goes with this start only; the draft keeps the options.
      // BACKEND: the topic and a skill focus share the direction's `topic`, so a
      // topic start drops the focus. It will keep it once the direction has a
      // practice target of its own (see StartOptions' Skill row).
      await onStart(topic ? { ...value, direction: { ...value.direction, topic } } : value)
    } catch (reason) { setError(nativeError(reason)) }
    finally { pending.current = false; setStarting(null) }
  }
  async function saveTopics(additions: string[], deletions: string[]) {
    if (!additions.length && !deletions.length) return
    const current = workspace ?? await readWorkspace()
    await executeAction(current, { kind: 'saveTopics', additions, deletions, expectedRevision: current.revision })
  }
  const [deleting, setDeleting] = useState<string | null>(null)
  async function deleteSaved(item: SavedTopic) {
    setDeleting(item.id); setError(null)
    try {
      await saveTopics([], [item.id])
      setSaved((await readWorkspace()).savedTopics)
    } catch (reason) { setError(nativeError(reason)) }
    finally { setDeleting(null) }
  }
  function startOwn() {
    const text = own.trim()
    if (!validTopicText(text)) { setError(tr('Enter a topic of 1–500 characters.')); return }
    const save = saveOwn && savedOwn !== text && !saved.some(topic => topic.text === text)
    void start('own', { kind: 'custom', text }, save ? async () => { await saveTopics([text], []); setSavedOwn(text) } : undefined)
  }
  // One guard for every control that begins or captures: the configuration must
  // not move under a recording that has already started.
  const disabled = busy || starting !== null || recording || transcribing
  const startDisabled = disabled || !canPartnerStart
  const name = partnerName ?? tr('partner')
  // An unset skill focus is offered once there is recorded practice; a configured
  // focus remains visible, including the default Coach’s choice in a new chat.
  const recordedPractice = Boolean(snapshot?.profile.skills.some(skill => skill.experience > 0))
  // A topic set in Prompt details goes with the partner's start and the first message, so the summary names it.
  const topic = value.direction.topic
  const configuredTopic = topic?.kind === 'builtin' ? topics.find(item => item.id === topic.id)?.target : topic?.kind === 'custom' ? topic.text : undefined
  const owner = workspace?.conversations.find(item => item.id === conversationId)
  const contact = workspace?.contacts.find(item => item.id === owner?.contactId)
  const persona = workspace?.personas.find(item => item.id === contact?.personaId)
  const language = workspace?.languages.find(item => item.id === owner?.languageId)
  const mark = (key: string) => starting === key ? <ActivityIndicator label={tr('Starting…')} compact /> : <ToolbarIcon name="chevron" size={16} />
  return <section className="conversation-start" aria-label={tr('Start a conversation')} aria-busy={starting !== null}>
    {/* First, a panel of their own: the options shape every start below them. */}
    <div className="start-options" data-open={optionsOpen} ref={options}>
      <button type="button" className="start-options-toggle" aria-expanded={optionsOpen} aria-controls={`${ids}-options`}
        onClick={() => { setOptionsOpen(!optionsOpen); if (!optionsOpen && stillMotion()) requestAnimationFrame(revealOptions) }}>
        <ToolbarIcon name="settings" size={16} /><span className="start-options-title">{tr('Options')}</span>
        <span className="start-options-summary">{[configuredTopic, startOptionsSummary(tr, value)].filter(Boolean).join(' · ')}</span>
        <ToolbarIcon name="chevron" size={16} />
      </button>
      <div className="start-options-body" id={`${ids}-options`} inert={!optionsOpen}
        onTransitionEnd={event => { if (optionsOpen && event.target === event.currentTarget && event.propertyName === 'grid-template-rows') revealOptions() }}>
        <div className="start-options-content">
          <StartOptions conversationId={conversationId} value={value} disabled={disabled} skillFocus={recordedPractice} onChange={onChange} />
        </div>
      </div>
    </div>
    {/* The partner's side of the table, in the partner's colours, with the main start beside them. */}
    <div className="start-partner">
      <PersonaAvatar symbol={partnerSymbol} />
      <div className="start-partner-text">
        {partnerName && <h2><bdi lang={targetTag} dir={targetDir}>{partnerName}</bdi></h2>}
        {(onAboutPartner || onChangePartner) && <div className="start-partner-links">
          {onAboutPartner && <button type="button" className="start-link" disabled={disabled} onClick={onAboutPartner}>{tr('About {name}', { name })}</button>}
          {onChangePartner && <button type="button" className="start-link" disabled={disabled} onClick={onChangePartner}>{tr('Change partner')}</button>}
        </div>}
      </div>
      <button type="button" className="btn primary start-conversation-button" disabled={startDisabled} onClick={() => void start('partner')}>
        {starting === 'partner' ? <ActivityIndicator label={tr('Starting…')} /> : <><ToolbarIcon name="chat" />{tr('{name} starts', { name })}</>}</button>
    </div>
    <fieldset className="start-topics" disabled={startDisabled}>
      <legend>{tr('Or pick a topic')}</legend>
      <div className="topic-starts">
        {topics.map(item => <TopicStart key={item.id} mark={mark(item.id)} onStart={() => void start(item.id, { kind: 'builtin', id: item.id })}>
          <span className="topic-start-target target-word"><bdi lang={targetTag} dir={targetDir}>{item.target}</bdi></span>
          {/* Romanization is Latin whatever the surrounding script, so it states
              its direction rather than inferring one: a leading modifier letter
              such as ʿ is not a strong character and would inherit RTL. */}
          {item.romanized && <span className="topic-start-roman"><bdi dir="ltr">{item.romanized}</bdi></span>}
          {item.translation !== item.target && <span className="topic-start-translation">{item.translation}</span>}
        </TopicStart>)}
        {/* Saved topics are the learner's own words, in whichever language they wrote them. */}
        {/* Each carries a delete control in its corner, shown on hover or focus. */}
        {saved.map(item => <span key={item.id} className="saved-topic">
          <TopicStart mark={mark(`saved:${item.id}`)} onStart={() => void start(`saved:${item.id}`, { kind: 'custom', text: item.text })}>
            <span className="topic-start-saved"><bdi>{item.text}</bdi></span>
          </TopicStart>
          <button type="button" className="saved-topic-delete" aria-label={tr('Delete {value0}', { value0: item.text })} title={tr('Delete {value0}', { value0: item.text })}
            aria-busy={deleting === item.id} disabled={deleting !== null} onClick={() => void deleteSaved(item)}>
            {deleting === item.id ? <ActivityIndicator label={tr('Delete {value0}', { value0: item.text })} compact /> : <ToolbarIcon name="trash" size={14} />}
          </button>
        </span>)}
      </div>
    </fieldset>
    <form className="start-own" onSubmit={event => { event.preventDefault(); startOwn() }}>
      <div className="start-own-row">
        <input className="field" value={own} disabled={disabled} aria-label={tr('Your own topic')} placeholder={tr('Enter your own topic…')}
          onChange={event => setOwn(event.target.value)} />
        <button type="submit" className="btn" disabled={startDisabled || !own.trim()}>{starting === 'own' ? <ActivityIndicator label={tr('Starting…')} compact /> : tr('Start')}</button>
      </div>
      <label className="start-own-save"><input type="checkbox" checked={saveOwn} disabled={disabled} onChange={event => setSaveOwn(event.target.checked)} />{tr('Save for later')}</label>
    </form>
    <p className="start-or-message">{tr('…or send a message to begin')}</p>
    <div className="start-prompt-editor">
      <button type="button" className="start-link" disabled={disabled}
        onClick={async () => { setError(null); try { setWorkspace(await readWorkspace()) } catch (reason) { setError(nativeError(reason)) } }}>
        <ToolbarIcon name="bot" size={16} />{tr('Open prompt editor…')}
      </button>
    </div>
    {error && <ErrorNotice as="p" error={error} className="start-error">{error}</ErrorNotice>}
    {workspace && persona && language && <ConversationPromptCreator conversationId={conversationId} initial={value} topics={topics} savedTopics={workspace.savedTopics} language={language} persona={persona.details} onClose={() => setWorkspace(null)} onApply={async (configuration, additions, deletions) => {
      await saveTopics(additions, deletions)
      onChange({ ...configuration, promptEditor: true })
      // The creator may have saved or deleted topics; the applied change stands either way.
      if (additions.length || deletions.length) readWorkspace().then(result => setSaved(result.savedTopics), reason => setError(nativeError(reason)))
    }} />}
  </section>
}

/** A topic that starts the conversation when pressed, marked as an action by the arrow at its end. */
function TopicStart({ mark, onStart, children }: { mark: ReactNode; onStart: () => void; children: ReactNode }) {
  return <button type="button" className="topic-start" onClick={onStart}><span className="topic-start-text">{children}</span>{mark}</button>
}
