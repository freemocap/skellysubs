import { useState } from 'react'
import { ToolbarIcon, type ToolbarIconName } from '../controls/ToolbarIcon'
import { effortDimensions } from './effort-dimensions'
import type { LanguageTotals } from '../../generated/contracts'
import { useI18n } from '../localization/i18n'

type Column = 'language' | 'xp' | 'conversations' | 'partnerUnderstood' | 'revisionsSent' | 'practiceAttempts' | 'explorations' | 'bot' | 'effort'
type NumberColumn = Exclude<Column, 'language'>

/** The language's short code: the primary subtag of its declared language tag. */
export function languageCode(row: Pick<LanguageTotals, 'languageTag' | 'target'>) {
  return (row.languageTag ?? row.target).split('-')[0].toUpperCase()
}

/** All of a row's effort units together. */
const effortTotal = (row: LanguageTotals) => effortDimensions.reduce((sum, { field }) => sum + row[field], 0)

/** Every language's XP and effort side by side, sortable by any column, with
 * an Effort column (the row's effort units added up) and a Total row (each
 * column added up over every language). `compact` (the top bar's card) names
 * the effort kinds by icon only and leaves out conversations. Pressing a row
 * (or its language's name) calls `onSelect` with that language. */
export function LanguageTable({ rows, active, compact, onSelect }: {
  rows: LanguageTotals[]; active: string | undefined; compact: boolean; onSelect: (target: string) => void
}) {
  const tr = useI18n()
  const [sort, setSort] = useState<{ column: Column; descending: boolean }>({ column: 'xp', descending: true })
  const columns: { id: Column; label: string; icon?: ToolbarIconName; description?: string }[] = [
    { id: 'xp', label: 'XP', icon: 'star' },
    ...(compact ? [] : [{ id: 'conversations' as const, label: 'Conversations', icon: 'chat' as const }]),
    ...effortDimensions.map(({ field, label, icon, description }) => ({ id: field, label, icon, description })),
    { id: 'effort', label: 'Effort', description: 'All effort together' },
  ]
  const count = (row: LanguageTotals, column: NumberColumn) => column === 'effort' ? effortTotal(row) : row[column]
  const value = (row: LanguageTotals, column: Column) => column === 'language' ? row.name : count(row, column)
  const sorted = [...rows].sort((a, b) => {
    const left = value(a, sort.column)
    const right = value(b, sort.column)
    const order = typeof left === 'string' ? left.localeCompare(String(right), tr.browserLocale) : left - Number(right)
    return (sort.descending ? -order : order) || b.xp - a.xp || a.name.localeCompare(b.name, tr.browserLocale)
  })
  const header = (id: Column, label: string, icon?: ToolbarIconName, description?: string) => {
    const current = sort.column === id
    return <th key={id} scope="col" data-column={id} data-unit={unitColumn(id)} aria-sort={current ? (sort.descending ? 'descending' : 'ascending') : 'none'}>
      <button type="button" title={tr(description ?? label)} aria-label={tr(label)} onClick={() => setSort({ column: id, descending: current ? !sort.descending : id !== 'language' })}>
        {icon && <ToolbarIcon name={icon} size={14} />}{(!compact || !icon) && <span>{tr(label)}</span>}
        {/* Numeric headers center independently of their sort arrow. Sorting never changes column width. */}
        <span className="language-table-sort" data-shown={current || undefined} aria-hidden="true">{current && !sort.descending ? '▴' : '▾'}</span>
      </button>
    </th>
  }
  return <table className="language-table" data-compact={compact || undefined}>
    <thead><tr>{header('language', 'Languages')}{columns.map(column => header(column.id, column.label, column.icon, column.description))}</tr></thead>
    <tbody>{sorted.map(row => <tr key={row.target} data-active={row.target === active || undefined} onClick={() => onSelect(row.target)}>
      <th scope="row">
        <button type="button" className="language-table-name" onClick={event => { event.stopPropagation(); onSelect(row.target) }}><Name row={row} /></button>
      </th>
      {columns.map(column => <td key={column.id} data-column={column.id} data-unit={unitColumn(column.id)}>{tr.number(count(row, column.id as NumberColumn))}</td>)}
    </tr>)}</tbody>
    <tfoot><tr>
      <th scope="row">{tr('Total')}</th>
      {columns.map(column => <td key={column.id} data-column={column.id} data-unit={unitColumn(column.id)}>{tr.number(rows.reduce((sum, row) => sum + count(row, column.id as NumberColumn), 0))}</td>)}
    </tr></tfoot>
  </table>
}

/** Effort columns carry their unit so they take its colour; hovering their header says what counts. */
function unitColumn(id: Column) {
  return effortDimensions.some(({ field }) => field === id) ? id : undefined
}

function Name({ row }: { row: LanguageTotals }) {
  return <><span className="language-code" aria-hidden="true">{languageCode(row)}</span><span>{row.name}</span></>
}
