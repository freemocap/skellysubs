import { useEffect, useRef, useState } from 'react'
import type { InspectionSnapshot } from '../../generated/graph-contracts'
import type { NativeAttemptInspection } from '../../generated/contracts'
import { readNativeGraphAttempt } from '../../platform/ipc/window'
import { recordBotInspection } from '../../platform/ipc/effort'
import { nativeError } from '../../platform/ipc/workspace'
import { useI18n } from '../../components/localization/i18n'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { ResponseDetails } from '../../components/feedback/ResponseDetails'
import { InspectionContent } from './InspectionContent'

/** Evidence is read only after the learner opens it, and is scoped to the exact cut. */
export function NativeAttemptDetails({ graph, node }: { graph: InspectionSnapshot; node: string }) {
  const tr = useI18n()
  const credited = useRef(new Set<string>())
  const [creditError, setCreditError] = useState<{ key: string; error: string } | null>(null)
  const [open, setOpen] = useState(false)
  const [selected, setSelected] = useState<string | null>(null)
  const attempts = graph.attempts[node] ?? []
  const attempt = selected === null ? attempts.at(-1)?.id : selected
  const key = JSON.stringify([graph.engine, graph.run, graph.artifact_id, graph.revision, node, attempt])
  const [result, setResult] = useState<{ key: string; value?: NativeAttemptInspection; error?: string } | null>(null)
  const loadedKey = useRef<string | null>(null)
  useEffect(() => {
    if (!open || !attempt) return
    if (loadedKey.current === key) return
    loadedKey.current = null
    let active = true
    void readNativeGraphAttempt(graph.engine, graph.run, graph.revision, node, attempt).then(value => {
      if (!active) return
      if (value.engine !== graph.engine || value.run !== graph.run || value.artifact !== graph.artifact_id || value.revision !== graph.revision || value.node !== node || value.attempt !== attempt)
        throw new Error('Graph attempt evidence does not match the selection.')
      loadedKey.current = key
      setResult({ key, value })
    }).catch(error => { if (active) setResult({ key, error: nativeError(error) }) })
    return () => { active = false }
  }, [open, key, graph.engine, graph.run, graph.artifact_id, graph.revision, node, attempt])
  // Record deliberate inspection after rendering; credit never gates evidence.
  useEffect(() => {
    const value = result?.key === key && open ? result.value : undefined
    if (!value || (value.evidence == null && value.retainedText == null)) return
    const identity = JSON.stringify([value.engine, value.attempt])
    if (credited.current.has(identity)) return
    credited.current.add(identity)
    void recordBotInspection(value).catch(error => {
      credited.current.delete(identity)
      setCreditError({ key, error: nativeError(error) })
    })
  }, [result, key, open])
  if (!attempts.length) return null
  const current = result?.key === key ? result : null
  return <details onToggle={event => setOpen(event.currentTarget.open)}>
    <summary>{tr('Response')}</summary>
    {open && <>
      <label>{tr('Attempt ID')} <select className="field" aria-label={tr('Attempt ID')} value={attempt} onChange={event => setSelected(event.target.value)}>
        {attempts.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
      </select></label>
      {!current && <p role="status">{tr('Loading…')}</p>}
      {creditError?.key === key && <ErrorNotice as="p" error={creditError.error}>{creditError.error}</ErrorNotice>}
      {current?.error && <ErrorNotice as="p" error={current.error}>{current.error}</ErrorNotice>}
      {current?.value && <>
        <ResponseDetails value={current.value} />
        {current.value.retainedText != null && <InspectionContent text={current.value.retainedText} mode="readable" />}
      </>}
    </>}
  </details>
}
