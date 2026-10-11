import { useMessageToolDefinitions } from './useMessageToolDefinitions'
import { sentenceBlanks } from '../../domain/reading/sentence-blanks'
import { readingWords } from '../../domain/reading/word-boundaries'
import { bubbleSelection } from './bubble-selection'
import { AddToDrillButton } from './AddToDrillButton'
import { ProvenanceTip, type MessageProvenance } from './MessageProvenance'
import { SelectionRing } from './SelectionRing'
import { MessageTools, type MessageInspect, type MessageTool } from './MessageTools'
import { useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import type { GlossSegment } from '../../generated/contracts'
import { errorDetails, errorMessage } from '../../platform/diagnostics/error-details'
import { ErrorDetails } from '../feedback/ErrorDetails'
import { ResponseDetails } from '../feedback/ResponseDetails'
import { useI18n } from '../localization/i18n'
import { ReadingTemplateContext, useReadingActions, useReadingLookup, useReadingPeek, useReadingScope } from './ReadingContext'
import { useReadAloud } from './useReadAloud'
import { useReadingAidSpace, useReadingPreferences } from './ReadingPreferences'
import { SavedGlossText } from './SavedGlossText'
import { TargetText } from './TargetText'
import { useSavedReading } from './SavedReadingProvider'
import { TranslationStatus, translationPending } from './TranslationStatus'

/// Where the message sits. `bubble` is a message in a thread with its actions
/// inside the bubble; `passage` is that same bubble with standalone spacing;
/// `compact` uses the same bubble without outer passage spacing.
export type TargetMessageLayout = 'bubble' | 'passage' | 'compact'

export interface TargetMessageSpeech {
  preparing?: boolean
  disabled?: boolean
  speaking: boolean
  onToggle: () => void
  error: { text: string; details: unknown } | null
}

export interface TargetMessageAnalysis {
  pending: boolean
  onOpen: () => void
}

export interface TargetMessageProps {
  graphTargets?: { text?: Record<string, string>; translation?: Record<string, string>; gloss?: Record<string, string>; audio?: Record<string, string> }
  additionalActions?: ReactNode
  menuActions?: ReactNode
  /** Durable message owners supply requests instead of generic text lookups. */
  onRequestWords?: () => Promise<void>
  onRequestTranslation?: () => Promise<void>
  /** Explicit list-wide actions; null leaves this aid under individual control. */
  bulkReading?: { translations: boolean | null; phonetics: boolean | null }
  /** Optional source presentation; the complete text still owns tools and speech. */
  sourcePresentation?: ReactNode
  side?: 'me' | 'bot'
  text: string
  /// Word meanings already known for `text`, anchored by UTF-16 offsets.
  segments: GlossSegment[]
  /// Changes when a different set of saved meanings replaces the current one.
  segmentsKey: string
  translation: string | null
  /// Whole-message reading aids, shown when no word-level aid covers them.
  romanization: string | null
  pronunciation: string | null
  layout: TargetMessageLayout
  /// Accessible name of the translate toggle; `null` uses its visible label.
  translateLabel: string | null
  /// Saved meanings for this message are still being produced.
  segmentsPending: boolean
  /// Words and Translate may request aids through the reading lookup.
  lookupWords: boolean
  /// Owner-supplied progress and failures for this message's aids.
  status: ReactNode
  /// Operation state of a translation the owner requested for this text. The
  /// Translate control carries its progress; a reserved line shows it only
  /// while the translation is set to show and has no text yet.
  translationState?: string | null
  /** Already saved Drill phrases use their owning reference controls. */
  addToDrill?: boolean
  /// An owner's own control in the Add to Practice slot, for text whose keeping
  /// runs through the owner (a generated card keeps its provenance). It takes
  /// the place of the shared Add to Practice button.
  practiceAction?: ReactNode
  /// Where this text came from. Only owners that know it pass one, and only
  /// then does the message show its "How this was added" tip.
  provenance: MessageProvenance | null
  readAloud?: boolean
  /// Owner-supplied content shown directly under the text.
  annotation: ReactNode
  speech: TargetMessageSpeech | null
  /// Null uses shared reading analysis when lookup is enabled; conversation
  /// owners supply their own saved-message analysis handler.
  analysis: TargetMessageAnalysis | null
  /// The audio inspector's toggle, when the owner can inspect this message's audio.
  inspect?: MessageInspect | null
  /// The open audio inspector, drawn inside the bubble above the tools.
  inspector?: ReactNode
  onSelect?: () => void
  focused: boolean
  rtl: boolean
}

/** One target-language message with its reading tools: word meanings,
 *  translation, whole-message reading aid, read-aloud and analysis. Every
 *  consumer supplies its own data and actions; the tools behave identically. */
export function TargetMessage({
  text, segments, segmentsKey, translation, romanization, pronunciation, layout, translateLabel,
  segmentsPending, lookupWords, status, translationState, annotation, speech, analysis, focused, rtl, addToDrill = true, practiceAction, provenance, readAloud = true, side = 'bot',
  inspect = null, inspector, onSelect, sourcePresentation, bulkReading, onRequestWords, onRequestTranslation, additionalActions, menuActions, graphTargets,
}: TargetMessageProps) {
  const tr = useI18n()
  const messageTools = useMessageToolDefinitions()
  const preferences = useReadingPreferences()
  const scope = useReadingScope(), saved = useSavedReading(), peek = useReadingPeek(), lookup = useReadingLookup()
  const shared = useReadAloud(text)
  const template = useContext(ReadingTemplateContext)
  const readingActions = useReadingActions()
  const messageAnalysis = analysis ?? (lookupWords && scope && readingActions ? {
    pending: false,
    onOpen: () => readingActions.inspect({ text, start: 0, end: text.length, scope, aid: template && sentenceBlanks(text, true).length ? 'completions' : 'explanations' }),
  } : null)
  // Owners supply playback behavior, never an alternative playback button.
  const playback = speech ?? (readAloud && shared ? { speaking: shared.playing, onToggle: shared.onToggle, error: null } : null)
  // Saved, cached and requested meanings come from the reading services only
  // when the owner allows lookup and a reading scope is present.
  const readingScope = lookupWords ? scope : null
  const canLookup = readingScope !== null && lookup !== null

  // Every lookup belongs to one source: this text in this reading scope.
  const sourceKey = readingScope ? JSON.stringify([readingScope, text]) : null
  const local = segments.length ? segments : readingScope ? saved(text, readingScope).filter(part => part.kind === 'gloss') : []
  const cachedResult = readingScope ? peek({ ...readingScope, text, aid:'word_gloss' }) : null
  const cached = cachedResult?.gloss?.segments ?? []
  const [fetchedResult, setFetchedResult] = useState<{ source: string; segments: GlossSegment[] } | null>(null)
  const fetched = fetchedResult !== null && fetchedResult.source === sourceKey ? fetchedResult.segments : []
  // A whole-passage result exists once the cache holds complete coverage or
  // this source's own lookup answered; saved or cached meanings may be partial.
  const lookedUp = cachedResult?.gloss?.coverage === 'complete' || (fetchedResult !== null && fetchedResult.source === sourceKey)
  const resolved = [...local, ...[...cached, ...fetched].filter(part => !local.some(item => item.start < part.end && item.end > part.start))]
  // Prefer one complete cached/fetched result over duplicating equivalent anchors.
  const known = resolved.filter((part, index) => !resolved.slice(0, index).some(item => item.start < part.end && item.end > part.start)).map(part => {
    const fresh = fetched.find(item => item.start === part.start && item.end === part.end)
    return fresh ? { ...part, romanization: part.romanization ?? fresh.romanization, pronunciation: part.pronunciation ?? fresh.pronunciation } : part
  })

  const aidsEnabled = preferences.autoTranslate || preferences.alwaysRomanize || preferences.alwaysPronunciation
  const [wordsOverride, setWordsOverride] = useState<boolean | null>(null)
  const aidSpace = useReadingAidSpace(wordsOverride === true)
  const wordsOpen = wordsOverride ?? (aidsEnabled && known.length > 0)
  const [translationOverride, setTranslationOverride] = useState<boolean | null>(null)
  const translationOpen = translationOverride ?? preferences.autoTranslate
  const sound = preferences.supportsRomanization && romanization ? romanization : pronunciation
  const [soundOverride, setSoundOverride] = useState<boolean | null>(null)
  const soundOpen = soundOverride ?? (sound
    ? (preferences.supportsRomanization && romanization ? preferences.alwaysRomanize : preferences.alwaysPronunciation)
    : wordsOpen && (wordsOverride === true || preferences.alwaysRomanize || preferences.alwaysPronunciation))
  useEffect(() => { setWordsOverride(null) }, [preferences.autoTranslate, preferences.alwaysRomanize, preferences.alwaysPronunciation])

  const words = useSourceRequest(sourceKey)
  async function toggleWords() {
    if (onRequestWords) {
      const request = known.length === 0 && !segmentsPending && !words.pending
      setWordsOverride(request ? true : !wordsOpen)
      if (request) await words.run(() => onRequestWords(), () => {})
      return
    }
    // Missing meanings: one explicit click requests the whole passage and shows
    // it, even when a preference already shows the partial meanings.
    const request = !lookedUp && !words.pending && readingScope !== null && lookup !== null && sourceKey !== null
    setWordsOverride(request ? true : !wordsOpen)
    if (!request) return
    const source = sourceKey
    await words.run(signal => lookup({ ...readingScope, text, aid: 'word_gloss' }, signal),
      result => setFetchedResult({ source, segments: result.gloss?.segments ?? [] }))
  }

  // A supplied translation wins; otherwise the reading lookup may translate
  // this source on request, through the same translation contract as Chat.
  const cachedTranslation = readingScope ? peek({ ...readingScope, text, aid: 'translation' })?.translation ?? null : null
  const [fetchedTranslation, setFetchedTranslation] = useState<{ source: string; text: string } | null>(null)
  const shownTranslation = translation ?? cachedTranslation ?? (fetchedTranslation !== null && fetchedTranslation.source === sourceKey ? fetchedTranslation.text : null)
  const translating = useSourceRequest(sourceKey)
  // What the learner sees: a translation is shown only when one exists.
  const translationShown = translationOpen && shownTranslation !== null
  const translationWorking = translationPending(translationState)
  async function showTranslation(visible: boolean) {
    setTranslationOverride(visible)
    if (!visible) { translating.cancel(); return }
    if (shownTranslation !== null) return
    // Nothing to show yet: one explicit click requests it and shows it.
    setTranslationOverride(true)
    if (onRequestTranslation) {
      await translating.run(() => onRequestTranslation(), () => {})
      return
    }
    if (readingScope === null || lookup === null || sourceKey === null) return
    const source = sourceKey
    await translating.run(signal => lookup({ ...readingScope, text, aid: 'translation' }, signal), result => {
      if (result.translation === null) throw new Error('The reading service returned no translation.')
      setFetchedTranslation({ source, text: result.translation })
    })
  }
  const toggleTranslation = () => showTranslation(shownTranslation === null || !translationOpen)

  const phonetics = useSourceRequest(sourceKey)
  const hasPhonetics = known.some(part => preferences.supportsRomanization ? part.romanization || part.pronunciation : part.pronunciation)
  const phoneticParts = known.filter(part => part.kind === 'gloss' && part.gloss !== null
    && (preferences.supportsRomanization ? part.romanization || part.pronunciation : part.pronunciation))
  const phoneticsComplete = phoneticParts.length > 0 && readingWords(text).filter(word => word.word).every(word => {
    let end = word.start
    for (const part of phoneticParts) if (part.start <= end && part.end > end) end = part.end
    return end >= word.end
  })
  async function showPhonetics(visible: boolean) {
    setSoundOverride(visible)
    if (!visible) { phonetics.cancel(); return }
    if (sound || phoneticsComplete || !readingScope || !lookup || !sourceKey) return
    const source = sourceKey
    await phonetics.run(signal => lookup({ ...readingScope, text, aid: 'word_gloss' }, signal), result => {
      const parts = result.gloss?.segments ?? []
      if (!parts.some(part => part.kind === 'gloss' && part.gloss !== null && (preferences.supportsRomanization ? part.romanization || part.pronunciation : part.pronunciation))) {
        throw new Error('The reading service returned no pronunciation or romanization.')
      }
      setFetchedResult({ source, segments: parts })
    })
  }
  // Only changes to an explicit bulk choice (or its source) dispatch work.
  // Ordinary renders and individual toggles never repeat the bulk command.
  const bulkActions = useRef({ showTranslation, showPhonetics })
  bulkActions.current = { showTranslation, showPhonetics }
  useEffect(() => {
    if (bulkReading?.translations != null) void bulkActions.current.showTranslation(bulkReading.translations)
  }, [bulkReading?.translations, sourceKey])
  useEffect(() => {
    if (bulkReading?.phonetics != null) void bulkActions.current.showPhonetics(bulkReading.phonetics)
  }, [bulkReading?.phonetics, sourceKey])

  const body = <>
    <span data-phrase-source {...graphTargets?.text}>{sourcePresentation ?? (known.length > 0
      ? <SavedGlossText graphTarget={graphTargets?.gloss} key={segmentsKey} text={text} segments={known} showAids={wordsOpen} revealAids={wordsOverride === true} showSound={soundOverride ?? undefined} />
      : <TargetText text={text} />)}</span>
    {annotation}
    {translationShown && <div className="trans" dir="auto" {...graphTargets?.translation}>{shownTranslation}</div>}
    {translationState !== undefined && <TranslationStatus state={translationState} shown={translationOpen && shownTranslation === null} />}
    {soundOpen && sound && !(wordsOpen && known.some(part => part.romanization || part.pronunciation)) && <div className="wroman" dir="auto">{sound}</div>}
    {status}
    {playback?.error && <ErrorDetails onRetry={() => playback.onToggle()} label={tr("Speech")} errorKey={playback.error.text} explanation={playback.error.text}><ResponseDetails value={playback.error.details} /></ErrorDetails>}
  </>
  const tools: MessageTool[] = [
    ...(shownTranslation || canLookup || onRequestTranslation || translationWorking ? [messageTools.translate({ ariaLabel: translateLabel ?? undefined,
      pressed: translationShown, pending: translating.pending || translationWorking, disabled: translating.pending, onSelect: () => void toggleTranslation() })] : []),
  ]
  const more: MessageTool[] = [
    messageTools.words({ pressed: wordsOpen, pending: segmentsPending || words.pending,
      disabled: words.pending || (known.length === 0 && !canLookup && !onRequestWords), onSelect: () => void toggleWords() }),
    ...(sound || hasPhonetics || bulkReading ? [messageTools.pronunciation({ pressed: soundOpen,
      pending: phonetics.pending, disabled: !sound && !hasPhonetics && !canLookup,
      onSelect: () => void showPhonetics(!soundOpen || phonetics.error != null) })] : []),
    ...(messageAnalysis ? [messageTools.analysis({ pending: messageAnalysis.pending, onSelect: messageAnalysis.onOpen })] : []),
  ]
  const actions = <MessageTools menuActions={menuActions} tools={tools} inspect={inspect} more={more}
    play={playback && { graphTarget: graphTargets?.audio, playing: playback.speaking, preparing: 'preparing' in playback && playback.preparing, disabled: playback.disabled, onToggle: playback.onToggle }}
    actions={<>{provenance && <ProvenanceTip provenance={provenance} />}{practiceAction ?? (addToDrill && <AddToDrillButton text={text} />)}{additionalActions}</>} />
  const failure = <>
    {words.error != null && <ErrorDetails onRetry={toggleWords} label={tr('Word meanings')} errorKey={errorMessage(words.error)} explanation={errorMessage(words.error)}><ResponseDetails value={errorDetails(words.error)} /></ErrorDetails>}
    {translating.error != null && <ErrorDetails onRetry={toggleTranslation} label={tr('Translation')} errorKey={errorMessage(translating.error)} explanation={errorMessage(translating.error)}><ResponseDetails value={errorDetails(translating.error)} /></ErrorDetails>}
    {phonetics.error != null && <ErrorDetails onRetry={() => showPhonetics(true)} label={tr('Pronunciation')} errorKey={errorMessage(phonetics.error)} explanation={errorMessage(phonetics.error)}><ResponseDetails value={errorDetails(phonetics.error)} /></ErrorDetails>}
  </>

  // Meanings that are set to show and still being produced keep their line pitch.
  const aidsReserved = (wordsOverride ?? aidsEnabled) && ((known.length === 0 && segmentsPending) || sourcePresentation !== undefined)
  const bubble = <div {...bubbleSelection(onSelect, focused, side === 'me' ? tr("Your message") : tr("Partner replied"))} style={aidSpace} className={`msg chat-message ${side} with-actions${focused ? ' focused' : ''}${rtl ? ' rtl' : ''}${aidsReserved ? ' aids-reserved' : ''}${inspect?.open ? ' inspecting' : ''}`}>
    {body}{inspector}{actions}{failure}
    {(onSelect || focused) && <SelectionRing />}
  </div>
  if (layout === 'bubble') return bubble
  return <div className={`reading-passage${layout === 'compact' ? ' reading-passage-compact' : ''}`}>{bubble}</div>
}

/// One explicit reading request at a time for one source. A different source
/// aborts the request; its result, progress and failure never reach the new one.
function useSourceRequest(source: string | null) {
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const controller = useRef<AbortController | null>(null)
  useEffect(() => {
    setPending(false); setError(null)
    return () => { controller.current?.abort(); controller.current = null }
  }, [source])
  async function run<T>(request: (signal: AbortSignal) => Promise<T>, publish: (value: T) => void) {
    controller.current?.abort()
    const current = new AbortController(); controller.current = current
    setPending(true); setError(null)
    try {
      const value = await request(current.signal)
      if (!current.signal.aborted) publish(value)
    } catch (failure) { if (!current.signal.aborted) setError(failure) }
    finally { if (!current.signal.aborted) setPending(false) }
  }
  const cancel = () => { controller.current?.abort(); controller.current = null; setPending(false); setError(null) }
  return { pending, error, run, cancel }
}
