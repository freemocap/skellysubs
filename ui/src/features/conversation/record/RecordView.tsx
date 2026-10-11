import { useSettingsStore } from '../../../state/settings/settings'
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { useI18n } from '../../../components/localization/i18n'
import { TargetText } from '../../../components/reading/TargetText'
import { turnGraphTarget } from '../../../domain/activity/graph-target'
import { OriginalContent } from '../../../components/feedback/OriginalContent'
import { ContentValue } from '../../../components/feedback/ContentValue'
import { recordMessages, type RecordField, type RecordMessage } from '../../../domain/conversation/record-view'
import type { ConversationSnapshot } from '../../../generated/contracts'

/// The conversation as its record, over the whole screen with the recorder kept
/// at the bottom: bubbles stacked in the chat's own shape (you on the right,
/// the partner on the left, in order), and beside each bubble the fields the
/// app has filled on that message, split outward (yours to the right, the
/// partner's to the left) in two columns per side. The two sides keep their
/// own flow, so they may overlap in height. The board scales down to fit the
/// screen's width. A field opens in place to show its whole value, identifiers included.

export interface RecordViewProps {
  snapshot: ConversationSnapshot
  rtl: boolean
  languageTag: string | undefined
  onClose: () => void
  /// The recorder, kept under the record.
  children: ReactNode
}

const FIELD_GAP = 10
const COLUMN_GAP = 16
const OUT = 34
const TOP = 36
const MESSAGE_GAP = 18
const TRUNK = 18
const RADIUS = 12
const MIN_SCALE = 0.5

interface Widths { bubble: number; field: number }
interface Box { x: number; y: number }

/// Widths come from the stylesheet's tokens so the drawn cards and the computed
/// places share one source.
function readWidths(): Widths {
  const style = getComputedStyle(document.documentElement)
  const px = (token: string) => {
    const raw = style.getPropertyValue(token).trim()
    const match = /^(\d+(?:\.\d+)?)px$/.exec(raw)
    if (!match) throw new Error(`Record token ${token} must resolve to pixels, got ${JSON.stringify(raw)}.`)
    return Number(match[1])
  }
  return { bubble: px('--record-bubble-width'), field: px('--record-field-width') }
}

/// Orthogonal polyline with rounded corners.
function elbow(points: Array<[number, number]>): string {
  let d = `M${points[0][0]} ${points[0][1]}`
  for (let index = 1; index < points.length - 1; index++) {
    const p0 = points[index - 1], p1 = points[index], p2 = points[index + 1]
    const v1 = [p1[0] - p0[0], p1[1] - p0[1]], v2 = [p2[0] - p1[0], p2[1] - p1[1]]
    const l1 = Math.hypot(v1[0], v1[1]), l2 = Math.hypot(v2[0], v2[1])
    if (l1 === 0 || l2 === 0) continue
    const r = Math.min(RADIUS, l1 / 2, l2 / 2)
    d += ` L${p1[0] - v1[0] / l1 * r} ${p1[1] - v1[1] / l1 * r} Q${p1[0]} ${p1[1]} ${p1[0] + v2[0] / l2 * r} ${p1[1] + v2[1] / l2 * r}`
  }
  const last = points[points.length - 1]
  return `${d} L${last[0]} ${last[1]}`
}

interface Placement { boxes: Record<string, Box>; width: number; height: number; chatLeft: number; chatRight: number; wires: Array<{ key: string; role: string; d: string }> }

/// Places bubbles and fields from their measured heights. Each side of the chat
/// keeps its own cursor; a bubble sits at the later of the chat's next slot and
/// its side's cursor, so a side's fields never collide with that side's next message.
function place(messages: RecordMessage[], heights: Record<string, number>, widths: Widths): Placement {
  const sideWidth = OUT + widths.field * 2 + COLUMN_GAP
  const width = widths.bubble + sideWidth * 2 + TOP * 2
  const chatLeft = (width - widths.bubble) / 2, chatRight = chatLeft + widths.bubble
  const boxes: Record<string, Box> = {}
  const wires: Placement['wires'] = []
  const cursor = { user: TOP, assistant: TOP }
  let next = TOP
  for (const message of messages) {
    const side = message.role === 'user' ? 'user' : 'assistant'
    const dir = side === 'user' ? 1 : -1
    const y = Math.max(next, cursor[side])
    const bubbleHeight = heights[`m:${message.id}`]
    if (bubbleHeight === undefined) throw new Error(`The record did not measure message ${message.id}.`)
    boxes[`m:${message.id}`] = { x: chatLeft, y }
    const x1 = dir > 0 ? chatRight + OUT : chatLeft - OUT - widths.field
    const x2 = x1 + dir * (widths.field + COLUMN_GAP)
    const ys = [y, y]
    for (const field of message.fields) {
      const key = `f:${message.id}:${field.name}`
      const height = heights[key]
      if (height === undefined) throw new Error(`The record did not measure field ${key}.`)
      const column = ys[0] <= ys[1] ? 0 : 1
      const box = { x: column ? x2 : x1, y: ys[column] }
      boxes[key] = box
      ys[column] += height + FIELD_GAP
      const px = dir > 0 ? chatRight : chatLeft, py = y + 22
      const fx = dir > 0 ? box.x : box.x + widths.field, fy = box.y + 14
      const trunk = px + dir * TRUNK
      const points: Array<[number, number]> = column === 0
        ? [[px, py], [trunk, py], [trunk, fy], [fx, fy]]
        : [[px, py], [trunk, py], [trunk, y - 8], [fx - dir * 10, y - 8], [fx - dir * 10, fy], [fx, fy]]
      wires.push({ key, role: side, d: elbow(points) })
    }
    cursor[side] = Math.max(y + bubbleHeight, ys[0], ys[1]) + MESSAGE_GAP
    next = y + bubbleHeight + MESSAGE_GAP
  }
  return { boxes, width, height: Math.max(cursor.user, cursor.assistant) + TOP, chatLeft, chatRight, wires }
}

/// One field as a card: its name as the data model names it, its state, the
/// lines of its value, and the run step that filled it. Opened, it shows the
/// whole value.
function FieldCard({ field, message, conversationId }: { field: RecordField; message: RecordMessage; conversationId: string }) {
  const tr = useI18n()
  const [open, setOpen] = useState(false)
  const target = field.surface ? turnGraphTarget(conversationId, message.turn ?? undefined, field.surface) : {}
  return <div className="record-field" data-error={field.error ? '' : undefined} data-open={open ? '' : undefined} {...target}>
    <button type="button" className="record-field-head" aria-expanded={open} onClick={() => setOpen(current => !current)} title={open ? tr('Show less') : tr('Show the whole value')}>
      <span className="record-field-name">{field.name}</span>
      {field.state && <span className="record-field-state" data-state={field.state}>{field.state}</span>}
      <ToolbarIcon name="chevron" size={12} />
    </button>
    {field.error && <div className="record-field-error">{field.error}</div>}
    {field.value !== null && <div className="record-field-body" dir="auto"><ContentValue value={field.value} compact={!open} /></div>}
    {field.results.length > 0 && <div className="record-field-results" title={tr('The run step that fills this field')}>← {field.results.join(', ')}</div>}
  </div>
}

export function RecordView({ snapshot, rtl, languageTag, onClose, children }: RecordViewProps) {
  const audioSettings = useSettingsStore(state => state.settings)
  const playback = { rate: audioSettings?.tts_rate ?? 1, volume: audioSettings ? audioSettings.master_volume * audioSettings.voice_volume / 10000 : 0 }
  const tr = useI18n()
  const messages = useMemo(() => recordMessages(snapshot), [snapshot])
  const board = useRef<HTMLDivElement>(null)
  const scroller = useRef<HTMLDivElement>(null)
  const [placement, setPlacement] = useState<Placement | null>(null)
  const [available, setAvailable] = useState(0)

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => { if (event.key === 'Escape') onClose() }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  useLayoutEffect(() => {
    const element = scroller.current
    if (!element) throw new Error('The record view did not mount its scroller.')
    const measure = () => setAvailable(element.clientWidth)
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  // Cards are placed from their measured heights, and placed again whenever a
  // card changes size (a font arriving, a field opening).
  useLayoutEffect(() => {
    const element = board.current
    if (!element) throw new Error('The record view did not mount its board.')
    const cards = [...element.querySelectorAll<HTMLElement>('[data-key]')]
    const measure = () => {
      const heights: Record<string, number> = {}
      for (const node of cards) heights[node.dataset.key!] = node.offsetHeight
      setPlacement(place(messages, heights, readWidths()))
    }
    measure()
    const observer = new ResizeObserver(measure)
    for (const node of cards) observer.observe(node)
    return () => observer.disconnect()
  }, [messages])

  const scale = placement && available ? Math.max(MIN_SCALE, Math.min(1, available / placement.width)) : 1
  const at = (key: string) => placement?.boxes[key] ? { left: placement.boxes[key].x, top: placement.boxes[key].y } : undefined
  return createPortal(<div className="record-screen" role="dialog" aria-modal="true" aria-label={tr('Data')}>
    <header className="record-head">
      <ToolbarIcon name="record" size={15} /><h2>{tr('Data')}</h2>
      <span className="record-head-note">{tr('{value0} messages · revision {value1}', { value0: snapshot.messages.length, value1: snapshot.revision })}</span>
      <button type="button" className="ai-icon-button" onClick={onClose} aria-label={tr('Close')} title={tr('Close')}><ToolbarIcon name="close" size={15} /></button>
    </header>
    <div className="record" ref={scroller} data-placed={placement ? '' : undefined}>
      <div className="record-scaler" style={placement ? { width: placement.width * scale, height: placement.height * scale } : undefined}>
        <div className="record-board" ref={board} style={placement ? { width: placement.width, height: placement.height, transform: `scale(${scale})` } : undefined}>
          {placement && <div className="record-chat-column" style={{ left: placement.chatLeft - 12, width: placement.chatRight - placement.chatLeft + 24 }}><span>{tr('chat')}</span></div>}
          <svg className="record-wires" aria-hidden="true" width={placement?.width ?? 0} height={placement?.height ?? 0}>
            {placement?.wires.map(wire => <path key={wire.key} d={wire.d} data-role={wire.role} />)}
          </svg>
          {messages.map(message => <article key={message.id} className={`record-msg msg chat-message ${message.role === 'user' ? 'me' : 'bot'}${rtl ? ' rtl' : ''}`} data-role={message.role} data-key={`m:${message.id}`} style={at(`m:${message.id}`)}
            aria-label={tr('{value0} · messages[{value1}]', { value0: message.role, value1: message.sequence })}>
            <div className="record-msg-who"><span>{tr('{value0} · messages[{value1}]', { value0: message.role, value1: message.sequence })}</span><span>{tr.dateTime(new Date(message.createdAt))}</span></div>
            <div className="record-msg-text" lang={languageTag} dir={rtl ? 'rtl' : 'ltr'}><TargetText text={message.text} interactive={false} /></div>
            {message.role === 'user' && message.turn?.recordingIds?.map(recording => <OriginalContent playback={playback} key={recording} source={{ recording }} />)}
            {message.turn && <div className="record-msg-turn">
              <span>{tr('Turn {value0}', { value0: message.turn.id.slice(0, 8) })}</span>{message.turn.channel && <span>{message.turn.channel}</span>}<span>{message.turn.state}</span>
              {message.turn.nativeGraph && <span>{tr('Revision {value0}', { value0: message.turn.nativeGraph.revision })}</span>}
            </div>}
          </article>)}
          {messages.flatMap(message => message.fields.map(field => <div key={`${message.id}:${field.name}`} className="record-field-slot" data-key={`f:${message.id}:${field.name}`} style={at(`f:${message.id}:${field.name}`)}>
            <FieldCard field={field} message={message} conversationId={snapshot.conversationId} />
          </div>))}
        </div>
      </div>
    </div>
    <div className="record-composer">{children}</div>
  </div>, document.body)
}
