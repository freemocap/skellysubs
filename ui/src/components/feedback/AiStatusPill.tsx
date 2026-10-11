import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import type { AiStatusLine } from '../../domain/activity/ai-status'
import { humanizeKind } from '../../domain/conversation/activity-summary'
import { useI18n } from '../localization/i18n'

/// How long a line stays before the next replaces it: long enough to read
/// three words at a glance, short enough to trail the work by one step.
const HOLD_MS = 800

/// The checked AI connection the pill shows; the state layer supplies it.
export interface AiPillAccess {
  connected: boolean
  checking: boolean
  error: string | null
  checkedAt: number | null
}

/// The AI at a recorder: a pill that opens the AI panel, grey at rest with a
/// green mark while AI is connected, circled by a moving edge while AI work
/// runs, and beside it the step under way in the longest wording that fits
/// (three words, two, one). `fallback` takes the line's place at rest.
export function AiStatusPill({ access, open, onOpen, busy, line, beside, fallback = null }: {
  access: AiPillAccess
  /// The AI View is showing, docked or in its own window.
  open: boolean
  onOpen: () => void
  busy: boolean
  line: AiStatusLine | null
  /// Controls that sit right after the pill, before the step line.
  beside: ReactNode
  fallback?: ReactNode
}) {
  const tr = useI18n()
  const shown = useSteadyLine(line)
  const detail = access.checking ? tr('Checking AI connection…') : access.error
    ? access.error : access.checkedAt ? `${tr('Last checked')}: ${tr.dateTime(access.checkedAt)}` : tr('Connection not checked yet')
  const wordings = shown ? shown.words?.map(key => tr(key)) ?? [`${humanizeKind(shown.kinds[0] ?? '')}…`] : []
  return <div className="ai-status" data-busy={busy || undefined} data-tone={shown?.tone}>
    <button type="button" className="ai-status-pill" data-configured={access.connected} onClick={onOpen}
      aria-label={access.connected ? tr('AI Connected') : access.checking ? tr('Checking AI connection…') : tr('AI Not Connected')} title={detail}
      aria-expanded={access.connected || access.checking ? open : undefined} aria-controls={access.connected || access.checking ? 'ai-activity' : undefined}>
      <span className="ai-status-pill-surface" aria-hidden="true" />
      <span aria-hidden="true">{tr('AI')}</span>
    </button>
    {beside}
    {shown ? <FittedLine key={shown.id} wordings={wordings} detail={[...shown.kinds, ...shown.models].join(' · ') || undefined} /> : fallback}
    <span className="ai-status-announce" role="status">{shown?.announce ? wordings[0] : ''}</span>
  </div>
}

/// One wording of the step: the longest that fits the line's width, measured
/// from hidden copies so each script and translation measures as it renders.
/// Nothing shows when even the shortest does not fit.
function FittedLine({ wordings, detail }: { wordings: readonly string[]; detail?: string }) {
  const line = useRef<HTMLSpanElement>(null)
  const copies = useRef<HTMLSpanElement>(null)
  const fit = useFittedIndex(line, copies, wordings.join('\n'))
  return <span ref={line} className="ai-status-line" title={detail}>
    {fit >= 0 && <span className="ai-status-text">{wordings[fit]}</span>}
    <span ref={copies} className="ai-status-measure" aria-hidden="true">{wordings.map((wording, index) => <span key={index}>{wording}</span>)}</span>
  </span>
}

function useFittedIndex(line: RefObject<HTMLElement | null>, copies: RefObject<HTMLElement | null>, key: string): number {
  const [fit, setFit] = useState(0)
  useLayoutEffect(() => {
    const box = line.current
    const measured = copies.current
    if (!box || !measured) return
    const measure = () => {
      const widths = [...measured.children].map(copy => copy.getBoundingClientRect().width)
      setFit(widths.findIndex(width => width <= box.clientWidth))
    }
    measure()
    if (typeof ResizeObserver === 'undefined') return
    // The line resizes with the window; the copies resize when a font arrives.
    const observer = new ResizeObserver(measure)
    observer.observe(box)
    observer.observe(measured)
    return () => observer.disconnect()
  }, [line, copies, key])
  return fit
}

/// Holds each step on screen for HOLD_MS before a different one replaces it,
/// so fast steps still read as a stream. From rest a step shows at once, and
/// the newest step always wins; the held step keeps its latest details.
function useSteadyLine(line: AiStatusLine | null): AiStatusLine | null {
  const [held, setHeld] = useState(line)
  const since = useRef(Date.now())
  const latest = useRef(line)
  latest.current = line
  const id = line?.id ?? null
  const heldId = held?.id ?? null
  useEffect(() => {
    if (id === heldId) return
    const show = () => { since.current = Date.now(); setHeld(latest.current) }
    const wait = heldId === null ? 0 : since.current + HOLD_MS - Date.now()
    if (wait <= 0) { show(); return }
    const timer = setTimeout(show, wait)
    return () => clearTimeout(timer)
  }, [id, heldId])
  return line?.id === heldId ? line : held
}
