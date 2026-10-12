import { useCallback, useEffect, useState } from 'react'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { useI18n } from '../../components/localization/i18n'
import type { NativeRunEntry, AiRevisionSelection } from '../../generated/contracts'
import type { InspectionSnapshot } from '../../generated/graph-contracts'
import { readNativeRunHistory, readNativeWorkspaceGraph } from '../../platform/ipc/window'
import { nativeError } from '../../platform/ipc/workspace'
import type { Orientation } from './workspace/layout'
import { RunWorkspace } from './workspace/RunWorkspace'

const POLL_MS = 1000

/// A run owned outside the conversation (reading, speech, proposals). Its graph
/// is read from persisted native ownership while visible; it has no controls.
export function WorkspaceRunView({ entry, visible, orientation, compact, popover, onPopover, onHoverResults, initialRevision, onRevision }: {
  initialRevision?: AiRevisionSelection
  onRevision?: (revision: AiRevisionSelection) => void
  entry: NativeRunEntry
  visible: boolean
  orientation: Orientation
  compact: boolean
  popover: string | null
  onPopover: (node: string | null) => void
  onHoverResults: (results: string[] | null) => void
}) {
  const tr = useI18n()
  const [graph, setGraph] = useState<InspectionSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const loadHistory = useCallback((before: string | null) => readNativeRunHistory(entry.engine, entry.run, before), [entry.engine, entry.run])
  useEffect(() => {
    if (!visible) return
    let active = true
    let timer: ReturnType<typeof setTimeout> | undefined
    let revision: string | null = null
    async function read() {
      try {
        const current = await readNativeWorkspaceGraph(entry.engine, entry.run, revision)
        if (!active) return
        if (current) {
          if (current.engine !== entry.engine || current.run !== entry.run || current.artifact_id !== entry.artifact) throw new Error('Graph history does not match the selected run.')
          revision = current.revision
          setGraph(current)
        }
        timer = setTimeout(() => void read(), POLL_MS)
      } catch (cause) { if (active) setError(nativeError(cause)) }
    }
    void read()
    return () => { active = false; clearTimeout(timer) }
  }, [visible, entry.engine, entry.run, entry.artifact])
  if (error) return <ErrorNotice as="p" className="ai-error" error={error}>{error}</ErrorNotice>
  if (!graph) return <p role="status" className="ai-muted">{tr('Loading…')}</p>
  return <RunWorkspace graph={graph} preview={null} stations={[]} turn={null} messages={[]} loadHistory={loadHistory} controls={null} orientation={orientation} compact={compact}
    initialRevision={initialRevision} onRevision={onRevision} popover={popover} onPopover={onPopover} onHoverResults={onHoverResults} />
}
