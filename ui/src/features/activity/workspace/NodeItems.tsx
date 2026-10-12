import { useState, type PointerEvent } from 'react'
import { useI18n } from '../../../components/localization/i18n'
import { TargetText } from '../../../components/reading/TargetText'
import { ContentValue } from '../../../components/feedback/ContentValue'
import type { StepProduct } from '../../../domain/conversation/record-view'
import type { GraphOverlay, NodeOverlay, Tone } from '../../../domain/activity/graph-overlay'
import type { GraphView, InputBinding } from '../../../domain/activity/graph-projection'
import { humanizeId, typeName } from '../../../domain/activity/graph-types'
import { cardMetrics, type PlacedFrame, type PlacedItem, type WorkspaceGeometry } from './layout'
import { typeColor } from './geometry'
import { dispositionWord, footerText, headline, nodeName, nodeVerb } from './wording'

export type HoverTarget = { kind: 'operation' | 'station'; id: string }

export interface Preview {
  node: string
  text: string
  live: boolean
}

/// A station outside the artifact, as the caller describes it: what it is
/// called, the facts it shows, and the graph ports that join it to the run.
export interface StationView {
  id: string
  side: 'before' | 'after'
  name: string
  rows: Array<{ label: string; value: string }>
  tone: Tone | 'none'
  ports: string[]
}

/// Which edge of a card a resize gesture holds.
export type ResizeEdge = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw'
export const RESIZE_EDGES: readonly ResizeEdge[] = ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw']

/// Pointer interaction the workspace owns: dragging an item and resizing a card.
/// A click that follows a drag is swallowed.
export interface Interaction {
  dragStart: (id: string, event: PointerEvent) => void
  resizeStart: (id: string, edge: ResizeEdge, event: PointerEvent) => void
  clickAllowed: () => boolean
}

interface ItemProps {
  view: GraphView
  overlay: GraphOverlay | null
  geometry: WorkspaceGeometry
  interaction: Interaction
  onHover: (target: HoverTarget | null) => void
}

const box = (item: PlacedItem) => ({ left: item.x, top: item.y, width: item.width, height: item.height })
const moved = (item: PlacedItem) => item.moved || undefined
const toneOf = (overlay: NodeOverlay | null): Tone | 'none' => overlay?.tone ?? 'none'

/// One glyph per tone, the way a node editor marks a node's state in its header.
export const TONE_GLYPH = { done: '✓', running: '●', waiting: '◌', ask: '○', held: '‖', problem: '!', ended: '×', none: '' } as const satisfies Record<Tone | 'none', string>

function hoverHandlers(onHover: ItemProps['onHover'], target: HoverTarget) {
  return {
    onPointerEnter: () => onHover(target), onPointerLeave: () => onHover(null),
    onFocus: () => onHover(target), onBlur: () => onHover(null),
  }
}

function StateGlyph({ node, tr }: { node: NodeOverlay | null; tr: ReturnType<typeof useI18n> }) {
  if (!node) return null
  const label = node.tone === 'running' && node.attempts.length > 1 ? tr('Retrying') : dispositionWord(tr, node.disposition)
  return <span className="ai-node-glyph" data-tone={node.tone} title={label} aria-label={label}>{TONE_GLYPH[node.tone]}</span>
}

/// Collapsed operation: the header bar alone, with one socket per side where the
/// artifact wires anything in or out.
export function OperationBubble({ item, view, overlay, interaction, onHover, onOpen }: ItemProps & { item: Extract<PlacedItem, { kind: 'operation' }>; onOpen: () => void }) {
  const tr = useI18n()
  const operation = view.operations[item.id]
  const node = overlay?.nodes[item.id] ?? null
  const name = nodeName(view, item.id)
  const wired = operation.inputs.find(input => input.kind === 'wired')
  const output = operation.outputs[0]
  return <button type="button" className="ai-node is-collapsed" data-tone={toneOf(node)} data-resource={operation.resource} data-id={item.id} data-moved={moved(item)}
    style={box(item)} aria-label={tr('{value0}. {value1}', { value0: name, value1: headline(view, item.id) })} title={tr('Everything recorded about {value0}', { value0: name })}
    onPointerDown={event => interaction.dragStart(item.id, event)} onClick={() => { if (interaction.clickAllowed()) onOpen() }}
    {...hoverHandlers(onHover, { kind: 'operation', id: item.id })}>
    {wired && wired.kind === 'wired' && <span className="ai-socket is-in" style={{ background: typeColor(wired.contract) }} aria-hidden="true" />}
    <span className="ai-node-name">{name}</span>
    <StateGlyph node={node} tr={tr} />
    {output && <span className="ai-socket is-out" style={{ background: typeColor(output.contract) }} aria-hidden="true" />}
  </button>
}

function ResizeEdges({ id, interaction }: { id: string; interaction: Interaction }) {
  return <>{RESIZE_EDGES.map(edge => <span key={edge} className="ai-node-edge" data-edge={edge} role="presentation" onPointerDown={event => interaction.resizeStart(id, edge, event)} />)}</>
}

/// An input read in place: a captured graph input named after the input it
/// reads, or a constant the artifact fixed.
function MarkRow({ input, tr }: { input: InputBinding; tr: ReturnType<typeof useI18n> }) {
  const detail = input.kind === 'captured' ? humanizeId(input.input).toLowerCase() : tr('constant')
  const title = input.kind === 'captured' ? tr('Graph input {value0} · {value1}', { value0: input.input, value1: typeName(input.contract) }) : typeName(input.contract)
  return <div className="ai-row is-mark" title={title}>
    <span className="ai-mark" style={{ color: typeColor(input.contract) }} aria-hidden="true">⊳</span>
    <span className="ai-row-name">{humanizeId(input.port).toLowerCase()}</span>
    <span className="ai-row-detail">{detail}</span>
  </div>
}

/// Open operation: header, one row per socket, inputs read in place, outputs,
/// what it made (the text as it streams, or the conversation fields its results
/// fill), and a footer with its state, its counted access inputs and the
/// details button. The slot opens the details.
export function OperationCard({ item, view, overlay, geometry, interaction, preview, products, onHover, onOpen }: ItemProps & {
  item: Extract<PlacedItem, { kind: 'operation' }>; preview: Preview | null; products: StepProduct[]; onOpen: () => void
}) {
  const tr = useI18n()
  const operation = view.operations[item.id]
  const node = overlay?.nodes[item.id] ?? null
  const name = nodeName(view, item.id)
  const metrics = cardMetrics(operation, geometry)
  const wired = operation.inputs.filter(input => input.kind === 'wired')
  const [properties, setProperties] = useState(false)
  const shown = preview?.node === item.id ? preview : null
  return <article className="ai-node" data-tone={toneOf(node)} data-resource={operation.resource} data-id={item.id} data-moved={moved(item)}
    style={box(item)} {...hoverHandlers(onHover, { kind: 'operation', id: item.id })}>
    <div className="ai-node-head" onPointerDown={event => interaction.dragStart(item.id, event)} title={nodeVerb(view, item.id)}>
      <span className="ai-node-name">{name}</span>
      <StateGlyph node={node} tr={tr} />
    </div>
    {wired.map(input => input.kind === 'wired' && <div className="ai-row is-in" key={input.port} title={`${nodeName(view, input.producer.node)} · ${typeName(input.contract)}`}>
      <span className="ai-socket is-in" style={{ background: typeColor(input.contract) }} aria-hidden="true" />
      <span className="ai-row-name">{humanizeId(input.port).toLowerCase()}</span>
    </div>)}
    {metrics.guardRow && operation.guard?.kind === 'wired' && <div className="ai-row is-in" title={`${nodeName(view, operation.guard.producer.node)} · ${typeName(operation.guard.contract)}`}>
      <span className="ai-socket is-in is-guard" style={{ background: typeColor(operation.guard.contract) }} aria-hidden="true" />
      <span className="ai-row-name">{tr('only if')}</span>
    </div>}
    {metrics.markRows.map(input => <MarkRow key={input.port} input={input} tr={tr} />)}
    <div className="ai-node-divider" />
    {operation.outputs.map(output => <div className="ai-row is-out" key={output.port} title={typeName(output.contract)}>
      <span className="ai-row-name">{humanizeId(output.port).toLowerCase()}</span>
      <span className="ai-socket is-out" style={{ background: typeColor(output.contract) }} aria-hidden="true" />
    </div>)}
    {metrics.slot && <div className="ai-node-slot" aria-live={shown?.live ? 'polite' : undefined} data-empty={shown || products.length ? undefined : 'true'}
      role={shown || products.length ? 'button' : undefined} tabIndex={shown || products.length ? 0 : undefined} title={tr('Everything recorded about {value0}', { value0: name })}
      onClick={onOpen} onKeyDown={event => { if (event.key === 'Enter') onOpen() }}>
      {shown ? (shown.live
        ? <div className="ai-node-text" dir="auto">{shown.text}<span className="ai-caret" aria-hidden="true" /></div>
        : <div className="ai-node-text"><TargetText text={shown.text} interactive={false} /></div>)
      : products.length > 0 && <div className="ai-node-text">
          {products.map(product => <div key={`${product.role}:${product.field}`} className="ai-node-product">
            <span className="ai-node-product-name">{product.field}</span>
            <ContentValue value={product.value} compact />
          </div>)}
        </div>}
    </div>}
    <div className="ai-node-foot">
      {node && <span className="ai-node-state">{footerText(tr, view, node)}</span>}
      {metrics.fixed.length > 0 && <button type="button" className="ai-node-props" aria-expanded={properties} onClick={() => setProperties(open => !open)}
        title={tr('Inputs fixed when the run started')}>+{metrics.fixed.length}</button>}
      <button type="button" className="ai-icon-button" onClick={onOpen} aria-label={tr('Everything recorded about {value0}', { value0: name })} title={tr('Details')}>{tr('Details')}</button>
    </div>
    {properties && <ul className="ai-node-proplist" aria-label={tr('Inputs fixed when the run started')}>
      {metrics.fixed.map(port => <li key={port}>{port === 'guard' ? tr('only if') : humanizeId(port).toLowerCase()}</li>)}
    </ul>}
    <ResizeEdges id={item.id} interaction={interaction} />
  </article>
}

/// A station outside the artifact: a dashed card with the facts the caller
/// supplied, joined to the graph by hand-off wires.
export function StationCard({ item, station, interaction, onHover }: { item: Extract<PlacedItem, { kind: 'station' }>; station: StationView; interaction: Interaction; onHover: ItemProps['onHover'] }) {
  return <article className="ai-node is-station" data-tone={station.tone} data-side={station.side} data-id={item.id} data-moved={moved(item)} style={box(item)}
    aria-label={station.name} {...hoverHandlers(onHover, { kind: 'station', id: item.id })}>
    <div className="ai-node-head" onPointerDown={event => interaction.dragStart(item.id, event)}>
      <span className="ai-node-name">{station.name}</span>
      {station.tone !== 'none' && <span className="ai-node-glyph" data-tone={station.tone} aria-hidden="true">{TONE_GLYPH[station.tone]}</span>}
    </div>
    {station.rows.map(row => <div className="ai-row is-fact" key={row.label} title={row.label}>
      <span className="ai-row-name">{row.label}</span>
      <span className="ai-row-detail">{row.value}</span>
    </div>)}
    <div className="ai-node-divider" />
  </article>
}

/// A composition group's frame: the box around its members, named once.
export function GroupFrame({ frame }: { frame: PlacedFrame }) {
  const name = humanizeId(frame.id.split('/').join(' '))
  return <section className="ai-group-frame" style={{ left: frame.x, top: frame.y, width: frame.width, height: frame.height }} aria-label={name} title={frame.id}>
    <span className="ai-group-head">{name}</span>
  </section>
}
