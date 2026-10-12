import { useSettingsStore } from '../../state/settings/settings'
import { useEffect, useRef, useState } from 'react'
import type { InspectionSnapshot } from '../../generated/graph-contracts'
import type { NativeAttemptInspection } from '../../generated/contracts'
import { readNativeGraphAttempt } from '../../platform/ipc/window'
import { recordBotInspection } from '../../platform/ipc/effort'
import { nativeError } from '../../platform/ipc/workspace'
import { useI18n } from '../../components/localization/i18n'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { OriginalContent } from '../../components/feedback/OriginalContent'
import { ContentValue } from '../../components/feedback/ContentValue'
import { ResponseDetails } from '../../components/feedback/ResponseDetails'
import { InspectionContent, InspectionModeControl, type InspectionMode } from './InspectionContent'

/** Evidence is read only after the learner opens it, and is scoped to the exact cut. */
export function NativeAttemptDetails({ graph, node, expanded = false }: { graph: InspectionSnapshot; node: string; expanded?: boolean }) {
  const audioSettings = useSettingsStore(state => state.settings)
  const playback = { rate: audioSettings?.tts_rate ?? 1, volume: audioSettings ? audioSettings.master_volume * audioSettings.voice_volume / 10000 : 0 }
  const tr = useI18n()
  const [mode, setMode] = useState<InspectionMode>('readable')
  const credited = useRef(new Set<string>())
  const [creditError, setCreditError] = useState<{ key: string; error: string } | null>(null)
  const [open, setOpen] = useState(expanded)
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
  if (!attempts.length) return <p>{tr('No attempts yet.')}</p>
  const current = result?.key === key ? result : null
  const content = open && <>
      <label>{tr('Attempt ID')} <select className="field" aria-label={tr('Attempt ID')} value={attempt} onChange={event => setSelected(event.target.value)}>
        {attempts.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
      </select></label>
      {!current && <p role="status">{tr('Loading…')}</p>}
      {creditError?.key === key && <ErrorNotice as="p" error={creditError.error}>{creditError.error}</ErrorNotice>}
      {current?.error && <ErrorNotice as="p" error={current.error}>{current.error}</ErrorNotice>}
      {current?.value && <>
        <OriginalContent playback={playback} source={{ engine: current.value.engine, execution: current.value.execution }} revision={current.value.revision} />
        <InspectionModeControl mode={mode} onChange={setMode} />
        <section>
          <h4>{tr('What went in')}</h4>
          <InspectionContent text={JSON.stringify(current.value.inputs, null, 2) ?? 'null'} mode={mode} />
        </section>
        <section>
          <h4>{tr('Retained text')}</h4>
          {current.value.retainedText != null
            ? <InspectionContent text={current.value.retainedText} mode={mode} />
            : <p>{tr('No original text was retained for this attempt.')}</p>}
        </section>
        <section>
          <h4>{tr('What it made')}</h4>
          {current.value.outcome != null
            ? <InspectionContent text={JSON.stringify(current.value.outcome, null, 2)} mode={mode} />
            : <p>{tr('No completed result at this revision.')}</p>}
        </section>
        {current.value.evidence != null && <div className="ai-evidence"><ContentValue value={current.value.evidence} compact={false} /></div>}
        <ResponseDetails value={current.value} />
      </>}
    </>
  return expanded ? <div>{content}</div> : <details onToggle={event => setOpen(event.currentTarget.open)}>
    <summary>{tr('Response')}</summary>{content}
  </details>
}
