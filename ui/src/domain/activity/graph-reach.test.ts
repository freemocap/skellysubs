import { describe, expect, it } from 'vitest'
import { pipelineArtifact } from '../../../tests/fixtures/graph-artifacts'
import { projectGraph } from './graph-projection'
import { downstreamResults } from './graph-reach'

const view = projectGraph({ artifact_id: 'a', artifact: pipelineArtifact() })

describe('downstreamResults', () => {
  it('follows data and guard bindings forward to graph results', () => {
    expect(downstreamResults(view, 'read')).toEqual(['noted', 'packed', 'written'])
    expect(downstreamResults(view, 'write')).toEqual(['packed', 'written'])
    expect(downstreamResults(view, 'pack/second')).toEqual(['packed'])
  })

  it('does not follow control prerequisites', () => {
    const artifact = pipelineArtifact()
    artifact.definition.nodes.note.inputs.text = { Input: 'seed' }
    artifact.definition.nodes.note.guard = null
    const controlOnly = projectGraph({ artifact_id: 'a', artifact })
    expect(controlOnly.edges.some(edge => edge.kind === 'control' && edge.from === 'read' && edge.to === 'note')).toBe(true)
    expect(downstreamResults(controlOnly, 'read')).toEqual(['packed', 'written'])
  })

  it('rejects unknown operations', () => {
    expect(() => downstreamResults(view, 'ghost')).toThrow()
  })
})
