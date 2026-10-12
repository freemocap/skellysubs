import { ReadingTemplateContext } from './ReadingContext'
import { sentenceBlanks } from '../../domain/reading/sentence-blanks'
import { createPortal } from 'react-dom'
import { positionWordHelp, wordHelpLayer } from './word-help-layer'
import { WordHelpContent } from './WordHelpContent'
import { UnannotatedText } from './UnannotatedText'
import { SpeechFollowText } from './SpeechFollowText'
import { useOverlayLayer } from '../dialogs/useOverlayLayer'
import { useI18n } from '../localization/i18n'
import { useUiDirection } from '../localization/useUiDirection'
import { useReadingPreferences } from './ReadingPreferences'
import { glossDisplayGroups } from '../../domain/reading/gloss-display'
import { Fragment, useContext, useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import type { GlossSegment } from '../../generated/contracts'

function PinnedGlossLayer({ host, onClose }: { host: RefObject<HTMLSpanElement | null>; onClose: () => void }) {
  useOverlayLayer(host, onClose, false)
  return null
}

/** Saved UTF-16 anchors select exact source occurrences; reading never requests analysis. */
export function SavedGlossText({ text, segments, afterSegment, decorateSegment, interactive = true, showAids = true, revealAids = false, showSound, graphTarget }: { graphTarget?: Record<string, string>; showSound?: boolean; revealAids?: boolean; showAids?: boolean; interactive?: boolean; text: string; segments: GlossSegment[]; afterSegment?: (start: number, end: number) => ReactNode; decorateSegment?: (node: ReactNode, start: number, end: number) => ReactNode }) {
  const tr = useI18n()
  const uiDirection = useUiDirection()
  const { autoTranslate, alwaysRomanize, alwaysPronunciation, supportsRomanization } = useReadingPreferences()
  const [revealed, setRevealed] = useState<Set<number>>(() => new Set())
  const [hovered, setHovered] = useState<number | null>(null)
  const hoveredWord = useRef<HTMLElement | null>(null)
  const helper = useRef<HTMLSpanElement>(null)
  const helperId = useId()
  const layer = wordHelpLayer(hoveredWord.current)
  const renderHelp = (node: ReactNode) => layer.popover ? node : createPortal(node, layer.host)
  const hoverExit = useRef<ReturnType<typeof setTimeout> | null>(null)
  const keepHover = () => { if (hoverExit.current !== null) clearTimeout(hoverExit.current); hoverExit.current = null }
  const leaveHover = () => { keepHover(); hoverExit.current = setTimeout(() => setHovered(null), 200) }
  useEffect(() => () => { if (hoverExit.current !== null) clearTimeout(hoverExit.current) }, [])
  // Both hosts of this text — the reply tray and the message stream — scroll and
  // clip their own box, so a helper anchored inside one of them is cut off at its
  // edge. Desktop uses the top layer; touch uses an unclipped portal to avoid
  // the device's top-layer scaling bug. Both stay beside the source word.
  useLayoutEffect(() => {
    const element = helper.current
    const word = hoveredWord.current
    if (!element || !word) return
    if (element.hasAttribute('popover')) element.showPopover()
    const position = () => positionWordHelp(element, word)
    position()
    const observer = new ResizeObserver(position); observer.observe(element)
    window.visualViewport?.addEventListener('resize', position)
    window.visualViewport?.addEventListener('scroll', position)
    window.addEventListener('resize', position)
    window.addEventListener('scroll', position, true)
    const dismiss = (event: Event) => {
      if (event instanceof KeyboardEvent ? event.key !== 'Escape' : word.contains(event.target as Node) || element.contains(event.target as Node)) return
      setRevealed(new Set()); setHovered(null)
      if (event instanceof KeyboardEvent) word.focus()
    }
    document.addEventListener('pointerdown', dismiss)
    document.addEventListener('keydown', dismiss)
    return () => {
      observer.disconnect()
      window.visualViewport?.removeEventListener('resize', position)
      window.visualViewport?.removeEventListener('scroll', position)
      window.removeEventListener('resize', position); window.removeEventListener('scroll', position, true)
      document.removeEventListener('pointerdown', dismiss); document.removeEventListener('keydown', dismiss)
      if (element.isConnected && element.hasAttribute('popover')) element.hidePopover()
    }
  }, [hovered, revealed, autoTranslate, alwaysRomanize, alwaysPronunciation, showAids])
  useEffect(() => { setRevealed(new Set()); setHovered(null) }, [text])
  const pieces = []
  let cursor = 0
  const template = useContext(ReadingTemplateContext)
  const blanks = sentenceBlanks(text, template)
  const wordSegments = segments.filter(segment => !blanks.some(blank => segment.start < blank.end && segment.end > blank.start))
  for (const segment of glossDisplayGroups(text, wordSegments)) {
    const annotations = segment.parts.filter(part => part.kind === 'gloss' && part.gloss !== null)
    // Under the word, a clitic group reads as one word: its sounds run together
    // ("al-" + "aklah" → "al-aklah") and its meanings read as a phrase. The
    // per-part values stay in the helper, where there is room.
    const phrase = (field: 'gloss' | 'romanization' | 'pronunciation') => annotations.map(part => field === 'pronunciation' && part.romanization ? undefined : part[field]).filter(Boolean).join(field === 'gloss' ? ' ' : '')
    const joined = (field: 'gloss' | 'romanization' | 'pronunciation', className: string) => {
      const value = phrase(field)
      return value ? <span className={className} dir="auto" data-gloss-start={segment.start} data-gloss-end={segment.end} {...graphTarget}>{value}</span> : null
    }
    if (segment.start > cursor) pieces.push(<Fragment key={`gap-${cursor}`}><UnannotatedText inline source={{ text, start: cursor }} text={text.slice(cursor, segment.start)} interactive={interactive} /></Fragment>)
    const source = text.slice(segment.start, segment.end)
    const open = revealed.has(segment.start)
    const hovering = hovered === segment.start && !open
    const toggle = () => {
      setHovered(null)
      setRevealed(previous => {
      return previous.has(segment.start) ? new Set<number>() : new Set([segment.start])
    })
    }
    const piece = interactive && annotations.length > 0
      ? <span className="wu saved-word" key={segment.start} data-source-start={segment.start} data-source-end={segment.end} onPointerEnter={event => { keepHover(); if (event.pointerType === 'mouse' && revealed.size === 0) { hoveredWord.current = event.currentTarget; setHovered(segment.start) } }} onPointerLeave={leaveHover}>
          <span data-speech-source className={`reading-word${open ? ' revealed' : ''}`} role="button" tabIndex={0} aria-expanded={open} aria-controls={open || hovering ? helperId : undefined}
            onClick={event => { event.stopPropagation(); hoveredWord.current = event.currentTarget; toggle() }}
            onDoubleClick={event => event.stopPropagation()}
            onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); event.stopPropagation(); hoveredWord.current = event.currentTarget; toggle() } }}>
            {source}
          </span>
          {(open || hovering) && renderHelp(<span id={helperId} ref={helper} className={`saved-word-help${segment.parts.length > 1 ? ' gloss-fragments' : ''}`} dir={uiDirection} popover={layer.popover ? "manual" : undefined} data-word-help-layer={!layer.popover ? "portal" : undefined}
            role="group" aria-label={tr("Word help")} {...graphTarget}
            onPointerEnter={keepHover} onPointerLeave={leaveHover}
            onClick={event => event.stopPropagation()}>
            {open && <PinnedGlossLayer host={helper} onClose={() => { setRevealed(new Set()); setHovered(null); hoveredWord.current?.focus() }} />}
            <WordHelpContent text={text} start={segment.start} end={segment.end} parts={annotations}
              onClose={() => { setRevealed(new Set()); setHovered(null) }} />
          </span>)}
          {showAids && (revealAids || autoTranslate) && joined('gloss', 'wg')}
          {(showSound ?? (showAids && (revealAids || alwaysRomanize))) && supportsRomanization && joined('romanization', 'wroman')}
          {(showSound ?? (showAids && (revealAids || alwaysPronunciation))) && joined('pronunciation', 'wpronunciation')}
        </span>
      : <UnannotatedText inline source={{ text, start: segment.start }} key={segment.start} text={source} interactive={interactive} />
    pieces.push(decorateSegment ? <Fragment key={segment.start}>{decorateSegment(piece, segment.start, segment.end)}</Fragment> : piece)
    if (afterSegment) pieces.push(<Fragment key={`credit-${segment.start}`}>{afterSegment(cursor, segment.end)}</Fragment>)
    cursor = segment.end
  }
  if (cursor < text.length) pieces.push(<Fragment key={`gap-${cursor}`}><UnannotatedText inline source={{ text, start: cursor }} text={text.slice(cursor)} interactive={interactive} /></Fragment>)
  if (afterSegment && cursor < text.length) pieces.push(<Fragment key="credit-tail">{afterSegment(cursor, text.length)}</Fragment>)
  return <SpeechFollowText text={text}><span className="w preserve-space" dir="auto">{pieces}</span></SpeechFollowText>
}
