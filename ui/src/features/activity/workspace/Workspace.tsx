import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { useI18n } from '../../../components/localization/i18n'
import type { GraphOverlay } from '../../../domain/activity/graph-overlay'
import type { GraphView } from '../../../domain/activity/graph-projection'
import { cardMetrics, layoutWorkspace, type Orientation, type PlacedEdge, type Point, type Station, type WorkspaceState } from './layout'
import { typeColor, useWorkspaceGeometry } from './geometry'
import { GroupFrame, OperationBubble, OperationCard, StationCard, type HoverTarget, type Interaction, type Preview, type ResizeEdge, type StationView } from './NodeItems'
import { nodeName } from './wording'
import type { StepProduct } from '../../../domain/conversation/record-view'

export interface WorkspaceProps {
  view: GraphView
  /// Facts of the selected revision; null shows structure only.
  overlay: GraphOverlay | null
  /// Animate active states while following live execution or playing recorded revisions.
  live: boolean
  preview: Preview | null
  /// What each step made, as the conversation shows it.
  products: Readonly<Record<string, StepProduct[]>>
  /// What feeds the artifact and what takes its results, outside the run.
  stations: StationView[]
  state: WorkspaceState
  orientation: Orientation
  /// A step to bring into view, with a counter so the same step can be asked for again.
  focus: { id: string; serial: number } | null
  onOffset: (id: string, offset: Point) => void
  /// A resize commits the new size and, for a top or start edge, the new offset.
  onBox: (id: string, offset: Point, size: Point) => void
  onOpen: (node: string) => void
  onHover: (target: HoverTarget | null) => void
}

const DRAG_THRESHOLD = 4
const MIN_SCALE = 0.55

/// Wires keep their static binding identity. Motion indicates consumer activity,
/// never per-port availability (optional outputs may be absent after success).
function edgeState(edge: PlacedEdge, overlay: GraphOverlay | null, live: boolean): 'structure' | 'flowing' {
  if (!overlay || !live) return 'structure'
  return overlay.nodes[edge.target]?.tone === 'running' ? 'flowing' : 'structure'
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
export function Workspace({ view, overlay, live, preview, products, stations, state, orientation, focus, onOffset, onBox, onOpen, onHover }: WorkspaceProps) {
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
  const stationShapes = useMemo<Station[]>(() => stations.map(station => ({ id: station.id, side: station.side, ports: station.ports, rows: station.rows.length })), [stations])
  const layout = useMemo(() => layoutWorkspace(view, shown, geometry, orientation, stationShapes), [view, shown, geometry, orientation, stationShapes])

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
    const operation = view.operations[id]
    const natural: Point = operation ? [geometry.cardWidth, cardMetrics(operation, geometry).height] : [geometry.cardWidth, 0]
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
  const stationById = new Map(stations.map(station => [station.id, station]))
  const endName = (id: string) => stationById.get(id)?.name ?? nodeName(view, id)
  const edgeTitle = (edge: PlacedEdge) => {
    const ends = `${endName(edge.source)} → ${endName(edge.target)}`
    if (edge.kind === 'control') return tr('{value0}: runs after', { value0: ends })
    if (edge.kind === 'guard') return tr('{value0}: only if', { value0: ends })
    if (edge.kind === 'hand') return tr('{value0}: hand-off outside the graph', { value0: ends })
    return ends
  }

  return <div className="ai-workspace" ref={scrollRef} data-orientation={orientation} data-live={live} data-focus={hovered ? 'true' : undefined} data-gesture={gesture?.kind} data-edge={gesture?.edge ?? undefined}>
    <div className="ai-workspace-scaler" style={{ width: layout.width * scale, height: layout.height * scale }}>
      <div className="ai-workspace-layer" style={{ width: layout.width, height: layout.height, transform: `scale(${scale})` }}>
        {layout.frames.map(frame => <GroupFrame key={frame.id} frame={frame} />)}
        <svg className="ai-workspace-edges" width={layout.width} height={layout.height} aria-hidden="true">
          {layout.edges.map(edge => <g key={edge.id} className="ai-edge-group" data-kind={edge.kind} data-state={edgeState(edge, overlay, live)} data-related={related(edge) || undefined}>
            <path d={edge.path} className="ai-edge-halo" />
            <path d={edge.path} className="ai-edge" style={edge.contract ? { stroke: typeColor(edge.contract) } : undefined}><title>{edgeTitle(edge)}</title></path>
          </g>)}
        </svg>
        {layout.items.map(item => {
          if (item.kind === 'station') {
            const station = stationById.get(item.id)
            if (!station) throw new Error(`The layout placed a station the workspace does not know: ${item.id}`)
            return <StationCard key={item.id} item={item} station={station} interaction={interaction} onHover={hover} />
          }
          const shared = { view, overlay, geometry, interaction, onHover: hover }
          return item.level === 'card'
            ? <OperationCard key={item.id} {...shared} item={item} preview={preview} products={products[item.id] ?? []} onOpen={() => onOpen(item.id)} />
            : <OperationBubble key={item.id} {...shared} item={item} onOpen={() => onOpen(item.id)} />
        })}
      </div>
    </div>
  </div>
}
