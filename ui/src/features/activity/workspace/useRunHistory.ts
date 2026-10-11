import { useCallback, useEffect, useRef, useState } from 'react'
import type { RunHistory } from '../../../generated/graph-contracts'
import { compareDecimal } from '../../../domain/activity/graph-overlay'
import { mergeHistory, type LoadedHistory } from '../../../domain/activity/graph-history'
import { nativeError } from '../../../platform/ipc/workspace'

/// Recorded run history, read only on request. While `follow` is set the newest
/// page is refreshed when the live revision passes what was loaded: one read in
/// flight, and a newer trigger replaces a queued one. A failed read is reported
/// and retried only on request.
export function useRunHistory(engine: string, run: string, artifactId: string, load: (before: string | null) => Promise<RunHistory>, enabled: boolean, follow: boolean, liveRevision: string) {
  const [history, setHistory] = useState<LoadedHistory | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const key = JSON.stringify([engine, run, artifactId])
  const scope = useRef(key)
  const inFlight = useRef<object | null>(null)
  const queued = useRef<string | null | undefined>(undefined)
  const loaded = useRef<LoadedHistory | null>(null)
  const failed = useRef(false)
  const mounted = useRef(true)
  useEffect(() => { mounted.current = true; return () => { mounted.current = false } }, [])
  const loader = useRef(load)
  loader.current = load

  if (scope.current !== key) {
    scope.current = key
    inFlight.current = null
    loaded.current = null
    queued.current = undefined
    failed.current = false
    if (history !== null) setHistory(null)
    if (error !== null) setError(null)
    if (loading) setLoading(false)
  }

  const read = useCallback((before: string | null) => {
    if (inFlight.current) { queued.current = before; return }
    const request = {}
    inFlight.current = request
    setLoading(true)
    const requested = key
    loader.current(before)
      .then(async page => {
        if (!mounted.current || scope.current !== requested) return
        const previousNewest = loaded.current?.frames.at(-1)?.revision
        const pages = [page]
        mergeHistory(null, page, { engine, run, artifactId })
        // A refresh may have advanced by more than a page. Read the intervening
        // cuts before joining it to old frames; never draw a continuous lane
        // across history we have not read.
        while (before === null && previousNewest && page.before && page.frames.length && compareDecimal(page.frames[0].revision, previousNewest) > 0) {
          const cursor = page.before
          page = await loader.current(cursor)
          if (!mounted.current || scope.current !== requested) return
          mergeHistory(null, page, { engine, run, artifactId })
          if (!page.frames.length || compareDecimal(page.frames[0].revision, cursor) >= 0) throw new Error('Graph history cursor did not advance.')
          pages.push(page)
        }
        let merged = loaded.current
        for (const part of pages) merged = mergeHistory(merged, part, { engine, run, artifactId })
        loaded.current = merged
        failed.current = false
        setHistory(merged)
        setError(null)
      })
      .catch(cause => {
        if (!mounted.current || scope.current !== requested) return
        failed.current = true
        setError(nativeError(cause))
      })
      .finally(() => {
        if (inFlight.current !== request) return
        inFlight.current = null
        if (!mounted.current) return
        setLoading(false)
        const next = queued.current
        queued.current = undefined
        if (next !== undefined && scope.current === requested && !failed.current) read(next)
      })
  }, [key, engine, run, artifactId])

  useEffect(() => {
    if (!enabled || failed.current) return
    const current = loaded.current
    if (!current) read(null)
    else if (follow && compareDecimal(liveRevision, current.currentRevision) > 0) read(null)
  }, [enabled, follow, liveRevision, read])

  const loadEarlier = useCallback(() => {
    const current = loaded.current
    if (!current || current.before === null) throw new Error('No earlier history is available to load.')
    read(current.before)
  }, [read])

  const retry = useCallback(() => { failed.current = false; read(null) }, [read])

  return { history, error, loading, loadEarlier, retry }
}
