import { useEffect, useRef, useState } from 'react'
import { replaySelectionAudio } from '../../platform/audio/reading-speech'
import type { ContentRecord, ContentPayload } from '../../generated/contracts'
import { listOriginalContent, readOriginalContent, type OriginalSource } from '../../platform/ipc/original-content'
import { nativeError } from '../../platform/ipc/workspace'
import { useI18n } from '../localization/i18n'
import { ErrorNotice } from './ErrorNotice'
import { ContentValue } from './ContentValue'

type Playback = { rate: number; volume: number }

function OriginalAudio({ bytes, playback }: { bytes: string; playback: Playback }) {
  const tr = useI18n()
  const controller = useRef<AbortController | null>(null)
  const [playing, setPlaying] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => () => { controller.current?.abort() }, [bytes])
  const toggle = async () => {
    if (controller.current) { controller.current.abort(); controller.current = null; setPlaying(false); return }
    const current = new AbortController()
    controller.current = current
    setPlaying(true); setError(null)
    try { await replaySelectionAudio(bytes, current.signal, () => {}, playback.rate, playback.volume) }
    catch (reason) { if (!current.signal.aborted) setError(nativeError(reason)) }
    finally { if (controller.current === current) { controller.current = null; setPlaying(false) } }
  }
  return <>
    <button type="button" className="btn" onClick={() => { void toggle() }}>{tr(playing ? 'Stop' : 'Play')}</button>
    {error && <ErrorNotice error={error}>{error}</ErrorNotice>}
  </>
}

function OriginalBody({ record, refresh, playback }: { record: ContentRecord; refresh: number; playback: Playback }) {
  const tr = useI18n()
  const [payload, setPayload] = useState<ContentPayload | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [open, setOpen] = useState(record.byteCount <= 256 * 1024 || record.mediaType.startsWith('audio/'))
  useEffect(() => {
    if (!open) return
    setPayload(null); setError(null)
    let active = true
    void readOriginalContent(record.id).then(value => { if (active) setPayload(value) }).catch(reason => { if (active) setError(nativeError(reason)) })
    return () => { active = false }
  }, [record.id, record.byteCount, record.state, record.availability, open, refresh])
  return <section>
    <button type="button" className="btn" aria-expanded={open} onClick={() => setOpen(value => !value)}>{record.kind} · {record.state} · {record.availability} · {record.byteCount.toLocaleString(tr.browserLocale)} B</button>
    {open && <>
      {error && <ErrorNotice error={error}>{error}</ErrorNotice>}
      {!payload && !error && <p role="status">{tr('Loading…')}</p>}
      {payload && <>
        <ContentValue value={payload.record} compact={false} />
        {payload.text != null && <pre dir="auto">{payload.text}</pre>}
        {payload.base64 != null && (payload.record.mediaType.startsWith('audio/')
          ? <OriginalAudio bytes={payload.base64} playback={playback} />
          : <a className="btn" href={`data:application/octet-stream;base64,${payload.base64}`} download={`${record.id}.bin`}>{tr('Download')}</a>)}
      </>}
    </>}
  </section>
}

/** Current retention is separate from the historical graph revision being viewed. */
export function OriginalContent({ source, revision, playback }: { source: OriginalSource; revision?: string; playback: Playback }) {
  const tr = useI18n()
  const [refresh, setRefresh] = useState(0)
  const identity = JSON.stringify(source)
  const [result, setResult] = useState<{ identity: string; records?: ContentRecord[]; error?: string } | null>(null)
  useEffect(() => {
    let active = true
    void listOriginalContent(JSON.parse(identity) as OriginalSource).then(records => { if (active) setResult({ identity, records }) }).catch(reason => { if (active) setResult({ identity, error: nativeError(reason) }) })
    return () => { active = false }
  }, [identity, revision, refresh])
  const current = result?.identity === identity ? result : null
  return <section className="original-content">
    <h4>{tr('Original content · current retention')}</h4>
    <button type="button" className="btn" onClick={() => setRefresh(value => value + 1)}>{tr('Refresh')}</button>
    {'engine' in source && <p>{tr('Transport boundary: app ↔ inference service.')}</p>}
    {!current && <p role="status">{tr('Loading…')}</p>}
    {current?.error && <ErrorNotice error={current.error}>{current.error}</ErrorNotice>}
    {current?.records?.length === 0 && <p>{tr('No original transport content was retained for this source.')}</p>}
    {current?.records?.map(record => <OriginalBody key={record.id} record={record} refresh={refresh} playback={playback} />)}
  </section>
}
