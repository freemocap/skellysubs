import type { Acquisition, Activation, Attempt, AttemptPreview, AttemptState, Availability, Disposition, FaultDisclosure, InspectionSnapshot, Reason, Resource } from '../../generated/graph-contracts'
import { groupMembers, type GraphView } from './graph-projection'

/// Runtime facts of one native snapshot, laid over a structural projection.
/// Everything is copied from the snapshot or resolved against the artifact's own
/// bindings; nothing here evaluates eligibility or invents a state.

export type Tone = 'done' | 'running' | 'waiting' | 'ask' | 'held' | 'problem' | 'ended'

/// Seven presentation tones for the fifteen native dispositions. The exact
/// disposition is always carried alongside its tone.
export const DISPOSITION_TONE = {
  Adopted: 'done', Available: 'done',
  Prepared: 'running', Running: 'running',
  Waiting: 'waiting', Ready: 'waiting',
  Unrequested: 'ask',
  Blocked: 'held', Paused: 'held', Held: 'held',
  Failed: 'problem', Unknown: 'problem',
  Skipped: 'ended', Disabled: 'ended', Cancelled: 'ended',
} as const satisfies Record<Disposition, Tone>

export const TONES: readonly Tone[] = ['done', 'running', 'waiting', 'ask', 'held', 'problem', 'ended']

export type AttemptStateKind = 'Prepared' | 'Running' | 'Available' | 'Adopted' | 'Failed' | 'Unknown' | 'Cancelled'

export interface AttemptSummary {
  id: string
  execution: string
  state: AttemptStateKind
  acquisition: Acquisition
  fault: FaultDisclosure | null
}

/// A reason as data. `input` names the consumer's own input port and, when the
/// artifact binds it to an operation output, the producer's output port.
export type ReasonFact =
  | { kind: 'demand-required' }
  | { kind: 'policy-disabled' }
  | { kind: 'paused' }
  | { kind: 'revoked' }
  | { kind: 'guard-false' }
  | { kind: 'guard-unavailable' }
  | { kind: 'admission'; resource: Resource }
  | { kind: 'input'; port: string; guard: boolean; producer: string | null; producerPort: string | null; availability: Availability }
  | { kind: 'control'; parent: string; state: Disposition }
  | { kind: 'attempt'; id: string; state: AttemptStateKind; fault: FaultDisclosure | null }

export interface NodeOverlay {
  disposition: Disposition
  tone: Tone
  /// Effective activation captured for the run.
  activation: Activation
  attempts: AttemptSummary[]
  reasons: ReasonFact[]
  /// Running now after an earlier attempt of this node failed.
  retrying: boolean
}

export interface GraphOverlay {
  engine: string
  run: string
  revision: string
  paused: boolean
  active: boolean
  stepping: string | null
  stepAvailable: boolean
  nodes: Record<string, NodeOverlay>
  status: { running: string[]; retrying: string[]; tones: Record<Tone, number> }
}

/// Order decimal-string identifiers (attempt ids, revisions) exactly.
export function compareDecimal(a: string, b: string): number {
  if (!/^\d+$/.test(a) || !/^\d+$/.test(b)) throw new Error(`Expected decimal identifiers, got ${a} and ${b}.`)
  const left = BigInt(a), right = BigInt(b)
  return left < right ? -1 : left > right ? 1 : 0
}

type DisclosedAttempt = Attempt<FaultDisclosure, string, string>
type DisclosedReason = Reason<FaultDisclosure, string>

function attemptState(state: AttemptState<FaultDisclosure>): { state: AttemptStateKind; fault: FaultDisclosure | null } {
  if (typeof state === 'string') return { state, fault: null }
  if ('Failed' in state) return { state: 'Failed', fault: state.Failed }
  throw new Error('Unrecognized attempt state shape.')
}

function summarize(attempt: DisclosedAttempt): AttemptSummary {
  return { id: attempt.id, execution: attempt.execution, acquisition: attempt.acquisition, ...attemptState(attempt.state) }
}

function reasonFact(view: GraphView, node: string, reason: DisclosedReason): ReasonFact {
  if (typeof reason === 'string') {
    switch (reason) {
      case 'PolicyDisabled': return { kind: 'policy-disabled' }
      case 'DemandRequired': return { kind: 'demand-required' }
      case 'Paused': return { kind: 'paused' }
      case 'Revoked': return { kind: 'revoked' }
      case 'GuardFalse': return { kind: 'guard-false' }
      case 'GuardUnavailable': return { kind: 'guard-unavailable' }
    }
    throw new Error(`Unrecognized reason: ${String(reason)}`)
  }
  if ('Admission' in reason) return { kind: 'admission', resource: reason.Admission }
  if ('Control' in reason) {
    const { parent, state } = reason.Control
    if (!view.operations[node].after.includes(parent)) throw new Error(`Node ${node} reports a control reason for ${parent}, which it does not run after.`)
    return { kind: 'control', parent, state }
  }
  if ('Attempt' in reason) return { kind: 'attempt', id: reason.Attempt.id, ...attemptState(reason.Attempt.state) }
  if ('Input' in reason) {
    const { port, producer, availability } = reason.Input
    const operation = view.operations[node]
    const input = operation.inputs.find(candidate => candidate.port === port)
    const guard = !input && operation.guard?.kind === 'wired' && operation.guard.producer.node === producer ? operation.guard : null
    const binding = input ?? guard
    if (!binding) throw new Error(`Node ${node} reports an input reason for unknown port ${port}.`)
    if (producer === null) return { kind: 'input', port, guard: !input, producer: null, producerPort: null, availability }
    if (binding.kind !== 'wired' || binding.producer.node !== producer) {
      throw new Error(`Node ${node} reports input ${port} from ${producer}, but the artifact does not bind it to that producer.`)
    }
    return { kind: 'input', port, guard: !input, producer, producerPort: binding.producer.port, availability }
  }
  throw new Error('Unrecognized reason shape.')
}

export function overlayGraph(view: GraphView, snapshot: InspectionSnapshot): GraphOverlay {
  if (snapshot.artifact_id !== view.artifactId) throw new Error(`Snapshot artifact ${snapshot.artifact_id} does not match projected artifact ${view.artifactId}.`)
  const reported = Object.keys(snapshot.nodes).sort()
  if (reported.join('\n') !== view.ids.join('\n')) throw new Error('Snapshot nodes do not match the projected artifact nodes.')
  const tones = Object.fromEntries(TONES.map(tone => [tone, 0])) as Record<Tone, number>
  const nodes: Record<string, NodeOverlay> = {}
  const running: string[] = []
  const retrying: string[] = []
  for (const id of view.ids) {
    const disposition = snapshot.nodes[id]
    const tone: Tone = DISPOSITION_TONE[disposition]
    if (!tone) throw new Error(`Unrecognized disposition for ${id}: ${String(disposition)}`)
    const activation = snapshot.activation[id]
    if (!activation) throw new Error(`Snapshot has no effective activation for ${id}.`)
    const attempts = (snapshot.attempts[id] ?? []).map(summarize).sort((a, b) => compareDecimal(a.id, b.id))
    const reasons = (snapshot.reasons[id] ?? []).map(reason => reasonFact(view, id, reason))
    const isRunning = tone === 'running'
    const isRetrying = isRunning && attempts.some((attempt, index) => attempt.state === 'Failed' && index < attempts.length - 1)
    nodes[id] = { disposition, tone, activation, attempts, reasons, retrying: isRetrying }
    tones[tone]++
    if (isRunning) running.push(id)
    if (isRetrying) retrying.push(id)
  }
  return {
    engine: snapshot.engine, run: snapshot.run, revision: snapshot.revision, paused: snapshot.paused, active: snapshot.active,
    stepping: snapshot.stepping, stepAvailable: snapshot.step_available, nodes, status: { running, retrying, tones },
  }
}

/// A folded composition's members and their facts. A group has no state of its
/// own; callers show these members, never a synthesized group disposition.
export function groupOverlay(view: GraphView, overlay: GraphOverlay, group: string): Array<{ id: string; disposition: Disposition; tone: Tone }> {
  return groupMembers(view, group).map(id => {
    const node = overlay.nodes[id]
    if (!node) throw new Error(`Overlay has no facts for group member ${id}.`)
    return { id, disposition: node.disposition, tone: node.tone }
  })
}

/// The node whose current attempt produced a live preview, or null when the
/// preview belongs to an attempt this snapshot does not list.
export function previewNode(snapshot: InspectionSnapshot, preview: AttemptPreview): string | null {
  const matches = Object.entries(snapshot.attempts)
    .filter(([, attempts]) => attempts.some(attempt => attempt.id === preview.attempt && attempt.execution === preview.execution))
    .map(([node]) => node)
  if (matches.length > 1) throw new Error(`Preview attempt ${preview.attempt} is listed under several nodes.`)
  return matches[0] ?? null
}
