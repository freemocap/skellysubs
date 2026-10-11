import { describe, expect, it } from 'vitest'
import { projectGraph } from '../../../domain/activity/graph-projection'
import { pipelineArtifact, pipelineSnapshot } from '../../../../tests/fixtures/graph-artifacts'
import { cardMetrics, layoutWorkspace, type PlacedItem, type WorkspaceGeometry, type WorkspaceState } from './layout'

const geometry: WorkspaceGeometry = {
  cardWidth: 272, bubbleWidth: 168, bubbleHeight: 34, border: 1, header: 52, row: 24, divider: 5, slot: 70, footer: 34,
  columnGap: 56, rowGap: 14, groupHeader: 36, groupPad: 12,
}
const view = projectGraph({ artifact_id: 'a', artifact: pipelineArtifact() })
const bubbles: WorkspaceState = { levels: {}, folded: {}, localSteps: true, offsets: {}, sizes: {} }
const find = (items: PlacedItem[], id: string) => {
  const item = items.find(candidate => candidate.id === id && candidate.kind !== 'block')
  if (!item) throw new Error(`missing ${id}`)
  return item
}
const shown = (items: PlacedItem[]) => items.flatMap(item => item.kind === 'operation' ? [item.id] : item.kind === 'group' ? item.members : [])

describe('layoutWorkspace', () => {
  it('defaults to bubbles with groups folded, counting every operation and binding once', () => {
    const layout = layoutWorkspace(view, bubbles, geometry, 'across')
    expect(layout.items.map(item => `${item.kind}:${item.id}`).sort()).toEqual(['group:pack', 'operation:note', 'operation:read', 'operation:write'])
    expect(shown(layout.items).sort()).toEqual([...view.ids].sort())
    expect(layout.edges.reduce((sum, edge) => sum + edge.bindings, 0) + layout.hiddenBindings).toBe(view.counts.bindings)
    expect(layout.hiddenBindings).toBe(2)
    expect(layout.inlineGroups).toEqual([])
  })

  it('is deterministic and independent of run facts (G2)', () => {
    const idle = layoutWorkspace(projectGraph(pipelineSnapshot()), bubbles, geometry, 'across')
    const busy = layoutWorkspace(projectGraph(pipelineSnapshot({ revision: '40', nodes: { write: 'Running', read: 'Adopted' } })), bubbles, geometry, 'across')
    expect(busy).toEqual(idle)
  })

  it('never moves another column, and moves only lower items in the same column, when a node expands', () => {
    const before = layoutWorkspace(view, bubbles, geometry, 'across')
    const after = layoutWorkspace(view, { ...bubbles, levels: { write: 'card' }, folded: {} }, geometry, 'across')
    const write0 = find(before.items, 'write'), write1 = find(after.items, 'write')
    for (const item of before.items) {
      const moved = find(after.items, item.id)
      const columnLeft = (placed: PlacedItem) => placed.x + placed.width / 2
      expect(columnLeft(moved)).toBe(columnLeft(item))
      const sameColumn = columnLeft(item) === columnLeft(write0)
      if (!sameColumn || item.y < write0.y || item.id === 'write') expect(moved.y).toBe(item.y)
      else expect(moved.y).toBe(item.y + write1.height - write0.height)
    }
  })

  it('anchors wired inputs, the guard and outputs on card rows', () => {
    const layout = layoutWorkspace(view, { ...bubbles, levels: { write: 'card', note: 'card', read: 'card' }, folded: {} }, geometry, 'across')
    const write = find(layout.items, 'write')
    const metrics = cardMetrics(view, view.operations.write, geometry)
    expect(metrics).toMatchObject({ wiredRows: ['text'], guardRow: false, fixed: ['extra', 'mode'], outputs: ['text'], slot: true })
    expect(write.height).toBe(metrics.height)
    const intoWrite = layout.edges.find(edge => edge.kind === 'data' && edge.target === 'write')!
    expect(intoWrite.to).toEqual([write.x, write.y + 1 + 52 + 12])
    const note = find(layout.items, 'note')
    const guard = layout.edges.find(edge => edge.kind === 'guard')!
    expect(guard.to).toEqual([note.x, note.y + 1 + 52 + 24 + 12])
    const read = find(layout.items, 'read')
    const readMetrics = cardMetrics(view, view.operations.read, geometry)
    expect(readMetrics.outputs).toEqual(['allowed', 'text'])
    expect(guard.from).toEqual([read.x + read.width, read.y + 1 + 52 + 5 + 12])
  })

  it('draws an unfolded group as a block around its members, keeping nested groups folded', () => {
    const layout = layoutWorkspace(view, { ...bubbles, levels: {}, folded: { pack: false } }, geometry, 'across')
    const block = layout.items.find(item => item.kind === 'block' && item.id === 'pack')!
    const first = find(layout.items, 'pack/first'), inner = find(layout.items, 'pack/inner')
    for (const member of [first, inner]) {
      expect(member.x).toBeGreaterThanOrEqual(block.x)
      expect(member.x + member.width).toBeLessThanOrEqual(block.x + block.width)
      expect(member.y).toBeGreaterThanOrEqual(block.y + geometry.groupHeader)
    }
    expect(inner.kind).toBe('group')
    expect(layout.edges.some(edge => edge.source === 'write' && edge.target === 'pack/first')).toBe(true)
    const all = layoutWorkspace(view, { ...bubbles, levels: {}, folded: { pack: false, 'pack/inner': false } }, geometry, 'across')
    expect(all.hiddenBindings).toBe(0)
    expect(shown(all.items).sort()).toEqual([...view.ids].sort())
  })

  it('shows a non-contiguous group inline and discloses it', () => {
    const artifact = pipelineArtifact()
    artifact.definition.nodes.note.inputs.text = { Output: { node: 'pack/first', port: 'text' } }
    artifact.definition.nodes['pack/second'].inputs.text = { Output: { node: 'note', port: 'text' } }
    const tangled = projectGraph({ artifact_id: 't', artifact })
    const layout = layoutWorkspace(tangled, bubbles, geometry, 'across')
    expect(layout.inlineGroups).toEqual(['pack'])
    expect(layout.items.filter(item => item.kind === 'operation').map(item => item.id).sort()).toEqual(['note', 'pack/first', 'pack/second', 'read', 'write'])
    expect(find(layout.items, 'pack/inner').kind).toBe('group')
  })

  it('stacks one column top-down for phones, with edges running from a bottom edge to a top edge', () => {
    const layout = layoutWorkspace(view, { ...bubbles, levels: { write: 'card' }, folded: {} }, geometry, 'down')
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
})

describe('hidden local steps', () => {
  const hidden: WorkspaceState = { ...bubbles, localSteps: false }
  it('draws only model steps and routes bindings through the hidden ones', () => {
    const layout = layoutWorkspace(view, hidden, geometry, 'across')
    expect(layout.hiddenLocal).toEqual(['pack/first', 'pack/inner/deep', 'pack/second', 'read'])
    expect(shown(layout.items).sort()).toEqual(['note', 'write'])
    expect(layout.items.some(item => item.kind === 'group')).toBe(false)
    expect(layout.edges).toEqual([])
  })
  it('names the hidden steps a routed connection passes through', () => {
    const artifact = pipelineArtifact()
    artifact.operations['op:read'].resource = 'Provider'
    const routed = layoutWorkspace(projectGraph({ artifact_id: 'b', artifact }), hidden, geometry, 'across')
    expect(shown(routed.items).sort()).toEqual(['note', 'read', 'write'])
    const direct = routed.edges.filter(edge => edge.source === 'read' && edge.target === 'write')
    expect(direct.map(edge => edge.via)).toEqual([[]])
  })
  it('leaves from the visible producer port when the hidden step renames the value', () => {
    const artifact = pipelineArtifact()
    artifact.operations['op:read'].resource = 'Provider'
    artifact.operations['op:relay'] = { contract: 'op:relay', implementation: 'fixture/relay/1', inputs: { text: { contract: 't:text', optional: false } }, outputs: { relayed: { contract: 't:text', optional: false } }, resource: 'Local', reuse: 'Exact' }
    artifact.definition.nodes.relay = { operation: 'op:relay', inputs: { text: { Output: { node: 'read', port: 'text' } } }, after: [], guard: null, activation: 'Automatic' }
    artifact.definition.nodes.write.inputs.text = { Output: { node: 'relay', port: 'relayed' } }
    const layout = layoutWorkspace(projectGraph({ artifact_id: 'c', artifact }), { ...hidden, levels: { read: 'card', write: 'card' } }, geometry, 'across')
    const edge = layout.edges.find(candidate => candidate.source === 'read' && candidate.target === 'write')!
    expect(edge.via).toEqual(['relay'])
    const read = find(layout.items, 'read')
    expect(edge.from).toEqual([read.x + read.width, read.y + 1 + 52 + 5 + 24 + 12])
  })
  it('lands connections on a group card member rows', () => {
    const layout = layoutWorkspace(view, { ...bubbles, levels: { pack: 'card' } }, geometry, 'across')
    const pack = find(layout.items, 'pack')
    const into = layout.edges.find(edge => edge.target === 'pack')!
    expect(into.to[1]).toBe(pack.y + geometry.border + geometry.header + geometry.row / 2)
  })
  it('keeps a dragged offset and grows the canvas to hold it', () => {
    const plain = layoutWorkspace(view, bubbles, geometry, 'across')
    const moved = layoutWorkspace(view, { ...bubbles, offsets: { write: [500, 300] } }, geometry, 'across')
    expect(find(moved.items, 'write').x).toBe(find(plain.items, 'write').x + 500)
    expect(find(moved.items, 'note').x).toBe(find(plain.items, 'note').x)
    expect(moved.width).toBeGreaterThan(plain.width)
  })
  it('never shrinks a resized card below its natural size', () => {
    const layout = layoutWorkspace(view, { ...bubbles, levels: { write: 'card' }, sizes: { write: [10, 10] } }, geometry, 'across')
    const write = find(layout.items, 'write')
    expect(write.width).toBe(geometry.cardWidth)
    expect(write.height).toBe(cardMetrics(view, view.operations.write, geometry).height)
  })
})
