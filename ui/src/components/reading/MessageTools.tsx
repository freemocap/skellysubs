import { MessageToolMenu } from './MessageToolMenu'
import { useToolOverflow } from './useToolOverflow'
import { useInspectorAnchor } from './useInspectorAnchor'
import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { ToolbarIcon, type ToolbarIconName } from '../controls/ToolbarIcon'
import { useI18n } from '../localization/i18n'
import { useUiDirection } from '../localization/useUiDirection'

/** A reading tool: an icon in the row, or an icon with its name in the ⋯ menu. */
export interface MessageTool {
  key: string
  /** The tool's name: the row button's accessible name and tooltip, and the
   * menu item's visible label. */
  label: string
  /** The row's icon, from the shared toolbar set. */
  icon: ToolbarIconName
  /** Accessible name when it differs from the label. */
  ariaLabel?: string
  /** A toggle's state; omitted for actions. */
  pressed?: boolean
  /** The tool's result is still being produced. */
  pending?: boolean
  disabled?: boolean
  opensDialog?: boolean
  onSelect: () => void
}

/** The message's audio: play or stop reading it aloud. */
export interface MessagePlay {
  graphTarget?: Record<string, string>
  playing: boolean
  preparing?: boolean
  disabled?: boolean
  onToggle: () => void
}

/** The audio inspector's toggle. Omit it when there is no inspectable source. */
export type MessageInspect = { kind: 'available'; open: boolean; disabled?: boolean; onToggle: () => void }

/** One row of tools, identical on every message and for both speakers: Play
 * and Inspect audio lead; then the reading tools as icons, each named by its
 * tooltip; then the message's actions as icons (Edit, Add to Practice), always
 * in view; and ⋯ last, holding owner menu actions and reading tools that do not
 * fit in the available width, each shown there with its name. */
export function MessageTools({ play, inspect, tools, actions, more, pending, menuActions }: {
  /** Owner actions that stay in the menu at every width. */
  menuActions?: ReactNode
  /** Admission feedback occupies the existing tools without changing their footprint. */
  pending?: ReactNode
  play: MessagePlay | null
  inspect: MessageInspect | null
  /** Reading tools shown in the row. */
  tools: MessageTool[]
  /** Icon actions always shown in the row. */
  actions: ReactNode
  /** Optional tools shown inline when they fit; only the remainder enters ⋯. */
  more: MessageTool[]
}) {
  const tr = useI18n()
  const waiting = pending !== undefined
  const uiDirection = useUiDirection()
  const [open, setOpen] = useState(false)
  const { root, measure, visible } = useToolOverflow(more.length, [...tools, ...more].map(tool => tool.key).join('|'), !!menuActions)
  const overflow = more.slice(visible)
  const anchorInspector = useInspectorAnchor(inspect?.open)
  const panelId = useId()
  const menuButton = useRef<HTMLButtonElement>(null)
  const closeMenu = useCallback(() => setOpen(false), [])
  const hasMenu = overflow.length > 0 || !!menuActions
  useEffect(() => { if (!hasMenu) setOpen(false) }, [hasMenu])
  const stop = (event: { stopPropagation: () => void }) => event.stopPropagation()
  const toolButton = (tool: MessageTool, inMenu: boolean) => <button key={tool.key} type="button"
    className={`${inMenu ? 'message-tools-item' : 'message-tools-icon'}${tool.pending ? ' is-hydrating' : ''}`}
    aria-label={inMenu ? tool.ariaLabel : tool.ariaLabel ?? tool.label} title={inMenu ? undefined : tool.label}
    aria-pressed={tool.pressed} aria-expanded={tool.pressed} aria-haspopup={tool.opensDialog ? 'dialog' : undefined}
    aria-hidden={waiting || undefined} disabled={waiting || tool.disabled} onKeyDown={stop} onClick={event => { stop(event); if (inMenu && tool.opensDialog) setOpen(false); tool.onSelect() }}>
    <ToolbarIcon name={tool.icon} />{inMenu && tool.label}</button>
  return <div className={`message-actions message-tools${waiting ? ' reply-activity' : ''}`} data-pending={waiting ? '' : undefined} dir={uiDirection} ref={root} onDoubleClick={stop}
    onKeyDown={event => { if (event.key === 'Escape' && open) { event.stopPropagation(); setOpen(false) } }}>
    <span className="message-tools-primary" aria-hidden={waiting || undefined} inert={waiting}>
    {play && <button {...play.graphTarget} type="button" className="message-tools-icon message-tools-play" disabled={play.disabled}
      aria-label={tr(play.preparing ? "Cancel speech preparation" : play.playing ? "Stop playback" : "Play")} title={tr(play.preparing ? "Cancel speech preparation" : play.playing ? "Stop playback" : "Play")}
      onClick={event => { stop(event); play.onToggle() }}>{play.preparing ? <><span className="activity-spinner" aria-hidden="true" /><span className="hydrating-announce" role="status">{tr("Preparing audio…")}</span></> : <ToolbarIcon name={play.playing ? 'stop' : 'play'} />}</button>}
    {inspect && <button type="button" className="message-tools-icon" aria-label={tr("Inspect recording")} title={tr("Inspect recording")} aria-pressed={inspect.open} disabled={inspect.disabled}
      onClick={event => { stop(event); anchorInspector(event.currentTarget); inspect.onToggle() }}><ToolbarIcon name="waveform" /></button>}
    {(play || inspect) && tools.length > 0 && <span className="message-tools-divider" aria-hidden="true" />}
    {tools.map(tool => toolButton(tool, false))}
    </span>
    {more.slice(0, visible).map(tool => toolButton(tool, false))}
    <span className="message-tools-actions" aria-hidden={waiting || undefined} inert={waiting}>
      <span className="message-tools-fixed">
      {actions}
      </span>
      {hasMenu && <button ref={menuButton} type="button" className="message-tools-icon message-tools-more" aria-label={tr("More actions")} title={tr("More actions")}
        aria-expanded={open} aria-controls={panelId} onClick={event => { stop(event); setOpen(!open) }}><ToolbarIcon name="more" /></button>}
    </span>
    {hasMenu && open && !waiting && <MessageToolMenu anchor={menuButton} id={panelId} direction={uiDirection} onClose={closeMenu}>
      {overflow.map(tool => toolButton(tool, true))}
      {menuActions}
    </MessageToolMenu>}
    <span className="message-tools-measure" ref={measure} aria-hidden="true" inert>
      {more.map(tool => <span key={tool.key} className="message-tools-icon" data-measured-tool><ToolbarIcon name={tool.icon} /></span>)}
      <span className="message-tools-icon message-tools-more"><ToolbarIcon name="more" /></span>
    </span>
    {waiting && <span className="message-tools-progress" role="status">{pending}</span>}
  </div>
}
