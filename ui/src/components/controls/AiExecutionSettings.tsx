import type { Settings } from '../../types'
import type { ExecutionMode, ExecutionPreferences } from '../../generated/contracts'
import { useI18n } from '../localization/i18n'
import { ErrorNotice } from '../feedback/ErrorNotice'
import { nativeError } from '../../platform/ipc/workspace'

/** View only; the settings owner supplies the shared controller. */
export function AiExecutionSettings({ settings, busy, error, change, reload }: {
  settings: Settings | null; busy: boolean; error: unknown
  change: (feature: keyof ExecutionPreferences, mode: ExecutionMode) => Promise<void>
  reload: () => void
}) {
  const tr = useI18n()
  const fields: [keyof ExecutionPreferences, string][] = [
    ['assessment', tr('Message assessment')],
    ['replyBrief', tr('Reply brief')], ['reading', tr('Reading support')],
  ]
  return <details className="ai-execution-settings" onToggle={event => { if (event.currentTarget.open) reload() }}>
    <summary>{tr('Automatic AI work')}</summary>
    <div className="ai-execution-fields">
    <p className="hint">{tr('Coaching feedback')}: {tr('Automatic')}</p>
    <p className="hint">{tr('Changes apply to new messages. Saved and pending work stays unchanged.')}</p>
    {fields.map(([feature, label]) => <label className="form-row" key={feature}>
      <span>{label}</span>
      <select className="field" aria-label={label} disabled={busy || !settings?.execution}
        value={settings?.execution?.[feature] ?? ''} onChange={event => void change(feature, event.target.value as ExecutionMode)}>
        {!settings?.execution && <option value="">{tr('Loading…')}</option>}
        <option value="automatic">{tr('Automatic')}</option>
        <option value="on_demand">{tr('On demand')}</option>
      </select>
    </label>)}
    {error != null && <ErrorNotice error={error}>{nativeError(error)}</ErrorNotice>}
    </div>
  </details>
}
