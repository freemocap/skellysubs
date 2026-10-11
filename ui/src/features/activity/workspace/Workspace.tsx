import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { useI18n } from '../../../components/localization/i18n'
import type { GraphOverlay } from '../../../domain/activity/graph-overlay'
import type { GraphView } from '../../../domain/activity/graph-projection'
import { humanizeId } from '../../../domain/activity/graph-types'
import { cardMetrics, layoutWorkspace, type NodeLevel, type Orientation, type PlacedEdge, type Point, type WorkspaceGeometry, type WorkspaceState } from './layout'
import { typeColor, useWorkspaceGeometry } from './geometry'
import { GroupBubble, GroupCard, GroupFrame, OperationBubble, OperationCard, type HoverTarget, type Interaction, type Preview, type ResizeEdge } from './NodeItems'
import { nodeName } from './wording'

export interface WorkspaceProps {
  view: GraphView
  /// Facts of the selected revision; null shows structure only.
  overlay: GraphOverlay | null
  /// Motion is reserved for the live revision.
  live: boolean
  preview: Preview | null
  state: WorkspaceState
  orientation: Orientation
  /// A step to bring into view, with a counter so the same step can be asked for again.
  focus: { id: string; serial: number } | null
  onLevel: (id: string, level: NodeLevel) => void
  onFold: (group: string, folded: boolean) => void
  onLocalSteps: (shown: boolean) => void
  onOffset: (id: string, offset: Point) => void
  /// A resize commits the new size and, for a top or start edge, the new offset.
  onBox: (id: string, offset: Point, size: Point) => void
  onOpen: (node: string) => void
  onHover: (target: HoverTarget | null) => void
}

const DRAG_THRESHOLD = 4
const MIN_SCALE = 0.55

/// Edges keep their static binding identity. Motion indicates consumer activity,
/// never per-port availability (optional outputs may be absent after success).
function edgeState(edge: PlacedEdge, overlay: GraphOverlay | null, live: boolean): 'structure' | 'flowing' {
  if (!overlay) return 'structure'
  return live && edge.consumers.some(node => overlay.nodes[node].tone === 'running') ? 'flowing' : 'structure'
}

type Gesture = { kind: 'drag' | 'resize'; id: string; edge: ResizeEdge | null; origin: Point; start: Point; baseOffset: Point; baseSize: Point; natural: Point; moved: boolean }

/// The offset and size a resize gesture produces from its pointer delta. A top
/// or start edge moves the item while sizing it; nothing shrinks below the
/// item's natural size.
function resized(gesture: Gesture, delta: Point): { offset: Point; size: Point } {
  const edge = gesture.edge ?? 'se'
  let width = gesture.baseSize[0], height = gesture.baseSize[1], dx = 0, dy = 0
  if (edge.includes('e')) width += delta[0]
  if (edge.includes('w')) { const w = Math.max(gesture.natural[0], gesture.baseSize[0] - delta[0]); dx = gesture.baseSize[0] - w; width = w }
  if (edge.includes('s')) height += delta[1]
  if (edge.includes('n')) { const h = Math.max(gesture.natural[1], gesture.baseSize[1] - delta[1]); dy = gesture.baseSize[1] - h; height = h }
  return { offset: [gesture.baseOffset[0] + dx, gesture.baseOffset[1] + dy], size: [Math.max(gesture.natural[0], width), Math.max(gesture.natural[1], height)] }
}

/// The AI workspace canvas. Geometry comes only from structure and the viewer's
/// presentation choices; the overlay changes colors, words and motion. The
/// canvas scales down to fit its container so the whole run is visible at rest.
export function Workspace({ view, overlay, live, preview, state, orientation, focus, onLevel, onFold, onLocalSteps, onOffset, onBox, onOpen, onHover }: WorkspaceProps) {
  const tr = useI18n()
  const geometry = useWorkspaceGeometry()
  const [gesture, setGesture] = useState<Gesture | null>(null)
  const gestureRef = useRef<Gesture | null>(null)
  const consumedClick = useRef(false)
  const scrollRef = useRef<HTMLDivElement>(null)
  const [available, setAvailable] = useState<Point | null>(null)

  /// Presentation state with the gesture in progress applied on top.
  const shown = useMemo<WorkspaceState>(() => {
    if (!gesture || !gesture.moved) return state
    const delta: Point = [gesture.start[0] - gesture.origin[0], gesture.start[1] - gesture.origin[1]]
    if (gesture.kind === 'drag') return { ...state, offsets: { ...state.offsets, [gesture.id]: [gesture.baseOffset[0] + delta[0], gesture.baseOffset[1] + delta[1]] } }
    const next = resized(gesture, delta)
    return { ...state, offsets: { ...state.offsets, [gesture.id]: next.offset }, sizes: { ...state.sizes, [gesture.id]: next.size } }
  }, [state, gesture])
  const layout = useMemo(() => layoutWorkspace(view, shown, geometry, orientation), [view, shown, geometry, orientation])

  useLayoutEffect(() => {
    const element = scrollRef.current
    if (!element) throw new Error('The AI workspace did not mount its scroll container.')
    const measure = () => setAvailable([element.clientWidth, element.clientHeight])
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])
  const scale = useMemo(() => {
    if (!available || !layout.width || !layout.height) return 1
    const padding = geometry.columnGap
    const fit = Math.min((available[0] - padding) / layout.width, (available[1] - padding) / layout.height)
    return Math.max(MIN_SCALE, Math.min(1, fit))
  }, [available, layout.width, layout.height, geometry.columnGap])

  useEffect(() => {
    if (!focus) return
    const element = scrollRef.current?.querySelector<HTMLElement>(`[data-id="${CSS.escape(focus.id)}"]`)
    if (!element) return
    element.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })
    element.focus({ preventScroll: true })
  }, [focus])

  const begin = useCallback((kind: Gesture['kind'], id: string, edge: ResizeEdge | null, event: ReactPointerEvent) => {
    if (event.button !== 0) return
    const point: Point = [event.clientX, event.clientY]
    const natural = naturalSize(view, geometry, id)
    const next: Gesture = { kind, id, edge, origin: point, start: point, baseOffset: state.offsets[id] ?? [0, 0], baseSize: state.sizes[id] ?? natural, natural, moved: false }
    gestureRef.current = next
    setGesture(next)
    consumedClick.current = false
    event.currentTarget.setPointerCapture(event.pointerId)
    event.stopPropagation()
  }, [state, view, geometry])

  const active = gesture !== null
  useEffect(() => {
    if (!active) return
    const move = (event: PointerEvent) => {
      const current = gestureRef.current
      if (!current) return
      const scaled: Point = [current.origin[0] + (event.clientX - current.origin[0]) / scale, current.origin[1] + (event.clientY - current.origin[1]) / scale]
      const moved = current.moved || Math.hypot(event.clientX - current.origin[0], event.clientY - current.origin[1]) > DRAG_THRESHOLD
      const next = { ...current, start: scaled, moved }
      gestureRef.current = next
      setGesture(next)
    }
    const end = () => {
      const current = gestureRef.current
      gestureRef.current = null
      setGesture(null)
      if (!current || !current.moved) return
      consumedClick.current = true
      const delta: Point = [current.start[0] - current.origin[0], current.start[1] - current.origin[1]]
      if (current.kind === 'drag') onOffset(current.id, [current.baseOffset[0] + delta[0], current.baseOffset[1] + delta[1]])
      else { const next = resized(current, delta); onBox(current.id, next.offset, next.size) }
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', end)
    window.addEventListener('pointercancel', end)
    return () => { window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', end); window.removeEventListener('pointercancel', end) }
  }, [active, scale, onOffset, onBox])

  const interaction: Interaction = useMemo(() => ({
    dragStart: (id, event) => begin('drag', id, null, event),
    resizeStart: (id, edge, event) => begin('resize', id, edge, event),
    clickAllowed: () => { const allowed = !consumedClick.current; consumedClick.current = false; return allowed },
  }), [begin])

  const [hovered, setHovered] = useState<HoverTarget | null>(null)
  const hover = (target: HoverTarget | null) => { setHovered(target); onHover(target) }
  const related = (edge: PlacedEdge) => hovered !== null && (edge.source === hovered.id || edge.target === hovered.id)
  const edgeTitle = (edge: PlacedEdge) => {
    const ends = `${nodeName(view, edge.producers[0])} → ${nodeName(view, edge.consumers[0])}`
    return edge.via.length ? tr('{value0}, through {value1}', { value0: ends, value1: edge.via.map(step => nodeName(view, step)).join(', ') }) : ends
  }
  const localCount = view.ids.filter(id => view.operations[id].resource === 'Local').length
  const localLabel = state.localSteps ? tr('Hide local steps') : tr('Show {value0} local steps', { value0: localCount })

  return <div className="ai-workspace" ref={scrollRef} data-orientation={orientation} data-live={live} data-focus={hovered ? 'true' : undefined} data-gesture={gesture?.kind} data-edge={gesture?.edge ?? undefined}>
    <div className="ai-workspace-tools">
      <button type="button" className="ai-chip" aria-pressed={state.localSteps} onClick={() => onLocalSteps(!state.localSteps)}
        title={tr('Local steps move data around inside the app. AI steps call a model.')}>
        <ToolbarIcon name="models" size={13} />{localLabel}
      </button>
      {layout.inlineGroups.length > 0 && <span className="ai-workspace-note">{tr('{value0} is drawn without a frame because its steps interleave with others.', { value0: layout.inlineGroups.map(group => humanizeId(group.slice(group.lastIndexOf('/') + 1))).join(', ') })}</span>}
    </div>
    <div className="ai-workspace-scaler" style={{ width: layout.width * scale, height: layout.height * scale }}>
      <div className="ai-workspace-layer" style={{ width: layout.width, height: layout.height, transform: `scale(${scale})` }}>
        <svg className="ai-workspace-edges" width={layout.width} height={layout.height} aria-hidden="true">
          {layout.edges.map(edge => <g key={edge.id} className="ai-edge-group" data-kind={edge.kind} data-state={edgeState(edge, overlay, live)} data-related={related(edge) || undefined}>
            <path d={edge.path} className="ai-edge-halo" />
            <path d={edge.path} className="ai-edge" style={edge.contract ? { stroke: typeColor(edge.contract) } : undefined}><title>{edgeTitle(edge)}</title></path>
            {edge.via.length > 0 && <circle className="ai-edge-via" cx={(edge.from[0] + edge.to[0]) / 2} cy={(edge.from[1] + edge.to[1]) / 2} r={3} />}
          </g>)}
        </svg>
        {layout.items.map(item => {
          const shared = { view, overlay, geometry, interaction, onHover: hover }
          if (item.kind === 'block') return <GroupFrame key={`block:${item.id}`} item={item} onFold={() => onFold(item.id, true)} />
          if (item.kind === 'group') return item.level === 'card'
            ? <GroupCard key={`group:${item.id}`} {...shared} item={item} onUnfold={() => onFold(item.id, false)} onShrink={() => onLevel(item.id, 'bubble')} />
            : <GroupBubble key={`group:${item.id}`} {...shared} item={item} onExpand={() => onLevel(item.id, 'card')} />
          return item.level === 'card'
            ? <OperationCard key={item.id} {...shared} item={item} preview={preview} onOpen={() => onOpen(item.id)} onShrink={() => onLevel(item.id, 'bubble')} />
            : <OperationBubble key={item.id} {...shared} item={item} onExpand={() => onLevel(item.id, 'card')} />
        })}
      </div>
    </div>
  </div>
}

/// The size an operation card has before any resize.
function naturalSize(view: GraphView, geometry: WorkspaceGeometry, id: string): Point {
  const operation = view.operations[id]
  if (!operation) throw new Error(`Cannot resize an item that is not an operation: ${id}`)
  return [geometry.cardWidth, cardMetrics(view, operation, geometry).height]
}
