import { describe, expect, it } from 'vitest'
import { projectGraph } from '../../../domain/activity/graph-projection'
import { pipelineArtifact, pipelineSnapshot } from '../../../../tests/fixtures/graph-artifacts'
import { cardMetrics, layoutWorkspace, type PlacedItem, type Station, type WorkspaceGeometry, type WorkspaceState } from './layout'

const geometry: WorkspaceGeometry = { cardWidth: 200, bubbleHeight: 30, border: 1, header: 30, row: 22, divider: 6, slot: 84, footer: 30, columnGap: 48, rowGap: 14, lane: 6 }
const view = projectGraph({ artifact_id: 'a', artifact: pipelineArtifact() })
const cards: WorkspaceState = { levels: {}, offsets: {}, sizes: {} }
const find = (items: PlacedItem[], id: string) => {
  const item = items.find(candidate => candidate.id === id)
  if (!item) throw new Error(`missing ${id}`)
  return item
}
const overlaps = (a: PlacedItem, b: PlacedItem) => a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
const column = (item: PlacedItem) => item.x + item.width / 2

describe('layoutWorkspace', () => {
  it('centers columns on a shared horizontal band', () => {
    const layout = layoutWorkspace(view, cards, geometry, 'across', [])
    const columns = new Map<number, PlacedItem[]>()
    for (const item of layout.items) columns.set(item.x, [...(columns.get(item.x) ?? []), item])
    const centers = [...columns.values()].map(items => (Math.min(...items.map(item => item.y)) + Math.max(...items.map(item => item.y + item.height))) / 2)
    for (const center of centers) expect(center).toBeCloseTo(centers[0])
  })
  it('draws every operation as an open card and every binding as one wire', () => {
    const layout = layoutWorkspace(view, cards, geometry, 'across', [])
    expect(layout.items.map(item => `${item.kind}:${item.id}`).sort()).toEqual(view.ids.map(id => `operation:${id}`).sort())
    expect(layout.items.every(item => item.kind === 'operation' && item.level === 'card')).toBe(true)
    expect(layout.edges.length).toBe(view.counts.bindings)
    expect(layout.frames.map(frame => frame.id).sort()).toEqual(['pack', 'pack/inner'])
  })

  it('is deterministic and independent of run facts', () => {
    const idle = layoutWorkspace(projectGraph(pipelineSnapshot()), cards, geometry, 'across', [])
    const busy = layoutWorkspace(projectGraph(pipelineSnapshot({ revision: '40', nodes: { write: 'Running', read: 'Adopted' } })), cards, geometry, 'across', [])
    expect(busy).toEqual(idle)
  })

  it('places each operation one column after its producers and never overlaps items', () => {
    const layout = layoutWorkspace(view, cards, geometry, 'across', [])
    for (const edge of layout.edges) expect(column(find(layout.items, edge.target))).toBeGreaterThan(column(find(layout.items, edge.source)))
    for (const a of layout.items) for (const b of layout.items) if (a !== b) expect(overlaps(a, b)).toBe(false)
  })

  it('anchors wired inputs, the guard and outputs on card rows, and lists inputs read in place as marker rows', () => {
    const layout = layoutWorkspace(view, cards, geometry, 'across', [])
    const write = find(layout.items, 'write')
    const metrics = cardMetrics(view.operations.write, geometry)
    expect(metrics).toMatchObject({ wiredRows: ['text'], guardRow: false, fixed: ['extra'], outputs: ['text'], slot: true })
    expect(metrics.markRows.map(input => input.port)).toEqual(['mode'])
    expect(write.height).toBe(metrics.height)
    const intoWrite = layout.edges.find(edge => edge.kind === 'data' && edge.target === 'write')!
    expect(intoWrite.to).toEqual([write.x, write.y + 1 + 30 + 11])
    const note = find(layout.items, 'note')
    const guard = layout.edges.find(edge => edge.kind === 'guard')!
    expect(guard.to).toEqual([note.x, note.y + 1 + 30 + 22 + 11])
    const read = find(layout.items, 'read')
    const readMetrics = cardMetrics(view.operations.read, geometry)
    expect(readMetrics.outputs).toEqual(['allowed', 'text'])
    expect(readMetrics.markRows.map(input => input.port)).toEqual(['seed'])
    expect(guard.from).toEqual([read.x + read.width, read.y + 1 + 30 + 22 + 6 + 11])
  })

  it('counts AI access inputs behind the footer instead of drawing them', () => {
    const artifact = pipelineArtifact()
    artifact.types['t:cred'] = 'Text'
    artifact.definition.inputs.credential = { contract: 't:cred', optional: true }
    artifact.operations['op:write'].inputs.credential = { contract: '1:ai.credential-reference', optional: true }
    artifact.definition.nodes.write.inputs.credential = { Input: 'credential' }
    artifact.types['1:ai.credential-reference'] = 'Text'
    const guarded = projectGraph({ artifact_id: 'g', artifact })
    const metrics = cardMetrics(guarded.operations.write, geometry)
    expect(metrics.fixed).toEqual(['credential', 'extra'])
    expect(metrics.markRows.map(input => input.port)).toEqual(['mode'])
  })

  it('routes a wire that skips a column through a gap, never behind a node', () => {
    const layout = layoutWorkspace(view, cards, geometry, 'across', [])
    const skip = layout.edges.find(edge => edge.source === 'read' && edge.target === 'write')!
    const note = find(layout.items, 'note')
    expect(column(note)).toBeGreaterThan(column(find(layout.items, 'read')))
    expect(column(note)).toBeLessThan(column(find(layout.items, 'write')))
    const points = [...skip.path.matchAll(/[LMQ]\s*(-?[\d.]+) (-?[\d.]+)/g)].map(match => [Number(match[1]), Number(match[2])])
    for (let index = 1; index < points.length; index++) {
      const [x0, y0] = points[index - 1], [x1, y1] = points[index]
      const crosses = Math.max(x0, x1) > note.x && Math.min(x0, x1) < note.x + note.width && Math.max(y0, y1) > note.y && Math.min(y0, y1) < note.y + note.height
      expect(crosses).toBe(false)
    }
  })

  it('wires a station to the operations that first read the graph input it supplies, and from the producer of the result it takes', () => {
    const stations: Station[] = [{ id: 'message', side: 'before', ports: ['seed'], rows: 2 }, { id: 'playback', side: 'after', ports: ['packed'], rows: 1 }]
    const layout = layoutWorkspace(view, cards, geometry, 'across', stations)
    const message = find(layout.items, 'message'), playback = find(layout.items, 'playback')
    expect(message.kind).toBe('station')
    expect(column(message)).toBeLessThan(Math.min(...layout.items.filter(item => item.kind === 'operation').map(column)))
    expect(column(playback)).toBeGreaterThan(Math.max(...layout.items.filter(item => item.kind === 'operation').map(column)))
    const hands = layout.edges.filter(edge => edge.kind === 'hand')
    expect(hands.map(edge => [edge.source, edge.target])).toEqual([['message', 'read'], ['pack/inner/deep', 'playback']])
    expect(() => layoutWorkspace(view, cards, geometry, 'across', [{ id: 'x', side: 'before', ports: ['ghost'], rows: 1 }])).toThrow('No operation reads the graph input ghost')
    expect(() => layoutWorkspace(view, cards, geometry, 'across', [{ id: 'read', side: 'before', ports: ['seed'], rows: 1 }])).toThrow('shares its id')
  })

  it('splits a column that is far taller than the others', () => {
    const artifact = pipelineArtifact()
    for (const name of ['a', 'b', 'c', 'd', 'e', 'f']) artifact.definition.nodes[name] = { operation: 'op:note', inputs: { text: { Output: { node: 'read', port: 'text' } } }, after: [], guard: null, activation: 'Automatic' }
    const fan = projectGraph({ artifact_id: 'f', artifact })
    const layout = layoutWorkspace(fan, cards, geometry, 'across', [])
    const columnsOf = new Set(layout.items.filter(item => ['a', 'b', 'c', 'd', 'e', 'f'].includes(item.id)).map(column))
    expect(columnsOf.size).toBeGreaterThan(1)
    for (const a of layout.items) for (const b of layout.items) if (a !== b) expect(overlaps(a, b)).toBe(false)
  })

  it('stacks one column top-down for phones, with wires running from a bottom edge to a top edge', () => {
    const layout = layoutWorkspace(view, cards, geometry, 'down', [])
    const ys = layout.items.map(item => item.y)
    expect([...ys].sort((a, b) => a - b)).toEqual(ys)
    expect(layout.items.every(item => item.x + item.width <= geometry.cardWidth)).toBe(true)
    for (const edge of layout.edges) {
      const source = find(layout.items, edge.source), target = find(layout.items, edge.target)
      expect(edge.from).toEqual([source.x + source.width / 2, source.y + source.height])
      expect(edge.to).toEqual([target.x + target.width / 2, target.y])
      expect(edge.to[1]).toBeGreaterThan(edge.from[1])
    }
  })

  it('keeps a dragged offset and grows the canvas to hold it', () => {
    const plain = layoutWorkspace(view, cards, geometry, 'across', [])
    const moved = layoutWorkspace(view, { ...cards, offsets: { write: [500, 300] } }, geometry, 'across', [])
    expect(find(moved.items, 'write').x).toBe(find(plain.items, 'write').x + 500)
    expect(find(moved.items, 'write').moved).toBe(true)
    expect(find(moved.items, 'note').x).toBe(find(plain.items, 'note').x)
    expect(moved.height).toBeGreaterThan(plain.height)
  })

  it('never shrinks a resized card below its natural size', () => {
    const layout = layoutWorkspace(view, { ...cards, sizes: { write: [10, 10] } }, geometry, 'across', [])
    const write = find(layout.items, 'write')
    expect(write.width).toBe(geometry.cardWidth)
    expect(write.height).toBe(cardMetrics(view.operations.write, geometry).height)
  })
})
