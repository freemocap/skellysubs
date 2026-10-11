import { useState } from 'react'
import { useI18n } from '../../../components/localization/i18n'
import { RunWorkspace } from '../../../features/activity/workspace/RunWorkspace'
import type { DefinitionSnapshot } from '../../../generated/graph-contracts'
import definition from '../../../generated/coach-graph.json'

const graph = definition as DefinitionSnapshot

/** The tour displays the generated executable coach artifact without executing it. */
export function AiDemo() {
  const tr = useI18n()
  const [popover, setPopover] = useState<string | null>(null)
  return <div className="demo-page">
    <section className="ai-view" data-mode="expanded" aria-label={tr('AI activity')}>
      <header className="ai-view-head"><h2 className="ai-view-title">{tr('AI activity')}</h2></header>
      <RunWorkspace graph={graph} preview={null} loadHistory={null} controls={null} orientation="across" compact={false}
        popover={popover} onPopover={setPopover} onHoverResults={() => {}} />
    </section>
  </div>
}
