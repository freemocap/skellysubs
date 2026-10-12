import { errorDetails } from '../../platform/diagnostics/error-details'
import { useI18n } from '../localization/i18n'
import { useMemo, useState } from 'react'

/** Metadata has already crossed the native redaction boundary. Render as text. */
export function ResponseDetails({ value }: { value?: unknown }) {
  const tr = useI18n()
  const [open, setOpen] = useState(false)
  const text = useMemo(() => open ? JSON.stringify(value instanceof Error ? errorDetails(value) : value, null, 2) : null, [open, value])
  if (value == null) return null
  return <details className="response-details" onToggle={event => setOpen(event.currentTarget.open)}><summary>{tr('Response details')}</summary>{open && <pre>{text}</pre>}</details>
}
