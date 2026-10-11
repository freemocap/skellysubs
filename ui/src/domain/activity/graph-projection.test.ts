import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import type { DefinitionSnapshot } from '../../generated/graph-contracts'
import { PIPELINE_NODES, pipelineArtifact, pipelineSnapshot } from '../../../tests/fixtures/graph-artifacts'
import { groupMembers, projectGraph } from './graph-projection'

const view = () => projectGraph({ artifact_id: 'artifact-pipeline', artifact: pipelineArtifact() })

describe('projectGraph', () => {
  it('projects every node exactly once, in a stable order', () => {
    expect(view().ids).toEqual([...PIPELINE_NODES])
    expect(view()).toEqual(view())
  })

  it('draws only operation-to-operation bindings as connections', () => {
    const { edges, counts } = view()
    const data = edges.filter(edge => edge.kind === 'data').map(edge => edge.kind === 'data' && `${edge.from.node}.${edge.from.port}->${edge.to.node}.${edge.to.port}`)
    expect(data.sort()).toEqual([
      'pack/first.text->pack/second.text', 'pack/second.text->pack/inner/deep.text',
      'read.text->note.text', 'read.text->write.text', 'write.text->pack/first.text',
    ])
    expect(edges.filter(edge => edge.kind === 'guard')).toEqual([{ kind: 'guard', id: JSON.stringify(['guard', 'read', 'allowed', 'note']), from: { node: 'read', port: 'allowed' }, to: 'note', contract: 't:flag' }])
    expect(edges.filter(edge => edge.kind === 'control')).toEqual([{ kind: 'control', id: JSON.stringify(['control', 'read', 'note']), from: 'read', to: 'note' }])
    expect(counts).toEqual({ operations: 6, bindings: 7, captured: 3, groups: 2 })
    expect(new Set(edges.map(edge => edge.id)).size).toBe(edges.length)
  })

  it('keeps captured, constant and absent inputs inside their consumer', () => {
    const { operations } = view()
    expect(operations.read.inputs).toEqual([{ port: 'seed', kind: 'captured', contract: 't:text', input: 'seed' }])
    expect(operations.write.inputs).toEqual([
      { port: 'extra', kind: 'absent', contract: 't:text' },
      { port: 'mode', kind: 'constant', contract: 't:text' },
      { port: 'text', kind: 'wired', contract: 't:text', producer: { node: 'read', port: 'text' } },
    ])
    expect(JSON.stringify(operations.write)).not.toContain('omitted')
  })

  it('keeps a guard separate from a data input named guard', () => {
    const artifact = pipelineArtifact()
    artifact.operations['op:note'].inputs.guard = { contract: 't:flag', optional: false }
    artifact.definition.nodes.note.inputs.guard = { Output: { node: 'read', port: 'allowed' } }
    const projected = projectGraph({ artifact_id: 'a', artifact })
    expect(projected.edges.filter(edge => edge.kind === 'data' && edge.to.node === 'note').map(edge => edge.kind === 'data' && edge.to.port).sort()).toEqual(['guard', 'text'])
    expect(projected.edges.filter(edge => edge.kind === 'guard')).toHaveLength(1)
  })

  it('derives composition membership from exported boundaries, including nesting', () => {
    const { groups, operations } = view()
    expect(groups.pack).toEqual({ id: 'pack', parent: null, artifact: 'a:pack', members: ['pack/first', 'pack/second'], children: ['pack/inner'] })
    expect(groups['pack/inner']).toEqual({ id: 'pack/inner', parent: 'pack', artifact: 'a:inner', members: ['pack/inner/deep'], children: [] })
    expect(operations['pack/inner/deep']).toMatchObject({ group: 'pack/inner', name: 'deep' })
    expect(operations.read).toMatchObject({ group: null, name: 'read' })
    expect(groupMembers(view(), 'pack')).toEqual(['pack/first', 'pack/inner/deep', 'pack/second'])
  })

  it('is independent of any run overlay (G2)', () => {
    const idle = projectGraph(pipelineSnapshot())
    const busy = projectGraph(pipelineSnapshot({ revision: '9', nodes: { read: 'Adopted', write: 'Running', note: 'Disabled' } }))
    expect(busy).toEqual(idle)
  })

  it('preserves topology for disabled and on-demand nodes', () => {
    const artifact = pipelineArtifact()
    artifact.definition.nodes.write.activation = 'Disabled'
    expect(projectGraph({ artifact_id: 'a', artifact }).edges).toEqual(projectGraph({ artifact_id: 'a', artifact: pipelineArtifact() }).edges)
  })

  describe('fails loudly on inconsistent artifacts', () => {
    const mutate = (change: (artifact: ReturnType<typeof pipelineArtifact>) => void) => () => {
      const artifact = pipelineArtifact()
      change(artifact)
      projectGraph({ artifact_id: 'a', artifact })
    }
    it.each([
      ['a dangling producer node', mutate(a => { a.definition.nodes.write.inputs.text = { Output: { node: 'ghost', port: 'text' } } })],
      ['a dangling producer port', mutate(a => { a.definition.nodes.write.inputs.text = { Output: { node: 'read', port: 'ghost' } } })],
      ['an undeclared graph input', mutate(a => { a.definition.nodes.read.inputs.seed = { Input: 'ghost' } })],
      ['an unbound operation input', mutate(a => { delete a.definition.nodes.write.inputs.extra })],
      ['an unexpected bound input', mutate(a => { a.definition.nodes.read.inputs.ghost = { Input: 'seed' } })],
      ['an unregistered operation', mutate(a => { a.definition.nodes.read.operation = 'op:ghost' })],
      ['an unknown control parent', mutate(a => { a.definition.nodes.note.after = ['ghost'] })],
      ['a result without an output port', mutate(a => { a.definition.results.ghost = { Output: { node: 'read', port: 'text' } } })],
      ['an output without a result', mutate(a => { delete a.definition.results.written })],
      ['a composition member missing from the artifact', mutate(a => { delete a.definition.nodes['pack/first'] })],
    ])('%s', (_, run) => expect(run).toThrow())
  })

  it('projects the generated coach graph with every node and binding', () => {
    const snapshot = JSON.parse(readFileSync(new URL('../../generated/coach-graph.json', import.meta.url), 'utf8')) as DefinitionSnapshot
    const projected = projectGraph(snapshot)
    const definition = snapshot.artifact.definition
    expect(projected.ids).toEqual(Object.keys(definition.nodes).sort())
    const outputBindings = Object.values(definition.nodes).flatMap(n => [...Object.values(n.inputs), ...(n.guard ? [n.guard] : [])]).filter(source => 'Output' in source).length
    const controls = Object.values(definition.nodes).reduce((sum, n) => sum + n.after.length, 0)
    expect(projected.counts.bindings).toBe(outputBindings + controls)
    expect(projected.results.map(result => result.port)).toEqual(Object.keys(definition.results).sort())
  })
})
