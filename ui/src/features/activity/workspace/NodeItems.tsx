import { useState, type KeyboardEvent, type PointerEvent } from 'react'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { useI18n } from '../../../components/localization/i18n'
import { TargetText } from '../../../components/reading/TargetText'
import type { GraphOverlay, NodeOverlay, Tone } from '../../../domain/activity/graph-overlay'
import type { GraphView } from '../../../domain/activity/graph-projection'
import { humanizeId, typeName } from '../../../domain/activity/graph-types'
import { cardMetrics, type PlacedItem, type WorkspaceGeometry } from './layout'
import { typeColor } from './geometry'
import { dispositionWord, footerText, headline, nodeName, nodeVerb } from './wording'

export type HoverTarget = { kind: 'operation' | 'group'; id: string }

export interface Preview {
  node: string
  text: string
  live: boolean
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
const TONE_GLYPH = { done: '✓', running: '●', waiting: '◌', ask: '○', held: '‖', problem: '!', ended: '×', none: '' } as const satisfies Record<Tone | 'none', string>

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
export function OperationBubble({ item, view, overlay, interaction, onHover, onExpand }: ItemProps & { item: Extract<PlacedItem, { kind: 'operation' }>; onExpand: () => void }) {
  const tr = useI18n()
  const operation = view.operations[item.id]
  const node = overlay?.nodes[item.id] ?? null
  const name = nodeName(view, item.id)
  const wired = operation.inputs.find(input => input.kind === 'wired')
  const output = operation.outputs[0]
  return <button type="button" className="ai-node is-collapsed" data-tone={toneOf(node)} data-resource={operation.resource} data-id={item.id} data-moved={moved(item)}
    style={box(item)} aria-label={tr('{value0}. {value1}', { value0: name, value1: headline(view, item.id) })} title={tr('Expand {value0}', { value0: name })}
    onPointerDown={event => interaction.dragStart(item.id, event)} onClick={() => { if (interaction.clickAllowed()) onExpand() }}
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

/// Open operation: header, one row per socket, outputs, the text it produced,
/// and a footer with its state, its fixed properties and the details button.
export function OperationCard({ item, view, overlay, geometry, interaction, preview, onHover, onOpen, onShrink }: ItemProps & {
  item: Extract<PlacedItem, { kind: 'operation' }>; preview: Preview | null; onOpen: () => void; onShrink: () => void
}) {
  const tr = useI18n()
  const operation = view.operations[item.id]
  const node = overlay?.nodes[item.id] ?? null
  const name = nodeName(view, item.id)
  const metrics = cardMetrics(view, operation, geometry)
  const wired = operation.inputs.filter(input => input.kind === 'wired')
  const [properties, setProperties] = useState(false)
  const onKeyDown = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.stopPropagation(); if (properties) setProperties(false); else onShrink() } }
  const shown = preview?.node === item.id ? preview : null
  return <article className="ai-node" data-tone={toneOf(node)} data-resource={operation.resource} data-id={item.id} data-moved={moved(item)}
    style={box(item)} onKeyDown={onKeyDown} {...hoverHandlers(onHover, { kind: 'operation', id: item.id })}>
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
    <div className="ai-node-divider" />
    {operation.outputs.map(output => <div className="ai-row is-out" key={output.port} title={typeName(output.contract)}>
      <span className="ai-row-name">{humanizeId(output.port).toLowerCase()}</span>
      <span className="ai-socket is-out" style={{ background: typeColor(output.contract) }} aria-hidden="true" />
    </div>)}
    {metrics.slot && <div className="ai-node-slot" aria-live={shown?.live ? 'polite' : undefined} data-empty={shown ? undefined : 'true'}>
      {shown && (shown.live
        ? <div className="ai-node-text" dir="auto">{shown.text}<span className="ai-caret" aria-hidden="true" /></div>
        : <div className="ai-node-text"><TargetText text={shown.text} interactive={false} /></div>)}
    </div>}
    <div className="ai-node-foot">
      {node && <span className="ai-node-state">{footerText(tr, view, node)}</span>}
      {metrics.fixed.length > 0 && <button type="button" className="ai-node-props" aria-expanded={properties} onClick={() => setProperties(open => !open)}
        title={tr('Inputs fixed when the run started')}>+{metrics.fixed.length}</button>}
      <button type="button" className="ai-icon-button" onClick={onOpen} aria-label={tr('Everything recorded about {value0}', { value0: name })} title={tr('Details')}><ToolbarIcon name="more" size={13} /></button>
      <button type="button" className="ai-icon-button" onClick={onShrink} aria-label={tr('Shrink {value0}', { value0: name })} title={tr('Shrink {value0}', { value0: name })}><ToolbarIcon name="collapse" size={13} /></button>
    </div>
    {properties && <ul className="ai-node-proplist" aria-label={tr('Inputs fixed when the run started')}>
      {metrics.fixed.map(port => <li key={port}>{port === 'guard' ? tr('only if') : humanizeId(port).toLowerCase()}</li>)}
    </ul>}
    <ResizeEdges id={item.id} interaction={interaction} />
  </article>
}

function memberLabel(group: string, member: string): string {
  return member.slice(group.length + 1).split('/').map(humanizeId).join(' / ')
}

function groupName(group: string): string {
  return humanizeId(group.slice(group.lastIndexOf('/') + 1))
}

/// Folded composition, collapsed: a header bar with the members' state glyphs.
export function GroupBubble({ item, view, overlay, interaction, onHover, onExpand }: ItemProps & { item: Extract<PlacedItem, { kind: 'group' }>; onExpand: () => void }) {
  const tr = useI18n()
  const name = groupName(item.id)
  return <button type="button" className="ai-node is-collapsed is-group" style={box(item)} data-id={item.id} data-moved={moved(item)}
    aria-label={tr('{value0}, {value1}', { value0: name, value1: tr('{value0} steps', { value0: item.members.length }) })} title={tr('Expand {value0}', { value0: name })}
    onPointerDown={event => interaction.dragStart(item.id, event)} onClick={() => { if (interaction.clickAllowed()) onExpand() }}
    {...hoverHandlers(onHover, { kind: 'group', id: item.id })}>
    <span className="ai-socket is-in" aria-hidden="true" />
    <span className="ai-node-name">{name}</span>
    <span className="ai-node-members" aria-hidden="true">{item.members.map(member => <span key={member} className="ai-node-glyph" data-tone={toneOf(overlay?.nodes[member] ?? null)}>{TONE_GLYPH[toneOf(overlay?.nodes[member] ?? null)]}</span>)}</span>
    <span className="ai-socket is-out" aria-hidden="true" />
    <span className="ai-sr-only">{item.members.map(member => nodeVerb(view, member)).join(', ')}</span>
  </button>
}

/// Folded composition, open: one row per member with its own sockets.
export function GroupCard({ item, view, overlay, interaction, onHover, onUnfold, onShrink }: ItemProps & { item: Extract<PlacedItem, { kind: 'group' }>; onUnfold: () => void; onShrink: () => void }) {
  const tr = useI18n()
  const name = groupName(item.id)
  return <article className="ai-node is-group" style={box(item)} data-id={item.id} data-moved={moved(item)} onKeyDown={event => { if (event.key === 'Escape') { event.stopPropagation(); onShrink() } }}
    {...hoverHandlers(onHover, { kind: 'group', id: item.id })}>
    <div className="ai-node-head" onPointerDown={event => interaction.dragStart(item.id, event)}>
      <ToolbarIcon name="layers" size={13} /><span className="ai-node-name">{name}</span>
    </div>
    {item.members.map(member => {
      const node = overlay?.nodes[member] ?? null
      return <div className="ai-row is-member" key={member} title={headline(view, member)}>
        <span className="ai-socket is-in" aria-hidden="true" />
        <span className="ai-row-name">{memberLabel(item.id, member)}</span>
        <StateGlyph node={node} tr={tr} />
        <span className="ai-socket is-out" aria-hidden="true" />
      </div>
    })}
    <div className="ai-node-foot">
      <button type="button" className="ai-node-props" onClick={onUnfold} title={tr('Show each step as its own item')}><ToolbarIcon name="expand" size={11} />{tr('Open up')}</button>
      <button type="button" className="ai-icon-button" onClick={onShrink} aria-label={tr('Shrink {value0}', { value0: name })} title={tr('Shrink {value0}', { value0: name })}><ToolbarIcon name="collapse" size={13} /></button>
    </div>
  </article>
}

export function GroupFrame({ item, onFold }: { item: Extract<PlacedItem, { kind: 'block' }>; onFold: () => void }) {
  const tr = useI18n()
  const name = groupName(item.id)
  return <section className="ai-group-frame" style={box(item)} aria-label={name}>
    <button type="button" className="ai-group-head" onClick={onFold} aria-label={tr('Fold {value0} back into one item', { value0: name })} title={item.id}>
      <ToolbarIcon name="layers" size={13} /><span>{name}</span><span className="ai-group-action">{tr('Fold up')}</span>
    </button>
  </section>
}
