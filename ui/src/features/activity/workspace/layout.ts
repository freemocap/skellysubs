import type { GraphEdge, GraphView, InputBinding, ProjectedOperation } from '../../../domain/activity/graph-projection'
import { isAccessContract } from '../../../domain/activity/graph-types'

/// Geometry of the AI workspace: a layered node graph with routed wires. It
/// reads the structural projection and the viewer's presentation choices only
/// (expansion levels, dragged offsets, resized cards), never run facts, so
/// runtime changes cannot move anything. Rules:
/// - Column = longest dependency path from a root. A root that nothing feeds sits
///   just before its first consumer, so its wire is short.
/// - A column far taller than every other column is split into side-by-side
///   columns, so the board grows sideways rather than down.
/// - Rows inside a column are ordered by composition group, then by the average
///   row of their producers.
/// - A wire between neighbouring columns is an S-curve across the gutter. A wire
///   that skips columns runs down the gutter, crosses each skipped column
///   through the gap between two nodes, and rises again; the gap widens to hold
///   its lanes, so nothing is drawn behind a node.
/// - Stations outside the artifact (what feeds its inputs, what takes its
///   results) are stacked in a column before and after the graph and joined by
///   hand-off wires.
/// - A dragged item keeps its offset from its computed place; the rest never move.

export type NodeLevel = 'bubble' | 'card'
export type Orientation = 'across' | 'down'

export interface WorkspaceState {
  /// Expansion per item; an item with no entry is an open card.
  levels: Readonly<Record<string, NodeLevel>>
  /// Dragged displacement from the computed place, per item.
  offsets: Readonly<Record<string, Point>>
  /// Resized card dimensions, per operation.
  sizes: Readonly<Record<string, Point>>
}

/// Something outside the artifact that supplies graph inputs or takes graph
/// results: the learner's message before the run, the playback after it. The
/// caller names it; the layout joins it to the artifact by the ports it names.
export interface Station {
  id: string
  side: 'before' | 'after'
  /// Graph input names supplied (before) or graph result names taken (after).
  ports: string[]
  /// Rows of detail the station shows; sets its height.
  rows: number
}

/// Resolved pixel geometry, taken from the stylesheet's tokens at runtime so the
/// drawn rows and the computed anchors share one source.
export interface WorkspaceGeometry {
  cardWidth: number
  bubbleHeight: number
  border: number
  header: number
  row: number
  divider: number
  slot: number
  footer: number
  columnGap: number
  rowGap: number
  lane: number
}

export type Point = readonly [number, number]

interface Placed { x: number; y: number; width: number; height: number; /** Displaced by the viewer from its computed place. */ moved: boolean }
export type PlacedItem =
  | ({ kind: 'operation'; id: string; level: NodeLevel } & Placed)
  | ({ kind: 'station'; id: string; side: Station['side'] } & Placed)

/// A composition group's frame: the box around its members.
export interface PlacedFrame { id: string; x: number; y: number; width: number; height: number }

export interface PlacedEdge {
  id: string
  /// `hand` joins a station to the artifact; the rest are artifact bindings.
  kind: GraphEdge['kind'] | 'hand'
  contract: string | null
  source: string
  target: string
  from: Point
  to: Point
  path: string
}

export interface WorkspaceLayout {
  width: number
  height: number
  items: PlacedItem[]
  frames: PlacedFrame[]
  edges: PlacedEdge[]
}

export interface CardMetrics {
  height: number
  /// Ports with a socket: inputs wired from another operation's output.
  wiredRows: string[]
  guardRow: boolean
  /// Inputs read in place, drawn as marker rows: captured graph inputs and constants.
  markRows: InputBinding[]
  /// AI access inputs, absent inputs and a fixed guard: listed behind a count.
  fixed: string[]
  outputs: string[]
  slot: boolean
}

const isMark = (input: InputBinding) => (input.kind === 'captured' && !isAccessContract(input.contract)) || input.kind === 'constant'

/// Rows of an operation card, in drawing order: wired inputs by port, the wired
/// guard, inputs read in place, the divider, outputs by port, the slot every
/// model call reserves for what it made, and the footer.
export function cardMetrics(operation: ProjectedOperation, geometry: WorkspaceGeometry): CardMetrics {
  const wiredRows = operation.inputs.filter(input => input.kind === 'wired').map(input => input.port)
  const guardRow = operation.guard?.kind === 'wired'
  const markRows = operation.inputs.filter(isMark)
  const fixed = [
    ...operation.inputs.filter(input => input.kind !== 'wired' && !isMark(input)).map(input => input.port),
    ...(operation.guard !== null && operation.guard.kind !== 'wired' ? ['guard'] : []),
  ]
  const outputs = operation.outputs.map(output => output.port)
  const slot = operation.resource === 'Provider'
  const height = geometry.border * 2 + geometry.header + geometry.row * (wiredRows.length + (guardRow ? 1 : 0) + markRows.length)
    + geometry.divider + geometry.row * outputs.length + (slot ? geometry.slot : 0) + geometry.footer
  return { height, wiredRows, guardRow, markRows, fixed, outputs, slot }
}

export function stationHeight(station: Station, geometry: WorkspaceGeometry): number {
  return geometry.border * 2 + geometry.header + geometry.row * station.rows + geometry.divider
}

const endpoints = (edge: GraphEdge): [string, string] => [edge.kind === 'control' ? edge.from : edge.from.node, edge.kind === 'data' ? edge.to.node : edge.to]

/// One wire to draw: an artifact binding, or a hand-off between a station and
/// the operation at the artifact's edge.
interface Wire {
  id: string
  kind: PlacedEdge['kind']
  contract: string | null
  from: string
  fromPort: string | null
  to: string
  toPort: string | null
  guard: boolean
}

/// The operations a graph input first reaches: those that read it in place and
/// stand in the earliest column, so a hand-off wire lands where the value enters.
function entryPoints(view: GraphView, input: string, column: Map<string, number>): string[] {
  const readers = view.ids.filter(id => view.operations[id].inputs.some(binding => binding.kind === 'captured' && binding.input === input))
  if (!readers.length) throw new Error(`No operation reads the graph input ${input}.`)
  const first = Math.min(...readers.map(id => column.get(id)!))
  return readers.filter(id => column.get(id) === first)
}

function stationWires(view: GraphView, stations: Station[], column: Map<string, number>): Wire[] {
  const wires: Wire[] = []
  for (const station of stations) {
    for (const port of station.ports) {
      if (station.side === 'before') {
        for (const node of entryPoints(view, port, column)) {
          wires.push({ id: JSON.stringify(['hand', station.id, port, node]), kind: 'hand', contract: view.operations[node].inputs.find(input => input.kind === 'captured' && input.input === port)!.contract, from: station.id, fromPort: null, to: node, toPort: null, guard: false })
        }
      } else {
        const result = view.results.find(entry => entry.port === port)
        if (!result) throw new Error(`The artifact declares no result named ${port}.`)
        if (result.value.kind !== 'wired') throw new Error(`Graph result ${port} is not produced by an operation.`)
        wires.push({ id: JSON.stringify(['hand', result.value.producer.node, port, station.id]), kind: 'hand', contract: result.contract, from: result.value.producer.node, fromPort: result.value.producer.port, to: station.id, toPort: null, guard: false })
      }
    }
  }
  return wires
}

function bindingWires(view: GraphView): Wire[] {
  return view.edges.map(edge => {
    const [from, to] = endpoints(edge)
    return {
      id: edge.id, kind: edge.kind, contract: edge.kind === 'control' ? null : edge.contract, from, to,
      fromPort: edge.kind === 'control' ? null : edge.from.port, toPort: edge.kind === 'data' ? edge.to.port : null, guard: edge.kind === 'guard',
    }
  })
}

/// Longest dependency path per operation over every binding kind; roots that
/// nothing feeds move to the column before their first consumer.
function columnsByDepth(view: GraphView): Map<string, number> {
  const preds = new Map<string, Set<string>>(view.ids.map(id => [id, new Set<string>()]))
  for (const edge of view.edges) { const [from, to] = endpoints(edge); preds.get(to)!.add(from) }
  const depth = new Map<string, number>()
  const depthOf = (id: string, visiting: Set<string>): number => {
    const known = depth.get(id)
    if (known !== undefined) return known
    if (visiting.has(id)) throw new Error(`AI graph layout found a dependency cycle at ${id}.`)
    visiting.add(id)
    const parents = [...preds.get(id)!]
    const value = parents.length ? 1 + Math.max(...parents.map(parent => depthOf(parent, visiting))) : 0
    visiting.delete(id)
    depth.set(id, value)
    return value
  }
  for (const id of view.ids) depthOf(id, new Set())
  for (const id of view.ids) {
    if (preds.get(id)!.size) continue
    const consumers = view.edges.filter(edge => endpoints(edge)[0] === id).map(edge => depth.get(endpoints(edge)[1])!)
    if (consumers.length) depth.set(id, Math.min(...consumers) - 1)
  }
  return depth
}

/// Split any column far taller than every other one into side-by-side columns.
function balanced(columns: string[][], heightOf: (id: string) => number, gap: number): string[][] {
  if (columns.length < 2) return columns
  const heights = columns.map(column => column.reduce((sum, id) => sum + heightOf(id) + gap, 0))
  const result: string[][] = []
  columns.forEach((column, index) => {
    const others = Math.max(...heights.filter((_, other) => other !== index))
    const parts = heights[index] > others * 1.3 ? Math.ceil(heights[index] / others) : 1
    if (parts === 1) { result.push(column); return }
    const per = Math.ceil(column.length / parts)
    for (let part = 0; part < parts; part++) result.push(column.slice(part * per, (part + 1) * per))
  })
  return result.filter(column => column.length > 0)
}

/// Orthogonal polyline with rounded corners.
function elbow(points: Point[], radius: number): string {
  let d = `M${points[0][0]} ${points[0][1]}`
  for (let index = 1; index < points.length - 1; index++) {
    const p0 = points[index - 1], p1 = points[index], p2 = points[index + 1]
    const v1 = [p1[0] - p0[0], p1[1] - p0[1]], v2 = [p2[0] - p1[0], p2[1] - p1[1]]
    const l1 = Math.hypot(v1[0], v1[1]), l2 = Math.hypot(v2[0], v2[1])
    if (l1 === 0 || l2 === 0) continue
    const r = Math.min(radius, l1 / 2, l2 / 2)
    const a = [p1[0] - v1[0] / l1 * r, p1[1] - v1[1] / l1 * r], b = [p1[0] + v2[0] / l2 * r, p1[1] + v2[1] / l2 * r]
    d += ` L${a[0]} ${a[1]} Q${p1[0]} ${p1[1]} ${b[0]} ${b[1]}`
  }
  const last = points[points.length - 1]
  return `${d} L${last[0]} ${last[1]}`
}

function sCurve(from: Point, to: Point, orientation: Orientation): string {
  if (orientation === 'down') {
    const dy = Math.max(24, Math.abs(to[1] - from[1]) / 2)
    return `M${from[0]} ${from[1]} C${from[0]} ${from[1] + dy} ${to[0]} ${to[1] - dy} ${to[0]} ${to[1]}`
  }
  const dx = Math.max(30, Math.abs(to[0] - from[0]) / 2)
  return `M${from[0]} ${from[1]} C${from[0] + dx} ${from[1]} ${to[0] - dx} ${to[1]} ${to[0]} ${to[1]}`
}

/// Lanes claimed in one gutter or one channel, keyed so a fan-out from one port
/// shares a trunk.
class Lanes {
  private readonly keys: string[] = []
  claim(key: string): number {
    const index = this.keys.indexOf(key)
    if (index >= 0) return index
    this.keys.push(key)
    return this.keys.length - 1
  }
  get count(): number { return this.keys.length }
}

export function layoutWorkspace(view: GraphView, state: WorkspaceState, geometry: WorkspaceGeometry, orientation: Orientation, stations: Station[]): WorkspaceLayout {
  const level = (id: string): NodeLevel => state.levels[id] ?? 'card'
  const stationById = new Map(stations.map(station => [station.id, station]))
  for (const station of stations) if (view.operations[station.id]) throw new Error(`A station shares its id with an operation: ${station.id}`)

  const sizeOf = (id: string): { width: number; height: number } => {
    const station = stationById.get(id)
    if (station) return { width: geometry.cardWidth, height: stationHeight(station, geometry) }
    if (level(id) === 'bubble') return { width: geometry.cardWidth, height: geometry.bubbleHeight }
    const natural = { width: geometry.cardWidth, height: cardMetrics(view.operations[id], geometry).height }
    const resized = state.sizes[id]
    return resized ? { width: Math.max(natural.width, resized[0]), height: Math.max(natural.height, resized[1]) } : natural
  }

  // Columns: operations by depth, balanced; stations stacked before and after.
  const depth = columnsByDepth(view)
  const depthCount = view.ids.length ? Math.max(...depth.values()) + 1 : 0
  const byDepth: string[][] = Array.from({ length: depthCount }, () => [])
  for (const id of view.ids) byDepth[depth.get(id)!].push(id)
  const before = stations.filter(station => station.side === 'before').map(station => station.id)
  const after = stations.filter(station => station.side === 'after').map(station => station.id)
  const columns = [...(before.length ? [before] : []), ...balanced(byDepth, id => sizeOf(id).height, geometry.rowGap), ...(after.length ? [after] : [])]
  const column = new Map<string, number>()
  columns.forEach((ids, index) => ids.forEach(id => column.set(id, index)))

  const wires = [...bindingWires(view), ...stationWires(view, stations, column)]
  const preds = new Map<string, string[]>()
  for (const wire of wires) { if (!preds.has(wire.to)) preds.set(wire.to, []); preds.get(wire.to)!.push(wire.from) }

  // Rows: composition members together, then by the average row of producers.
  const rank = new Map<string, number>()
  const groupKey = (id: string) => view.operations[id]?.group ?? ''
  columns.forEach(ids => {
    const center = (id: string) => {
      const ranks = (preds.get(id) ?? []).filter(parent => rank.has(parent)).map(parent => rank.get(parent)!)
      return ranks.length ? ranks.reduce((sum, value) => sum + value, 0) / ranks.length : Number.POSITIVE_INFINITY
    }
    ids.sort((a, b) => {
      const ga = groupKey(a), gb = groupKey(b)
      if (ga !== gb) return ga === '' ? 1 : gb === '' ? -1 : ga < gb ? -1 : 1
      return center(a) - center(b) || (a < b ? -1 : a > b ? 1 : 0)
    })
    ids.forEach((id, position) => rank.set(id, position))
  })

  // Channels: a wire that skips columns crosses each one through the gap nearest
  // its height; gaps and gutters widen to hold the lanes they carry.
  const channels = new Map<string, Lanes>()
  const gutters = new Map<number, Lanes>()
  const laneKey = (wire: Wire) => `${wire.from}:${wire.fromPort ?? wire.kind}`
  const gutterOf = (index: number) => { if (!gutters.has(index)) gutters.set(index, new Lanes()); return gutters.get(index)! }
  const channelOf = (index: number, gap: number) => { const key = `${index}:${gap}`; if (!channels.has(key)) channels.set(key, new Lanes()); return channels.get(key)! }

  const placed = new Map<string, { x: number; y: number; width: number; height: number }>()
  const columnX: number[] = []
  const place = () => {
    if (orientation === 'down') {
      let y = 0
      columns.forEach((ids, index) => {
        columnX[index] = 0
        for (const id of ids) { const size = sizeOf(id); placed.set(id, { x: 0, y, ...size }); y += size.height + geometry.rowGap }
      })
      return
    }
    let x = 0
    columns.forEach((ids, index) => {
      columnX[index] = x
      let y = 0
      ids.forEach((id, position) => {
        if (position > 0) {
          const lanes = channels.get(`${index}:${position}`)
          if (lanes && lanes.count) y += lanes.count * geometry.lane + geometry.lane * 2
        }
        const size = sizeOf(id)
        placed.set(id, { x, y, ...size })
        y += size.height + geometry.rowGap
      })
      const laneSpan = (gutters.get(index)?.count ?? 0) * geometry.lane
      x += geometry.cardWidth + Math.max(geometry.columnGap, laneSpan + geometry.lane * 4)
    })
    const heights = columns.map(ids => Math.max(...ids.map(id => { const box = placed.get(id)!; return box.y + box.height })))
    const height = Math.max(0, ...heights)
    columns.forEach((ids, index) => {
      for (const id of ids) placed.get(id)!.y += (height - heights[index]) / 2
    })
  }
  place()

  const metricsOf = (id: string) => cardMetrics(view.operations[id], geometry)
  const headerY = (id: string) => placed.get(id)!.y + geometry.border + geometry.header / 2
  const outPoint = (wire: Wire): Point => {
    const box = placed.get(wire.from)!
    if (orientation === 'down') return [box.x + box.width / 2, box.y + box.height]
    const x = box.x + box.width
    if (stationById.has(wire.from) || level(wire.from) === 'bubble' || wire.fromPort === null) return [x, headerY(wire.from)]
    const metrics = metricsOf(wire.from)
    const index = metrics.outputs.indexOf(wire.fromPort)
    if (index < 0) throw new Error(`No output row for ${wire.from}.${wire.fromPort}`)
    const top = box.y + geometry.border + geometry.header + geometry.row * (metrics.wiredRows.length + (metrics.guardRow ? 1 : 0) + metrics.markRows.length) + geometry.divider
    return [x, top + geometry.row * index + geometry.row / 2]
  }
  const inPoint = (wire: Wire): Point => {
    const box = placed.get(wire.to)!
    if (orientation === 'down') return [box.x + box.width / 2, box.y]
    const x = box.x
    if (stationById.has(wire.to) || level(wire.to) === 'bubble' || wire.kind === 'control' || wire.kind === 'hand') return [x, headerY(wire.to)]
    const metrics = metricsOf(wire.to)
    const top = box.y + geometry.border + geometry.header
    if (wire.guard) return [x, top + geometry.row * metrics.wiredRows.length + geometry.row / 2]
    const index = metrics.wiredRows.indexOf(wire.toPort!)
    if (index < 0) throw new Error(`No wired row for ${wire.to}.${wire.toPort}`)
    return [x, top + geometry.row * index + geometry.row / 2]
  }

  // Claim lanes from computed places, then place again with the widened gaps.
  const routes = new Map<string, Array<{ column: number; gap: number; lane: number }>>()
  if (orientation === 'across') {
    for (const wire of wires) {
      const from = column.get(wire.from)!, to = column.get(wire.to)!
      if (to <= from + 1 && wire.kind !== 'control') continue
      const y = (outPoint(wire)[1] + inPoint(wire)[1]) / 2
      const via: Array<{ column: number; gap: number; lane: number }> = []
      gutterOf(from).claim(laneKey(wire))
      for (let index = from + 1; index < to; index++) {
        const ids = columns[index]
        let gap = ids.length, best = Number.POSITIVE_INFINITY
        for (let position = 1; position < ids.length; position++) {
          const previous = placed.get(ids[position - 1])!, next = placed.get(ids[position])!
          const middle = (previous.y + previous.height + next.y) / 2
          if (Math.abs(middle - y) < best) { best = Math.abs(middle - y); gap = position }
        }
        via.push({ column: index, gap, lane: channelOf(index, gap).claim(laneKey(wire)) })
        gutterOf(index).claim(laneKey(wire))
      }
      routes.set(wire.id, via)
    }
    place()
  }

  const gutterX = (index: number, wire: Wire) => columnX[index] + geometry.cardWidth + geometry.lane * 2 + gutterOf(index).claim(laneKey(wire)) * geometry.lane
  const channelY = (index: number, gap: number, lane: number) => {
    const ids = columns[index]
    if (gap >= ids.length) return placed.get(ids[ids.length - 1])!.y + placed.get(ids[ids.length - 1])!.height + geometry.lane * 2 + lane * geometry.lane
    const previous = placed.get(ids[gap - 1])!
    return previous.y + previous.height + geometry.lane * 2 + lane * geometry.lane
  }

  const offsetBox = (id: string) => {
    const box = placed.get(id)!
    const offset = state.offsets[id] ?? [0, 0]
    return { ...box, x: box.x + offset[0], y: box.y + offset[1], moved: offset[0] !== 0 || offset[1] !== 0 }
  }
  for (const id of placed.keys()) placed.set(id, offsetBox(id))

  // Down: a wire that skips an item runs along the right edge of the stack.
  const stackOrder = new Map(columns.flat().map((id, index) => [id, index]))
  const sideLanes = new Lanes()
  const edges: PlacedEdge[] = wires.map(wire => {
    const from = outPoint(wire), to = inPoint(wire)
    const via = routes.get(wire.id)
    let path: string
    if (orientation === 'down') {
      const skips = stackOrder.get(wire.to)! > stackOrder.get(wire.from)! + 1
      if (!skips) path = sCurve(from, to, orientation)
      else {
        const x = geometry.cardWidth + geometry.lane * 2 + sideLanes.claim(laneKey(wire)) * geometry.lane
        path = elbow([from, [from[0], from[1] + geometry.lane * 2], [x, from[1] + geometry.lane * 2], [x, to[1] - geometry.lane * 2], [to[0], to[1] - geometry.lane * 2], to], geometry.lane * 2)
      }
    } else if (!via) path = sCurve(from, to, orientation)
    else {
      const points: Point[] = [from]
      let x = gutterX(column.get(wire.from)!, wire)
      points.push([x, from[1]])
      for (const hop of via) {
        const y = channelY(hop.column, hop.gap, hop.lane)
        points.push([x, y])
        x = gutterX(hop.column, wire)
        points.push([x, y])
      }
      points.push([x, to[1]], to)
      path = elbow(points, geometry.lane * 2)
    }
    return { id: wire.id, kind: wire.kind, contract: wire.contract, source: wire.from, target: wire.to, from, to, path }
  })

  const items: PlacedItem[] = [...placed.entries()].map(([id, box]) => {
    const station = stationById.get(id)
    const moved = (state.offsets[id] ?? [0, 0]).some(value => value !== 0)
    return station
      ? { kind: 'station', id, side: station.side, x: box.x, y: box.y, width: box.width, height: box.height, moved }
      : { kind: 'operation', id, level: level(id), x: box.x, y: box.y, width: box.width, height: box.height, moved }
  })
  const frames: PlacedFrame[] = Object.values(view.groups).map(group => {
    const members = view.ids.filter(id => view.operations[id].group === group.id || view.operations[id].group?.startsWith(`${group.id}/`))
    const boxes = members.map(id => placed.get(id)!)
    const pad = geometry.lane * 2
    const x = Math.min(...boxes.map(box => box.x)) - pad, y = Math.min(...boxes.map(box => box.y)) - geometry.header / 2
    return { id: group.id, x, y, width: Math.max(...boxes.map(box => box.x + box.width)) + pad - x, height: Math.max(...boxes.map(box => box.y + box.height)) + pad - y }
  })

  let width = orientation === 'down' ? geometry.cardWidth + geometry.lane * 2 + sideLanes.count * geometry.lane : 0, height = 0
  for (const item of items) { width = Math.max(width, item.x + item.width); height = Math.max(height, item.y + item.height) }
  for (const frame of frames) { width = Math.max(width, frame.x + frame.width); height = Math.max(height, frame.y + frame.height) }
  return { width, height, items, frames, edges }
}
