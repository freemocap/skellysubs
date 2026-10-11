import type { Activation, Availability, Disposition, FaultDisclosure, Omission, Resource } from '../../../generated/graph-contracts'
import type { useI18n } from '../../../components/localization/i18n'
import { messageKey } from '../../../domain/localization'
import type { AttemptStateKind, NodeOverlay, ReasonFact } from '../../../domain/activity/graph-overlay'
import { shapeOf, type GraphView, type ProjectedOperation } from '../../../domain/activity/graph-projection'
import { humanizeId, operationVerb, shapeFields, shapeWords, typeName } from '../../../domain/activity/graph-types'

/// Interface wording for native graph facts. Every phrase is keyed by a closed
/// native enum or built from the artifact's own identifiers and shapes; no
/// operation is named here.

type Translate = ReturnType<typeof useI18n>

export const DISPOSITION_WORD = {
  Adopted: messageKey('Done'), Available: messageKey('Ready to use'),
  Prepared: messageKey('Starting'), Running: messageKey('Working'),
  Waiting: messageKey('Waiting'), Ready: messageKey('Queued'),
  Unrequested: messageKey('Runs when asked'),
  Blocked: messageKey('Blocked'), Paused: messageKey('Paused'), Held: messageKey('Held'),
  Failed: messageKey('Failed'), Unknown: messageKey('Outcome unknown'),
  Skipped: messageKey('Skipped'), Disabled: messageKey('Off'), Cancelled: messageKey('Cancelled'),
} as const satisfies Record<Disposition, string>

const ATTEMPT_WORD = {
  Prepared: messageKey('Starting'), Running: messageKey('Working'), Available: messageKey('Ready to use'),
  Adopted: messageKey('Done'), Failed: messageKey('Failed'), Unknown: messageKey('Outcome unknown'), Cancelled: messageKey('Cancelled'),
} as const satisfies Record<AttemptStateKind, string>

const RESOURCE_WORD = { Provider: messageKey('AI model call'), Local: messageKey('local work') } as const satisfies Record<Resource, string>
const ACTIVATION_WORD = { Automatic: messageKey('runs on its own'), OnDemand: messageKey('runs when asked'), Disabled: messageKey('turned off') } as const satisfies Record<Activation, string>
const AVAILABILITY_WORD = { Waiting: messageKey('waiting'), Blocked: messageKey('blocked'), Absent: messageKey('absent') } as const satisfies Record<Availability, string>
const OMISSION_WORD = { Content: messageKey('Withheld: content'), Unclassified: messageKey('Withheld: unclassified') } as const satisfies Record<Omission, string>

export function dispositionWord(tr: Translate, disposition: Disposition): string {
  return tr(DISPOSITION_WORD[disposition])
}

export function attemptWord(tr: Translate, state: AttemptStateKind): string {
  return tr(ATTEMPT_WORD[state])
}

export function nodeName(view: { operations: Record<string, ProjectedOperation> }, node: string): string {
  const operation = view.operations[node]
  if (!operation) throw new Error(`Unknown operation: ${node}`)
  return humanizeId(operation.name)
}

/// The verb phrase the artifact's author gave the operation.
export function nodeVerb(view: { operations: Record<string, ProjectedOperation> }, node: string): string {
  const operation = view.operations[node]
  if (!operation) throw new Error(`Unknown operation: ${node}`)
  return operationVerb(operation.operation)
}

/// What the operation produces, as the words of its output types.
export function outputWords(operation: ProjectedOperation): string {
  return operation.outputs.map(output => typeName(output.contract)).join(', ')
}

/// One line for a resting bubble: what it does and what that makes.
export function headline(view: GraphView, node: string): string {
  const operation = view.operations[node]
  if (!operation) throw new Error(`Unknown operation: ${node}`)
  const made = outputWords(operation)
  return made ? `${operationVerb(operation.operation)} → ${made}` : operationVerb(operation.operation)
}

/// The fields a typed value carries, for a port row's detail.
export function valueDetail(view: GraphView, contract: string): string {
  const shape = shapeOf(view, contract)
  return shapeFields(shape).length ? shapeWords(shape) : ''
}

/// How the operation runs: resource and activation, as a short phrase.
export function operationMeta(tr: Translate, operation: ProjectedOperation, activation: Activation): string {
  return tr('{value0} · {value1}', { value0: tr(RESOURCE_WORD[operation.resource]), value1: tr(ACTIVATION_WORD[activation]) })
}

export function faultText(tr: Translate, fault: FaultDisclosure): string {
  const code = 'Known' in fault.code ? fault.code.Known : tr(OMISSION_WORD[fault.code.Omitted])
  const path = 'Known' in fault.path ? fault.path.Known : tr(OMISSION_WORD[fault.path.Omitted])
  return tr('Code {value0} at {value1}', { value0: code, value1: path })
}

/// One sentence per reason, from the reason's own fields.
export function reasonText(tr: Translate, view: { operations: Record<string, ProjectedOperation> }, reason: ReasonFact): string {
  switch (reason.kind) {
    case 'demand-required': return tr('Runs only when it is requested.')
    case 'policy-disabled': return tr('Its activation policy is Disabled for this run.')
    case 'paused': return tr('The run is paused.')
    case 'revoked': return tr('Its authority to finish was revoked.')
    case 'guard-false': return tr('Skipped because its guard was false.')
    case 'guard-unavailable': return tr('Waiting for its guard value.')
    case 'admission': return reason.resource === 'Provider' ? tr('Waiting for a free provider slot.') : tr('Waiting for a free local slot.')
    case 'input': {
      const state = tr(AVAILABILITY_WORD[reason.availability])
      const port = reason.guard ? tr('guard') : reason.port
      if (reason.producer === null || reason.producerPort === null) return tr('Input {value0}: {value1}.', { value0: port, value1: state })
      return tr('Input {value0} ← {value1}.{value2}: {value3}.', { value0: port, value1: nodeName(view, reason.producer), value2: reason.producerPort, value3: state })
    }
    case 'control': return tr('Runs after {value0}, which is {value1}.', { value0: nodeName(view, reason.parent), value1: tr(DISPOSITION_WORD[reason.state]) })
    case 'attempt': {
      const text = tr('Attempt #{value0}: {value1}.', { value0: reason.id, value1: tr(ATTEMPT_WORD[reason.state]) })
      return reason.fault ? `${text} ${faultText(tr, reason.fault)}` : text
    }
  }
}

/// The short line at the foot of a card: what the run says about this step.
export function footerText(tr: Translate, view: { operations: Record<string, ProjectedOperation> }, overlay: NodeOverlay): string {
  const last = overlay.attempts[overlay.attempts.length - 1]
  if (overlay.tone === 'running' && last) return tr('Attempt #{value0} working', { value0: last.id })
  if (overlay.tone === 'problem' && last) return tr('Attempt #{value0}: {value1}', { value0: last.id, value1: tr(ATTEMPT_WORD[last.state]) })
  if (overlay.tone === 'done') return overlay.attempts.length === 1 ? tr('Done in one attempt') : tr('Done after {value0} attempts', { value0: overlay.attempts.length })
  const reason = overlay.reasons[0]
  if (reason) return reasonText(tr, view, reason)
  return dispositionWord(tr, overlay.disposition)
}

/// A summary built only from the artifact and recorded facts.
export function summaryText(tr: Translate, view: GraphView, node: string, overlay: NodeOverlay | null): string {
  const operation = view.operations[node]
  if (!operation) throw new Error(`Unknown operation: ${node}`)
  const activation = overlay?.activation ?? operation.activation
  const made = outputWords(operation)
  const does = made
    ? tr('{value0}: {value1}, producing {value2}.', { value0: nodeName(view, node), value1: operationVerb(operation.operation).toLowerCase(), value2: made })
    : tr('{value0}: {value1}.', { value0: nodeName(view, node), value1: operationVerb(operation.operation).toLowerCase() })
  const how = activation === 'Disabled' ? tr('This step is turned off for this run.')
    : operation.resource === 'Provider'
      ? activation === 'OnDemand' ? tr('It calls an AI model, and only when it is requested.') : tr('It calls an AI model on its own once its inputs are ready.')
      : activation === 'OnDemand' ? tr('It is local work that runs only when it is requested.') : tr('It is local work that runs on its own once its inputs are ready.')
  if (!overlay) return `${does} ${how} ${tr('No run is selected, so only its structure is shown.')}`
  const failed = overlay.attempts.filter(attempt => attempt.state === 'Failed')
  const last = overlay.attempts[overlay.attempts.length - 1]
  const state = overlay.retrying && last ? tr('An earlier attempt failed and attempt #{value0} is working now.', { value0: last.id })
    : overlay.tone === 'running' ? tr('It is working now.')
    : overlay.disposition === 'Adopted' ? tr('It finished and its result was used.')
    : overlay.disposition === 'Available' ? tr('It finished; its result is ready but not used yet.')
    : overlay.tone === 'ask' ? tr('Nobody has requested it in this run.')
    : overlay.tone === 'problem' ? tr('Its latest attempt did not finish successfully.')
    : overlay.tone === 'ended' ? tr('It ended without producing a result.')
    : tr('It has not started yet.')
  const history = failed.length && !overlay.retrying && overlay.tone !== 'problem' ? ` ${tr('Failed attempts: {value0}', { value0: failed.length })}` : ''
  return `${does} ${how} ${state}${history}`
}
