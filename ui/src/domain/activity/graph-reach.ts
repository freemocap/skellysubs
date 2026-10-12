import type { GraphView } from './graph-projection'

/// Graph results structurally downstream of an operation: results whose value
/// is bound to an output of the operation itself or of anything it feeds
/// through data or guard bindings. Control prerequisites carry no value and are
/// not followed. This states structural influence only; it does not say which
/// execution produced a value currently shown in the app.
export function downstreamResults(view: GraphView, node: string): string[] {
  if (!view.operations[node]) throw new Error(`Unknown operation: ${node}`)
  return resultsFed(view, new Set([node]))
}

function resultsFed(view: GraphView, start: Set<string>): string[] {
  const consumers = new Map<string, string[]>()
  for (const edge of view.edges) {
    if (edge.kind === 'control') continue
    const to = edge.kind === 'data' ? edge.to.node : edge.to
    consumers.set(edge.from.node, [...(consumers.get(edge.from.node) ?? []), to])
  }
  const reached = new Set(start)
  const queue = [...start]
  while (queue.length) {
    const current = queue.shift()!
    for (const next of consumers.get(current) ?? []) {
      if (reached.has(next)) continue
      reached.add(next)
      queue.push(next)
    }
  }
  return view.results.filter(result => result.value.kind === 'wired' && reached.has(result.value.producer.node)).map(result => result.port)
}
