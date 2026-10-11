import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { RunWorkspace } from '../src/features/activity/workspace/RunWorkspace'
import type { AttemptPreview, InspectionSnapshot, RunHistory } from '../src/generated/graph-contracts'
import { pipelineSnapshot } from '../tests/fixtures/graph-artifacts'
import '../src/styles/index.css'

/// Visual review of the AI workspace: a live run with retries, an on-demand
/// branch, a nested group and a streaming preview, across and down.
const graph: InspectionSnapshot = (() => { const snapshot = pipelineSnapshot({
  revision: '7',
  nodes: { read: 'Available', write: 'Running', note: 'Unrequested', 'pack/first': 'Waiting', 'pack/second': 'Waiting', 'pack/inner/deep': 'Waiting' },
  attempts: {
    read: [{ id: '1', execution: '1', state: 'Available', acquisition: 'Produced' }],
    write: [
      { id: '2', execution: '2', state: { Failed: { code: { Known: 'invalid_value' }, path: { Omitted: 'Content' } } }, acquisition: 'Produced' },
      { id: '3', execution: '3', state: 'Running', acquisition: 'Produced' },
    ],
  },
  reasons: { note: ['DemandRequired'] },
})
  snapshot.artifact.operations['op:read'].resource = 'Provider'
  return snapshot
})()
const preview: AttemptPreview = { attempt: '3', execution: '3', capture: { session: 's1', sequence: '4', text: 'Hola, ¿cómo estás? Hoy vamos a practicar', failure: null }, retained_sequence: null, uncommitted: true, live: true, complete: false }
const history = (): Promise<RunHistory> => Promise.resolve({ engine: graph.engine, run: graph.run, current_revision: '7', frames: [graph], before: null } as RunHistory)

function Panel({ orientation }: { orientation: 'across' | 'down' }) {
  const [popover, setPopover] = useState<string | null>(null)
  return <section className="ai-view" data-mode="expanded" style={{ height: '90vh' }}>
    <header className="ai-view-head"><h2 className="ai-view-title">AI activity · {orientation}</h2></header>
    <RunWorkspace graph={graph} preview={preview} loadHistory={history} controls={null} orientation={orientation} compact={orientation === 'down'}
      popover={popover} onPopover={setPopover} onHoverResults={() => {}} />
  </section>
}

const which = new URLSearchParams(location.search).get('orientation') === 'down' ? 'down' : 'across'
createRoot(document.getElementById('root')!).render(<Panel orientation={which} />)
