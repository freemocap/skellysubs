import { useState } from 'react'
import { ToolbarIcon } from '../../../components/controls/ToolbarIcon'
import { DetailDialog } from '../../../components/dialogs/DetailDialog'
import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { useI18n } from '../../../components/localization/i18n'
import type { InspectionSnapshot } from '../../../generated/graph-contracts'
import type { Timeline } from '../../../domain/activity/graph-history'
import type { GraphOverlay } from '../../../domain/activity/graph-overlay'
import type { GraphView } from '../../../domain/activity/graph-projection'
import { typeName } from '../../../domain/activity/graph-types'
import { NativeAttemptDetails } from '../NativeAttemptDetails'
import { ContentValue } from '../../../components/feedback/ContentValue'
import type { StepProduct } from '../../../domain/conversation/record-view'
import { typeColor, typeInk } from './geometry'
import type { Preview } from './NodeItems'
import { attemptWord, dispositionWord, faultText, nodeName, operationMeta, reasonText, summaryText } from './wording'

/// Everything recorded about one operation at the selected revision: first what
/// it made, then the response evidence the provider adapter retained, then its
/// ports, tries and reasons. Deliberately opening the inspector loads evidence
/// for the selected attempt and revision.
export function NodePopover({ view, node, overlay, snapshot, timeline, preview, products, onClose, onSelectNode }: {
  view: GraphView
  node: string
  overlay: GraphOverlay | null
  /// The snapshot the overlay came from, when a run is shown.
  snapshot: InspectionSnapshot | null
  timeline: Timeline | null
  preview: Preview | null
  /// What the step made, as the conversation shows it.
  products: StepProduct[]
  onClose: () => void
  onSelectNode: (node: string) => void
}) {
  const tr = useI18n()
  const [copyError, setCopyError] = useState<string | null>(null)
  const operation = view.operations[node]
  if (!operation) throw new Error(`Unknown operation: ${node}`)
  const facts = overlay?.nodes[node] ?? null
  const name = nodeName(view, node)
  const shown = preview?.node === node ? preview : null
  const copy = (text: string) => {
    setCopyError(null)
    navigator.clipboard.writeText(text).catch(cause => setCopyError(String(cause)))
  }
  const ready = (producer: string) => {
    if (!overlay) return tr('structure only')
    const disposition = overlay.nodes[producer].disposition
    return dispositionWord(tr, disposition)
  }
  const lane = timeline?.lanes[node] ?? null
  const spans = timeline?.attempts[node] ?? []
  return <DetailDialog title={name} onClose={onClose} size="wide" closeControl="content" className="ai-popover">
    <header className="ai-popover-head">
      <h2>{name}</h2>
      {facts && <span className="ai-status-chip" data-tone={facts.tone}><span className="ai-dot" aria-hidden="true" />{dispositionWord(tr, facts.disposition)}</span>}
      <button type="button" className="ai-icon-button ai-popover-close" onClick={onClose} aria-label={tr('Close {value0}', { value0: name })}><ToolbarIcon name="close" size={17} /></button>
    </header>
    <p className="ai-popover-summary">{summaryText(tr, view, node, facts)}</p>
    <p className="ai-popover-meta">{operationMeta(tr, operation, facts?.activation ?? operation.activation)}</p>

    {(products.length > 0 || shown) && <section aria-labelledby="ai-made" className="ai-popover-made">
      <h3 id="ai-made">{tr('What it made')}</h3>
      {shown && <div className="ai-popover-preview">
        <span className="ai-card-slot-label">{shown.live ? tr('Live preview · not used yet') : tr('Retained text')}</span>
        <p dir="auto">{shown.text}{shown.live && <span className="ai-caret" aria-hidden="true" />}</p>
      </div>}
      {products.map(product => <div key={`${product.role}:${product.field}`} className="ai-popover-product">
        <span className="ai-popover-product-name">{product.role === 'user' ? tr('your message') : tr('partner message')} · {product.field}</span>
        <ContentValue value={product.value} compact={false} />
      </div>)}
    </section>}

    {snapshot && <section aria-labelledby="ai-evidence">
      <h3 id="ai-evidence">{tr('Response evidence')}</h3>
      <p className="ai-popover-meta">{tr('Retained inputs, outputs and response metadata for this attempt.')}</p>
      <NativeAttemptDetails key={JSON.stringify([snapshot.engine, snapshot.run, node])} graph={snapshot} node={node} expanded />
    </section>}

    <div className="ai-popover-columns">
      <section aria-labelledby="ai-went-in">
        <h3 id="ai-went-in">{tr('What went in')}</h3>
        <dl className="ai-popover-ports">
          {operation.inputs.map(input => <div key={input.port} className="ai-popover-port">
            <dt><span className="ai-pin-mark" style={input.kind === 'wired' ? { background: typeColor(input.contract) } : undefined} data-kind={input.kind} />{input.port}</dt>
            <dd>
              <span className="ai-port-type" style={{ color: typeInk(input.contract) }} title={input.contract}>{typeName(input.contract)}</span>
              <span className="ai-popover-source">{input.kind === 'wired' ? <button type="button" className="inspection-action" onClick={() => onSelectNode(input.producer.node)}>{tr('From {value0}.{value1} · {value2}', { value0: nodeName(view, input.producer.node), value1: input.producer.port, value2: ready(input.producer.node) })}</button>
                : input.kind === 'captured' ? tr('Inputs fixed when the run started')
                : input.kind === 'constant' ? tr('Fixed in the graph')
                : tr('Absent for this run')}</span>
            </dd>
          </div>)}
          {operation.guard && <div className="ai-popover-port">
            <dt><span className="ai-pin-mark is-guard" style={operation.guard.kind === 'wired' ? { background: typeColor(operation.guard.contract) } : undefined} data-kind={operation.guard.kind} />{tr('guard')}</dt>
            <dd>
              <span className="ai-port-type" style={{ color: typeInk(operation.guard.contract) }} title={operation.guard.contract}>{typeName(operation.guard.contract)}</span>
              <span className="ai-popover-source">{operation.guard.kind === 'wired' ? tr('From {value0}.{value1} · {value2}', { value0: nodeName(view, operation.guard.producer.node), value1: operation.guard.producer.port, value2: ready(operation.guard.producer.node) }) : tr('Fixed in the graph')}</span>
            </dd>
          </div>}
          {operation.after.map(parent => <div key={`after:${parent}`} className="ai-popover-port">
            <dt><span className="ai-pin-mark" data-kind="control" />{tr('runs after')}</dt>
            <dd><span className="ai-popover-source"><button type="button" className="inspection-action" onClick={() => onSelectNode(parent)}>{nodeName(view, parent)}</button>{overlay ? ` · ${dispositionWord(tr, overlay.nodes[parent].disposition)}` : ''}</span></dd>
          </div>)}
        </dl>
      </section>
      <section aria-labelledby="ai-came-out">
        <h3 id="ai-came-out">{tr('What came out')}</h3>
        <dl className="ai-popover-ports">
          {operation.outputs.map(output => <div key={output.port} className="ai-popover-port">
            <dt><span className="ai-pin-mark" style={{ background: typeColor(output.contract) }} />{output.port}</dt>
            <dd><span className="ai-port-type" style={{ color: typeInk(output.contract) }} title={output.contract}>{typeName(output.contract)}</span></dd>
          </div>)}
        </dl>
      </section>
    </div>

    <section aria-labelledby="ai-tries">
      <h3 id="ai-tries">{tr('Tries')}</h3>
      {!facts && <p className="ai-muted">{tr('No run is selected.')}</p>}
      {facts && !facts.attempts.length && <p className="ai-muted">{tr('No attempts yet.')}</p>}
      {facts && facts.attempts.length > 0 && <ul className="ai-tries">
        {facts.attempts.map(attempt => {
          const span = spans.find(candidate => candidate.id === attempt.id)
          return <li key={attempt.id} className="ai-try" data-state={attempt.state}>
            <span className="ai-try-head"><span className="ai-dot" aria-hidden="true" />{attemptWord(tr, attempt.state)}<span className="ai-try-id">#{attempt.id}</span></span>
            {span && timeline && <span className="ai-try-span">{tr('Revisions {value0}–{value1}', { value0: timeline.revisions[span.fromIndex], value1: timeline.revisions[span.toIndex] })}{span.openStart ? ` · ${tr('Started before the loaded history')}` : ''}</span>}
            {attempt.fault && <span className="ai-try-fault">{faultText(tr, attempt.fault)}</span>}
          </li>
        })}
      </ul>}
      {lane && timeline && <div className="ai-lane-cells ai-popover-lane" style={{ gridTemplateColumns: `repeat(${timeline.revisions.length}, minmax(0, 1fr))` }} aria-hidden="true">
        {lane.map(segment => <span key={segment.fromIndex} className="ai-lane-segment" data-tone={segment.tone} data-open-start={segment.openStart || undefined} data-open-end={segment.openEnd || undefined}
          style={{ gridColumn: `${segment.fromIndex + 1} / ${segment.toIndex + 2}` }} />)}
      </div>}
    </section>

    {facts && facts.reasons.length > 0 && <section aria-labelledby="ai-why">
      <h3 id="ai-why">{tr('Why')}</h3>
      <ul className="ai-reasons">{facts.reasons.map((reason, index) => <li key={index}>{reasonText(tr, view, reason)}</li>)}</ul>
    </section>}

    <details className="ai-developer">
      <summary>{tr('For developers')}</summary>
      <dl className="ai-facts">
        <dt>{tr('Node')}</dt><dd>{node}</dd>
        <dt>{tr('State')}</dt><dd>{facts?.disposition ?? tr('Structure')}</dd>
        <dt>{tr('Contract')}</dt><dd>{operation.operation}</dd>
        <dt>{tr('Definition source')}</dt><dd>{operation.implementation}</dd>
        <dt>{tr('Policy')}</dt><dd>{`${operation.resource} · ${operation.reuse} · ${facts?.activation ?? operation.activation}`}</dd>
      </dl>
      <div className="ai-developer-actions">
        <button type="button" className="ai-chip" onClick={() => copy(operation.implementation)}>{tr('Copy implementation ID')}</button>
        {snapshot && <button type="button" className="ai-chip" onClick={() => copy(JSON.stringify(snapshot, null, 2))}>{tr('Copy snapshot JSON')}</button>}
      </div>
      {copyError && <ErrorNotice as="p" error={copyError}>{copyError}</ErrorNotice>}
    </details>
  </DetailDialog>
}
