import { shapeOf, type GraphEdge, type GraphView, type ProjectedOperation } from '../../../domain/activity/graph-projection'

/// Geometry of the AI workspace. It reads the structural projection and the
/// viewer's presentation choices only (expansion levels, folding, hidden local
/// steps, dragged offsets and resized cards), never run facts, so runtime
/// changes cannot move anything (G2). Rules:
/// - Every column is at least one card wide, so expanding or shrinking an
///   operation never moves another column.
/// - Columns stack from the top, so expanding an operation moves only the items
///   below it in the same column.
/// - Order within a column is derived from structure once, by one barycenter sweep.
/// - A dragged item keeps its offset from its computed place; the rest never move.
/// - Local steps (resource Local) can be hidden: their bindings are drawn as one
///   connection between the visible producer and consumer, and the hidden steps
///   it passes through are named on the connection.

export type NodeLevel = 'bubble' | 'card'
export type Orientation = 'across' | 'down'

export interface WorkspaceState {
  levels: Readonly<Record<string, NodeLevel>>
  /// Composition groups default to folded.
  folded: Readonly<Record<string, boolean>>
  /// Whether local (non-model) steps are drawn as their own items.
  localSteps: boolean
  /// Dragged displacement from the computed place, per item.
  offsets: Readonly<Record<string, Point>>
  /// Resized card dimensions, per operation.
  sizes: Readonly<Record<string, Point>>
}

/// Resolved pixel geometry, taken from the stylesheet's tokens at runtime so the
/// drawn rows and the computed anchors share one source.
export interface WorkspaceGeometry {
  cardWidth: number
  bubbleWidth: number
  bubbleHeight: number
  border: number
  header: number
  row: number
  divider: number
  slot: number
  footer: number
  columnGap: number
  rowGap: number
  groupHeader: number
  groupPad: number
}

export type Point = readonly [number, number]

interface Placed { x: number; y: number; width: number; height: number; /** Displaced by the viewer from its computed place. */ moved: boolean }
export type PlacedItem =
  | ({ kind: 'operation'; id: string; level: NodeLevel } & Placed)
  | ({ kind: 'group'; id: string; level: NodeLevel; members: string[] } & Placed)
  | ({ kind: 'block'; id: string } & Placed)

export interface PlacedEdge {
  id: string
  kind: GraphEdge['kind']
  contract: string | null
  /// Visible endpoints: an operation, or the folded group that contains it.
  source: string
  target: string
  from: Point
  to: Point
  path: string
  /// How many artifact bindings this drawn connection stands for (more than one
  /// where a folded group or hidden local steps merge parallel bindings).
  bindings: number
  /// The artifact operations at each end of the bindings this connection draws.
  producers: string[]
  consumers: string[]
  /// Hidden local steps the connection passes through, in order from the producer.
  via: string[]
}

export interface WorkspaceLayout {
  width: number
  height: number
  items: PlacedItem[]
  edges: PlacedEdge[]
  /// Bindings inside a folded group, not drawn but counted.
  hiddenBindings: number
  /// Local steps not drawn as items because local steps are hidden.
  hiddenLocal: string[]
  /// Groups whose members can't be drawn as one contiguous block (folding them
  /// would create a cycle). They are always shown inline and must be disclosed.
  inlineGroups: string[]
}

export interface CardMetrics {
  height: number
  /// Ports with a socket: inputs wired from another operation's output.
  wiredRows: string[]
  guardRow: boolean
  /// Inputs fixed when the run started (captured, constant, absent) and a fixed
  /// guard: node properties with no socket, listed behind the footer button.
  fixed: string[]
  outputs: string[]
  slot: boolean
}

/// Rows of an operation card, in drawing order: wired inputs by port, the wired
/// guard, the divider, outputs by port, the text slot that a model call with a
/// plain-text output reserves, and the footer. Wired rows name the producers
/// that are drawn; a hidden local producer is still a wired row.
export function cardMetrics(view: GraphView, operation: ProjectedOperation, geometry: WorkspaceGeometry): CardMetrics {
  const wiredRows = operation.inputs.filter(input => input.kind === 'wired').map(input => input.port)
  const guardRow = operation.guard?.kind === 'wired'
  const fixed = [
    ...operation.inputs.filter(input => input.kind !== 'wired').map(input => input.port),
    ...(operation.guard !== null && operation.guard.kind !== 'wired' ? ['guard'] : []),
  ]
  const outputs = operation.outputs.map(output => output.port)
  const slot = operation.resource === 'Provider' && operation.outputs.some(output => shapeOf(view, output.contract) === 'Text')
  const height = geometry.border * 2 + geometry.header + geometry.row * (wiredRows.length + (guardRow ? 1 : 0))
    + geometry.divider + geometry.row * outputs.length + (slot ? geometry.slot : 0) + geometry.footer
  return { height, wiredRows, guardRow, fixed, outputs, slot }
}

function groupCardHeight(members: number, geometry: WorkspaceGeometry): number {
  return geometry.border * 2 + geometry.header + geometry.row * members + geometry.footer
}

const sortedIds = (values: Iterable<string>) => [...values].sort()

function ancestors(view: GraphView, group: string | null): string[] {
  const chain: string[] = []
  for (let current = group; current; current = view.groups[current]?.parent ?? null) {
    if (!view.groups[current]) throw new Error(`Unknown composition group: ${current}`)
    chain.push(current)
  }
  return chain
}

function allMembers(view: GraphView, group: string): string[] {
  const entry = view.groups[group]
  return [...entry.members, ...entry.children.flatMap(child => allMembers(view, child))]
}

const endpoints = (edge: GraphEdge): [string, string] => [edge.kind === 'control' ? edge.from : edge.from.node, edge.kind === 'data' ? edge.to.node : edge.to]

/// Whether contracting a group's members into one item keeps the dependency
/// relation acyclic.
function contractible(view: GraphView, group: string, edges: GraphEdge[]): boolean {
  const members = new Set(allMembers(view, group))
  const key = (node: string) => members.has(node) ? `#${group}` : node
  const next = new Map<string, Set<string>>()
  for (const edge of edges) {
    const [a, b] = endpoints(edge)
    const from = key(a), to = key(b)
    if (from === to) continue
    if (!next.has(from)) next.set(from, new Set())
    next.get(from)!.add(to)
  }
  const state = new Map<string, 1 | 2>()
  const visit = (node: string): boolean => {
    const seen = state.get(node)
    if (seen === 1) return false
    if (seen === 2) return true
    state.set(node, 1)
    for (const target of next.get(node) ?? []) if (!visit(target)) return false
    state.set(node, 2)
    return true
  }
  return [...next.keys()].every(visit)
}

/// A binding between two drawn operations, possibly passing through hidden
/// local steps. The kind and contract are those of the last hop.
interface RoutedEdge {
  edge: GraphEdge
  from: string
  /// The output port on `from` the connection leaves; null for a control edge.
  fromPort: string | null
  to: string
  via: string[]
}

/// Route every binding across hidden local steps. A hidden step's incoming
/// bindings are joined to each of its outgoing bindings; a hidden step with no
/// drawn producer contributes nothing to draw. Control and guard edges through
/// hidden steps are routed the same way.
function routeAcrossHidden(view: GraphView, hidden: Set<string>): RoutedEdge[] {
  const incoming = new Map<string, RoutedEdge[]>()
  for (const edge of view.edges) {
    const [from, to] = endpoints(edge)
    if (!incoming.has(to)) incoming.set(to, [])
    incoming.get(to)!.push({ edge, from, fromPort: edge.kind === 'control' ? null : edge.from.port, to, via: [] })
  }
  const resolved = new Map<string, RoutedEdge[]>()
  const sourcesOf = (node: string, trail: Set<string>): RoutedEdge[] => {
    const known = resolved.get(node)
    if (known) return known
    if (trail.has(node)) throw new Error(`AI graph layout found a dependency cycle at ${node}.`)
    trail.add(node)
    const routes: RoutedEdge[] = []
    for (const route of incoming.get(node) ?? []) {
      if (!hidden.has(route.from)) { routes.push(route); continue }
      for (const upstream of sourcesOf(route.from, trail)) {
        routes.push({ edge: route.edge, from: upstream.from, fromPort: upstream.fromPort, to: node, via: [...upstream.via, route.from] })
      }
    }
    trail.delete(node)
    resolved.set(node, routes)
    return routes
  }
  const routed: RoutedEdge[] = []
  for (const node of view.ids) if (!hidden.has(node)) routed.push(...sourcesOf(node, new Set()))
  return routed
}

interface Item {
  key: string
  kind: PlacedItem['kind']
  id: string
}

interface Placement {
  width: number
  height: number
  items: PlacedItem[]
}

export function layoutWorkspace(view: GraphView, state: WorkspaceState, geometry: WorkspaceGeometry, orientation: Orientation): WorkspaceLayout {
  const hiddenLocal = state.localSteps ? [] : view.ids.filter(id => view.operations[id].resource === 'Local')
  const hidden = new Set(hiddenLocal)
  const drawnIds = view.ids.filter(id => !hidden.has(id))
  const routes = routeAcrossHidden(view, hidden)
  const routedEdges: GraphEdge[] = routes.map(route => {
    if (route.edge.kind === 'control' || route.fromPort === null) return { kind: 'control', id: route.edge.id, from: route.from, to: route.to }
    return route.edge.kind === 'guard'
      ? { kind: 'guard', id: route.edge.id, from: { node: route.from, port: route.fromPort }, to: route.to, contract: route.edge.contract }
      : { kind: 'data', id: route.edge.id, from: { node: route.from, port: route.fromPort }, to: route.edge.to, contract: route.edge.contract }
  })
  const drawnGroups = Object.keys(view.groups).filter(group => allMembers(view, group).some(member => !hidden.has(member)))
  const inlineGroups = sortedIds(drawnGroups.filter(group => !contractible(view, group, routedEdges)))
  const inline = new Set(inlineGroups)
  const isFolded = (group: string) => !inline.has(group) && (state.folded[group] ?? true)
  const level = (id: string): NodeLevel => state.levels[id] ?? 'bubble'
  const visibleMembers = (group: string) => sortedIds(allMembers(view, group).filter(member => !hidden.has(member)))
  /// The item standing for an operation inside a given scope, or null when the
  /// operation is outside that scope.
  const ownerIn = (node: string, scope: string | null): Item | null => {
    const chain = ancestors(view, view.operations[node].group).filter(group => !inline.has(group))
    if (scope === null) {
      const top = chain[chain.length - 1]
      if (!top) return { key: `op:${node}`, kind: 'operation', id: node }
      return isFolded(top) ? { key: `group:${top}`, kind: 'group', id: top } : { key: `block:${top}`, kind: 'block', id: top }
    }
    const index = chain.indexOf(scope)
    if (index < 0) return null
    if (index === 0) return { key: `op:${node}`, kind: 'operation', id: node }
    const child = chain[index - 1]
    return isFolded(child) ? { key: `group:${child}`, kind: 'group', id: child } : { key: `block:${child}`, kind: 'block', id: child }
  }

  /// The visible endpoint for an operation: itself, or its outermost folded group.
  const visible = (node: string): Item => {
    const chain = ancestors(view, view.operations[node].group).filter(group => !inline.has(group))
    for (let index = chain.length - 1; index >= 0; index--) {
      if (isFolded(chain[index])) return { key: `group:${chain[index]}`, kind: 'group', id: chain[index] }
    }
    return { key: `op:${node}`, kind: 'operation', id: node }
  }

  const place = (scope: string | null): Placement => {
    const items = new Map<string, Item>()
    for (const node of drawnIds) {
      const owner = ownerIn(node, scope)
      if (owner) items.set(owner.key, owner)
    }
    const preds = new Map<string, Set<string>>([...items.keys()].map(key => [key, new Set<string>()]))
    for (const edge of routedEdges) {
      const [from, to] = endpoints(edge)
      const a = ownerIn(from, scope), b = ownerIn(to, scope)
      if (a && b && a.key !== b.key) preds.get(b.key)!.add(a.key)
    }
    const depth = new Map<string, number>()
    const depthOf = (key: string, visiting: Set<string>): number => {
      const known = depth.get(key)
      if (known !== undefined) return known
      if (visiting.has(key)) throw new Error(`AI graph layout found a dependency cycle at ${key}.`)
      visiting.add(key)
      const parents = [...preds.get(key)!]
      const value = parents.length ? 1 + Math.max(...parents.map(parent => depthOf(parent, visiting))) : 0
      visiting.delete(key)
      depth.set(key, value)
      return value
    }
    for (const key of sortedIds(items.keys())) depthOf(key, new Set())

    const columnCount = items.size ? Math.max(...depth.values()) + 1 : 0
    const columns: string[][] = Array.from({ length: columnCount }, () => [])
    for (const key of sortedIds(items.keys())) columns[depth.get(key)!].push(key)
    const rank = new Map<string, number>()
    columns.forEach((column, index) => {
      if (index > 0) {
        const center = (key: string) => {
          const ranks = [...preds.get(key)!].map(parent => rank.get(parent)!)
          return ranks.length ? ranks.reduce((sum, value) => sum + value, 0) / ranks.length : Number.POSITIVE_INFINITY
        }
        column.sort((a, b) => center(a) - center(b) || (a < b ? -1 : a > b ? 1 : 0))
      }
      column.forEach((key, position) => rank.set(key, position))
    })

    const sizes = new Map<string, { width: number; height: number; sub?: Placement }>()
    for (const item of items.values()) {
      if (item.kind === 'operation') {
        const card = level(item.id) === 'card'
        if (!card) { sizes.set(item.key, { width: geometry.bubbleWidth, height: geometry.bubbleHeight }); continue }
        const natural = { width: geometry.cardWidth, height: cardMetrics(view, view.operations[item.id], geometry).height }
        const resized = state.sizes[item.id]
        sizes.set(item.key, resized ? { width: Math.max(natural.width, resized[0]), height: Math.max(natural.height, resized[1]) } : natural)
      } else if (item.kind === 'group') {
        const card = level(item.id) === 'card'
        sizes.set(item.key, card
          ? { width: geometry.cardWidth, height: groupCardHeight(visibleMembers(item.id).length, geometry) }
          : { width: geometry.bubbleWidth, height: geometry.bubbleHeight })
      } else {
        const sub = place(item.id)
        sizes.set(item.key, { width: sub.width + geometry.groupPad * 2, height: geometry.groupHeader + sub.height + geometry.groupPad * 2, sub })
      }
    }

    const placed: PlacedItem[] = []
    const put = (key: string, x: number, y: number, width: number) => {
      const item = items.get(key)!
      const size = sizes.get(key)!
      const offset = state.offsets[item.id] ?? [0, 0]
      const moved = offset[0] !== 0 || offset[1] !== 0
      const left = (item.kind === 'block' ? x : x + (width - size.width) / 2) + offset[0]
      const top = y + offset[1]
      if (item.kind === 'operation') placed.push({ kind: 'operation', id: item.id, level: level(item.id), x: left, y: top, width: size.width, height: size.height, moved })
      else if (item.kind === 'group') placed.push({ kind: 'group', id: item.id, level: level(item.id), members: visibleMembers(item.id), x: left, y: top, width: size.width, height: size.height, moved })
      else {
        placed.push({ kind: 'block', id: item.id, x: left, y: top, width: size.width, height: size.height, moved })
        for (const inner of size.sub!.items) placed.push({ ...inner, x: inner.x + left + geometry.groupPad, y: inner.y + top + geometry.groupHeader + geometry.groupPad })
      }
      return size.height
    }

    if (orientation === 'down') {
      const order = columns.flat()
      const width = Math.max(geometry.cardWidth, ...order.map(key => sizes.get(key)!.width))
      let y = 0
      for (const key of order) y += put(key, 0, y, items.get(key)!.kind === 'block' ? width : geometry.cardWidth) + geometry.rowGap
      return { width, height: Math.max(0, y - geometry.rowGap), items: placed }
    }
    let x = 0
    let height = 0
    for (const column of columns) {
      const width = Math.max(geometry.cardWidth, ...column.map(key => sizes.get(key)!.width))
      let y = 0
      for (const key of column) y += put(key, x, y, width) + geometry.rowGap
      height = Math.max(height, y - geometry.rowGap)
      x += width + geometry.columnGap
    }
    return { width: Math.max(0, x - geometry.columnGap), height, items: placed }
  }

  const placement = place(null)
  const byKey = new Map<string, PlacedItem>()
  for (const item of placement.items) byKey.set(`${item.kind === 'operation' ? 'op' : item.kind}:${item.id}`, item)
  const box = (item: Item) => {
    const found = byKey.get(item.key)
    if (!found) throw new Error(`AI graph layout did not place ${item.key}.`)
    return found
  }

  interface PortSpec { input?: string; guard?: boolean; output?: string; member?: string }

  const anchor = (item: Item, side: 'in' | 'out', port: PortSpec): Point => {
    const placed = box(item)
    if (orientation === 'down') return [placed.x + placed.width / 2, side === 'in' ? placed.y : placed.y + placed.height]
    const x = side === 'in' ? placed.x : placed.x + placed.width
    if (placed.kind === 'block') return [x, placed.y + Math.min(placed.height, geometry.bubbleHeight) / 2]
    if (placed.kind === 'group') {
      if (placed.level === 'bubble' || port.member === undefined) return [x, placed.y + (placed.level === 'card' ? geometry.border + geometry.header / 2 : placed.height / 2)]
      const index = placed.members.indexOf(port.member)
      if (index < 0) throw new Error(`No member row for ${placed.id}: ${port.member}`)
      return [x, placed.y + geometry.border + geometry.header + geometry.row * index + geometry.row / 2]
    }
    if (placed.level === 'bubble') return [x, placed.y + placed.height / 2]
    const metrics = cardMetrics(view, view.operations[placed.id], geometry)
    const top = placed.y + geometry.border + geometry.header
    if (side === 'in' && port.guard) return [x, top + geometry.row * metrics.wiredRows.length + geometry.row / 2]
    if (side === 'in' && port.input !== undefined) {
      const index = metrics.wiredRows.indexOf(port.input)
      if (index < 0) throw new Error(`No wired row for ${placed.id}.${port.input}`)
      return [x, top + geometry.row * index + geometry.row / 2]
    }
    if (side === 'out' && port.output !== undefined) {
      const index = metrics.outputs.indexOf(port.output)
      if (index < 0) throw new Error(`No output row for ${placed.id}.${port.output}`)
      const outputsTop = top + geometry.row * (metrics.wiredRows.length + (metrics.guardRow ? 1 : 0)) + geometry.divider
      return [x, outputsTop + geometry.row * index + geometry.row / 2]
    }
    return [x, placed.y + geometry.border + geometry.header / 2]
  }

  const curve = (from: Point, to: Point): string => {
    if (orientation === 'down') {
      const dy = Math.max(24, Math.abs(to[1] - from[1]) / 2)
      return `M${from[0]} ${from[1]} C${from[0]} ${from[1] + dy} ${to[0]} ${to[1] - dy} ${to[0]} ${to[1]}`
    }
    const dx = Math.max(40, Math.abs(to[0] - from[0]) / 2)
    return `M${from[0]} ${from[1]} C${from[0] + dx} ${from[1]} ${to[0] - dx} ${to[1]} ${to[0]} ${to[1]}`
  }

  const drawn = new Map<string, PlacedEdge>()
  let hiddenBindings = 0
  for (const route of routes) {
    const { edge } = route
    const source = visible(route.from), target = visible(route.to)
    if (source.key === target.key) { hiddenBindings++; continue }
    const sourceRows = source.kind === 'group' && level(source.id) === 'card'
    const targetRows = target.kind === 'group' && level(target.id) === 'card'
    const fromPort: PortSpec = source.kind === 'group' ? (sourceRows ? { member: route.from } : {}) : route.fromPort === null ? {} : { output: route.fromPort }
    const toPort: PortSpec = target.kind === 'group' ? (targetRows ? { member: route.to } : {}) : edge.kind === 'data' ? { input: edge.to.port } : edge.kind === 'guard' ? { guard: true } : {}
    const contract = edge.kind === 'control' ? null : edge.contract
    const key = JSON.stringify([edge.kind, source.key, fromPort, target.key, toPort, contract])
    const existing = drawn.get(key)
    if (existing) {
      existing.bindings++
      if (!existing.producers.includes(route.from)) existing.producers.push(route.from)
      if (!existing.consumers.includes(route.to)) existing.consumers.push(route.to)
      for (const step of route.via) if (!existing.via.includes(step)) existing.via.push(step)
      continue
    }
    const from = anchor(source, 'out', fromPort), to = anchor(target, 'in', toPort)
    drawn.set(key, { id: key, kind: edge.kind, contract, source: source.id, target: target.id, from, to, path: curve(from, to), bindings: 1, producers: [route.from], consumers: [route.to], via: [...route.via] })
  }

  let width = placement.width, height = placement.height
  for (const item of placement.items) { width = Math.max(width, item.x + item.width); height = Math.max(height, item.y + item.height) }
  return { width, height, items: placement.items, edges: [...drawn.values()], hiddenBindings, hiddenLocal: sortedIds(hiddenLocal), inlineGroups }
}
