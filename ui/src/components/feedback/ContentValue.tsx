import { useState } from 'react'
import { useI18n } from '../localization/i18n'

/// A value from the conversation's record, laid out to be read: text as
/// paragraphs, a record as its fields, a list as its items. Identifiers and
/// version stamps (`…Id`, `…Version`, `id`) are plumbing, kept behind a toggle
/// so the content comes first. Compact, long text is clipped and nested
/// records are summarized.

const PLUMBING = /(^id$|Id$|Version$)/
const CLIP = 160

export function isPlumbing(key: string): boolean {
  return PLUMBING.test(key)
}

function Text({ text, compact }: { text: string; compact: boolean }) {
  const shown = compact && text.length > CLIP ? `${text.slice(0, CLIP)}…` : text
  return <p className="content-text" dir="auto">{shown}</p>
}

function Scalar({ value }: { value: number | boolean | null }) {
  return <span className="content-scalar">{String(value)}</span>
}

function List({ items, compact }: { items: unknown[]; compact: boolean }) {
  const tr = useI18n()
  if (!items.length) return <span className="content-scalar">{tr('nothing')}</span>
  const shown = compact ? items.slice(0, 3) : items
  return <ol className="content-list">
    {shown.map((item, index) => <li key={index}><ContentValue value={item} compact={compact} /></li>)}
    {shown.length < items.length && <li className="content-more">{tr('and {value0} more', { value0: items.length - shown.length })}</li>}
  </ol>
}

function Fields({ record, compact }: { record: Record<string, unknown>; compact: boolean }) {
  const tr = useI18n()
  const [ids, setIds] = useState(false)
  const entries = Object.entries(record).filter(([, value]) => value !== undefined)
  const content = entries.filter(([key]) => !isPlumbing(key))
  const plumbing = entries.filter(([key]) => isPlumbing(key))
  return <dl className="content-fields">
    {content.map(([key, value]) => <div key={key} className="content-field">
      <dt>{key}</dt>
      <dd><ContentValue value={value} compact={compact} /></dd>
    </div>)}
    {plumbing.length > 0 && !compact && <div className="content-field is-plumbing">
      <dt><button type="button" className="content-ids" aria-expanded={ids} onClick={() => setIds(open => !open)}>{tr('{value0} identifiers', { value0: plumbing.length })}</button></dt>
      {ids && <dd><dl className="content-fields">{plumbing.map(([key, value]) => <div key={key} className="content-field"><dt>{key}</dt><dd><ContentValue value={value} compact={false} /></dd></div>)}</dl></dd>}
    </div>}
  </dl>
}

export function ContentValue({ value, compact }: { value: unknown; compact: boolean }) {
  if (value === null || typeof value === 'number' || typeof value === 'boolean') return <Scalar value={value} />
  if (typeof value === 'string') return <Text text={value} compact={compact} />
  if (Array.isArray(value)) return <List items={value} compact={compact} />
  if (typeof value === 'object') return <Fields record={value as Record<string, unknown>} compact={compact} />
  throw new Error(`A record value has an unsupported type: ${typeof value}`)
}
