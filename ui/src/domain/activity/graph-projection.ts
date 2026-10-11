import type { Activation, Artifact, ConstantDisclosure, Contract, Definition, Port, Resource, Reuse, Shape, Source } from '../../generated/graph-contracts'

/// Structural projection of one exported executable artifact. It reads only the
/// artifact, never a run, so the same artifact always yields the same view (G2)
/// and every node and binding the artifact declares appears exactly once (G11).
/// Unknown shapes, dangling bindings and inconsistent composition metadata throw.

type DisclosedArtifact = Artifact<ConstantDisclosure>
type DisclosedDefinition = Definition<ConstantDisclosure>
type DisclosedSource = Source<ConstantDisclosure>

export interface PortRef {
  node: string
  port: string
}

/// Where one consumer input (or guard) gets its value. `wired` is a binding to
/// another operation's output and is drawn as a connection; the other kinds are
/// fixed when a run starts and are shown inside the consuming operation.
export type BoundValue =
  | { kind: 'wired'; contract: Contract; producer: PortRef }
  | { kind: 'captured'; contract: Contract; input: string }
  | { kind: 'constant'; contract: Contract }
  | { kind: 'absent'; contract: Contract }

export type InputBinding = BoundValue & { port: string }

export interface OutputPort {
  port: string
  contract: Contract
  optional: boolean
}

export interface ProjectedOperation {
  id: string
  /// The node's own segment: the id itself, or the part after its group prefix.
  name: string
  /// Innermost composition group containing this node, or null.
  group: string | null
  operation: Contract
  implementation: string
  resource: Resource
  reuse: Reuse
  activation: Activation
  inputs: InputBinding[]
  guard: BoundValue | null
  outputs: OutputPort[]
  after: string[]
}

export type GraphEdge =
  | { kind: 'data'; id: string; from: PortRef; to: PortRef; contract: Contract }
  | { kind: 'guard'; id: string; from: PortRef; to: string; contract: Contract }
  | { kind: 'control'; id: string; from: string; to: string }

export interface GraphResult {
  port: string
  contract: Contract
  value: BoundValue
}

export interface CompositionGroup {
  id: string
  parent: string | null
  artifact: string
  /// Operations directly inside this group, excluding nested groups' members.
  members: string[]
  /// Directly nested groups.
  children: string[]
}

export interface GraphCounts {
  operations: number
  bindings: number
  captured: number
  groups: number
}

export interface GraphView {
  artifactId: string
  contract: Contract
  /// Operation ids in a stable order.
  ids: string[]
  operations: Record<string, ProjectedOperation>
  edges: GraphEdge[]
  results: GraphResult[]
  groups: Record<string, CompositionGroup>
  counts: GraphCounts
  /// Every type contract the artifact declares, with its value shape.
  types: Record<Contract, Shape>
}

export interface ArtifactSnapshot {
  artifact_id: string
  artifact: DisclosedArtifact
}

const sorted = (values: Iterable<string>): string[] => [...values].sort()

function operationOf(artifact: DisclosedArtifact, node: string) {
  const definitionNode = artifact.definition.nodes[node]
  if (!definitionNode) throw new Error(`Graph binding names an unknown node: ${node}`)
  const operation = artifact.operations[definitionNode.operation]
  if (!operation) throw new Error(`Graph node ${node} uses an unregistered operation: ${definitionNode.operation}`)
  return operation
}

function outputPort(artifact: DisclosedArtifact, ref: PortRef): Port {
  const port = operationOf(artifact, ref.node).outputs[ref.port]
  if (!port) throw new Error(`Graph binding names an unknown output: ${ref.node}.${ref.port}`)
  return port
}

/// Resolve a source to a bound value. `declared` is the contract of the port it
/// feeds when that port has one; a guard has none and takes its source's contract.
function bind(artifact: DisclosedArtifact, source: DisclosedSource, declared: Contract | null, where: string): BoundValue {
  if ('Output' in source) {
    const producer = { node: source.Output.node, port: source.Output.port }
    return { kind: 'wired', contract: declared ?? outputPort(artifact, producer).contract, producer }
  }
  if ('Input' in source) {
    const input = artifact.definition.inputs[source.Input]
    if (!input) throw new Error(`${where} reads an undeclared graph input: ${source.Input}`)
    return { kind: 'captured', contract: declared ?? input.contract, input: source.Input }
  }
  if ('Constant' in source) return { kind: 'constant', contract: declared ?? source.Constant.contract }
  if ('Absent' in source) return { kind: 'absent', contract: declared ?? source.Absent }
  throw new Error(`${where} has an unrecognized source shape.`)
}

function collectGroups(definition: DisclosedDefinition): Record<string, CompositionGroup> {
  const groups: Record<string, CompositionGroup> = {}
  const walk = (id: string, boundary: DisclosedDefinition['compositions'][string], parent: string | null) => {
    if (groups[id]) throw new Error(`Composition group is declared twice: ${id}`)
    const nested = boundary.source.compositions
    groups[id] = {
      id, parent, artifact: boundary.artifact,
      members: sorted(Object.keys(boundary.source.nodes).map(name => `${id}/${name}`)),
      children: sorted(Object.keys(nested).map(prefix => `${id}/${prefix}`)),
    }
    for (const prefix of sorted(Object.keys(nested))) walk(`${id}/${prefix}`, nested[prefix], id)
  }
  for (const prefix of sorted(Object.keys(definition.compositions))) walk(prefix, definition.compositions[prefix], null)
  return groups
}

export function projectGraph(snapshot: ArtifactSnapshot): GraphView {
  const { artifact } = snapshot
  const definition = artifact.definition
  const groups = collectGroups(definition)
  const groupOf = new Map<string, string>()
  for (const group of Object.values(groups)) {
    for (const member of group.members) {
      if (!definition.nodes[member]) throw new Error(`Composition ${group.id} names a member missing from the artifact: ${member}`)
      const existing = groupOf.get(member)
      if (existing) throw new Error(`Node ${member} belongs to both ${existing} and ${group.id}`)
      groupOf.set(member, group.id)
    }
  }

  const ids = sorted(Object.keys(definition.nodes))
  const operations: Record<string, ProjectedOperation> = {}
  const edges: GraphEdge[] = []
  let captured = 0
  for (const id of ids) {
    const node = definition.nodes[id]
    const operation = operationOf(artifact, id)
    const declared = sorted(Object.keys(operation.inputs))
    const bound = sorted(Object.keys(node.inputs))
    if (declared.join('\n') !== bound.join('\n')) {
      throw new Error(`Node ${id} binds inputs [${bound.join(', ')}] but its operation declares [${declared.join(', ')}]`)
    }
    const inputs: InputBinding[] = bound.map(port => ({ port, ...bind(artifact, node.inputs[port], operation.inputs[port].contract, `Node ${id} input ${port}`) }))
    const guard = node.guard ? bind(artifact, node.guard, null, `Node ${id} guard`) : null
    for (const input of inputs) {
      if (input.kind === 'wired') {
        outputPort(artifact, input.producer)
        edges.push({ kind: 'data', id: JSON.stringify(['data', input.producer.node, input.producer.port, id, input.port]), from: input.producer, to: { node: id, port: input.port }, contract: input.contract })
      } else captured++
    }
    if (guard) {
      if (guard.kind === 'wired') edges.push({ kind: 'guard', id: JSON.stringify(['guard', guard.producer.node, guard.producer.port, id]), from: guard.producer, to: id, contract: guard.contract })
      else captured++
    }
    for (const parent of node.after) {
      if (!definition.nodes[parent]) throw new Error(`Node ${id} runs after an unknown node: ${parent}`)
      edges.push({ kind: 'control', id: JSON.stringify(['control', parent, id]), from: parent, to: id })
    }
    const group = groupOf.get(id) ?? null
    operations[id] = {
      id, name: group ? id.slice(group.length + 1) : id, group,
      operation: node.operation, implementation: operation.implementation, resource: operation.resource, reuse: operation.reuse,
      activation: node.activation, inputs, guard,
      outputs: sorted(Object.keys(operation.outputs)).map(port => ({ port, contract: operation.outputs[port].contract, optional: operation.outputs[port].optional })),
      after: [...node.after],
    }
  }

  const results = sorted(Object.keys(definition.results)).map(port => {
    const declared = definition.outputs[port]
    if (!declared) throw new Error(`Graph result ${port} has no declared output port.`)
    const value = bind(artifact, definition.results[port], declared.contract, `Graph result ${port}`)
    if (value.kind === 'wired') outputPort(artifact, value.producer)
    return { port, contract: declared.contract, value }
  })
  const unbound = Object.keys(definition.outputs).filter(port => !Object.hasOwn(definition.results, port))
  if (unbound.length) throw new Error(`Graph outputs have no result binding: ${unbound.join(', ')}`)

  return {
    artifactId: snapshot.artifact_id, contract: definition.contract, ids, operations, edges, results, groups,
    counts: { operations: ids.length, bindings: edges.length, captured, groups: Object.keys(groups).length },
    types: artifact.types,
  }
}

/// All operations inside a group, including nested groups' members.
export function groupMembers(view: GraphView, group: string): string[] {
  const entry = view.groups[group]
  if (!entry) throw new Error(`Unknown composition group: ${group}`)
  return sorted([...entry.members, ...entry.children.flatMap(child => groupMembers(view, child))])
}

/// The declared shape of a type contract. Every port contract an artifact uses
/// must be declared in its types.
export function shapeOf(view: GraphView, contract: Contract): Shape {
  const shape = view.types[contract]
  if (!shape) throw new Error(`Graph artifact does not declare the type ${contract}.`)
  return shape
}
