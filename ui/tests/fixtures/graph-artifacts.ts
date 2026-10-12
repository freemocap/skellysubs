import type { Artifact, Attempt, ConstantDisclosure, Definition, Disposition, FaultDisclosure, InspectionSnapshot, Node, Reason } from '../../src/generated/graph-contracts'

/// Domain-independent graph fixtures: a chain, a fan-out, a guarded on-demand
/// branch with a control prerequisite, captured/constant/absent inputs and a
/// nested composition. Operation and node names are deliberately generic.

const port = (contract: string, optional = false) => ({ contract, optional })
const TEXT = 't:text'
const FLAG = 't:flag'
const omitted: ConstantDisclosure = { omitted: 'Content' }

const node = (operation: string, inputs: Node<ConstantDisclosure>['inputs'], extra: Partial<Node<ConstantDisclosure>> = {}): Node<ConstantDisclosure> =>
  ({ operation, inputs, after: [], guard: null, activation: 'Automatic', ...extra })
const out = (nodeId: string, portName: string) => ({ Output: { node: nodeId, port: portName } })

const innerSource: Definition<ConstantDisclosure> = {
  contract: 'c:inner', inputs: { text: port(TEXT) }, outputs: { text: port(TEXT) },
  nodes: { deep: node('op:pick', { text: { Input: 'text' } }) },
  results: { text: out('deep', 'text') }, compositions: {},
}
const packSource: Definition<ConstantDisclosure> = {
  contract: 'c:pack', inputs: { text: port(TEXT) }, outputs: { text: port(TEXT) },
  nodes: { first: node('op:pick', { text: { Input: 'text' } }), second: node('op:pick', { text: out('first', 'text') }) },
  results: { text: out('inner/deep', 'text') },
  compositions: { inner: { artifact: 'a:inner', source: innerSource, bindings: { text: out('second', 'text') } } },
}

export function pipelineArtifact(): Artifact<ConstantDisclosure> {
  return structuredClone({
    types: { [TEXT]: 'Text', [FLAG]: 'Boolean' },
    operations: {
      'op:read': { contract: 'op:read', implementation: 'fixture/read/1', inputs: { seed: port(TEXT) }, outputs: { text: port(TEXT), allowed: port(FLAG) }, resource: 'Local', reuse: 'Exact' },
      'op:write': { contract: 'op:write', implementation: 'fixture/write/1', inputs: { text: port(TEXT), mode: port(TEXT), extra: port(TEXT, true) }, outputs: { text: port(TEXT) }, resource: 'Provider', reuse: 'Fresh' },
      'op:note': { contract: 'op:note', implementation: 'fixture/note/1', inputs: { text: port(TEXT) }, outputs: { text: port(TEXT) }, resource: 'Provider', reuse: 'Fresh' },
      'op:pick': { contract: 'op:pick', implementation: 'fixture/pick/1', inputs: { text: port(TEXT) }, outputs: { text: port(TEXT) }, resource: 'Local', reuse: 'Exact' },
    },
    definition: {
      contract: 'c:pipeline',
      inputs: { seed: port(TEXT) },
      outputs: { written: port(TEXT), noted: port(TEXT, true), packed: port(TEXT) },
      nodes: {
        read: node('op:read', { seed: { Input: 'seed' } }),
        write: node('op:write', { text: out('read', 'text'), mode: { Constant: { contract: TEXT, value: omitted } }, extra: { Absent: TEXT } }),
        note: node('op:note', { text: out('read', 'text') }, { after: ['read'], guard: out('read', 'allowed'), activation: 'OnDemand' }),
        'pack/first': node('op:pick', { text: out('write', 'text') }),
        'pack/second': node('op:pick', { text: out('pack/first', 'text') }),
        'pack/inner/deep': node('op:pick', { text: out('pack/second', 'text') }),
      },
      results: { written: out('write', 'text'), noted: out('note', 'text'), packed: out('pack/inner/deep', 'text') },
      compositions: { pack: { artifact: 'a:pack', source: packSource, bindings: { text: out('write', 'text') } } },
    },
  } satisfies Artifact<ConstantDisclosure>)
}

export const PIPELINE_NODES = ['note', 'pack/first', 'pack/inner/deep', 'pack/second', 'read', 'write'] as const

type Attempts = Record<string, Array<Attempt<FaultDisclosure, string, string>>>

export function pipelineSnapshot(fields: {
  revision?: string
  nodes?: Partial<Record<string, Disposition>>
  attempts?: Attempts
  reasons?: Record<string, Array<Reason<FaultDisclosure, string>>>
  paused?: boolean
  active?: boolean
} = {}): InspectionSnapshot {
  const artifact = pipelineArtifact()
  const nodes = Object.fromEntries(Object.keys(artifact.definition.nodes).map(id => [id, fields.nodes?.[id] ?? 'Waiting'])) as Record<string, Disposition>
  return {
    protocol: 1, engine: 'engine-1', revision: fields.revision ?? '1', run: 'run-1', artifact_id: 'artifact-pipeline', artifact,
    nodes, activation: Object.fromEntries(Object.entries(artifact.definition.nodes).map(([id, n]) => [id, n.activation])),
    paused: fields.paused ?? false, active: fields.active ?? true, stepping: null, step_available: false,
    attempts: fields.attempts ?? {}, reasons: fields.reasons ?? {},
  }
}
