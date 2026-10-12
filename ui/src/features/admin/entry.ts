import { errorDetails } from '../../platform/diagnostics/error-details'
import { ReportTable, badge, count, dateTime, duration, metadata, number, timestamp } from './report-table'
/** Standalone hosted admin surface. All access decisions remain on the server. */
type Row = Record<string, unknown>
type Usage = { day: string; present: boolean; micros: number; tokens: number; requests: number; micros_credit: number }
type User = { id: string; email_label?: string | null; created_at?: string; last_seen?: string; last_inference_at?: string | null; effective_limit_micros: number; daily_limit_micros: number | null; admin_revision: number; token_version: number; usage: Usage; usage_24_hours_micros: number | null; usage_7_days_micros: number | null; usage_90_days_micros: number; admission: Record<string, number> }
type Overview = { usage_limits_enforced?: boolean; environment: string; administrator: string; generated_at: string; revision: string; policy: Record<string, number>; environment_defaults: Record<string, number>; spending_paused: boolean; account_count: number; global_admission: Record<string, number>; users: User[]; next_cursor: string | null; global_usage: Usage[]; scope: string }
type Logs = { entries: Row[]; next_page_token?: string; scope: string; since: string }
type Command = { operation_id: string; expected_revision: number; action: string; target: string; values: Row }
const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T
const el = <K extends keyof HTMLElementTagNameMap>(tag: K, text?: string) => { const node = document.createElement(tag); if (text !== undefined) node.textContent = text; return node }
const money = (value: number) => `$${(value / 1e6).toFixed(6)}`
const text = (value: unknown) => value === null || value === undefined ? '—' : typeof value === 'object' ? JSON.stringify(value) : String(value)
let overview: Overview | undefined
let cursor = ''
let command: Command | undefined
let logRows: Row[] = []
let nextLogPage: string | undefined
let activeLogQuery = ''
let logSince = ''
let busy = false
let limitsEnforced = true
const policyLabels: Record<string, string> = {
  max_users: 'Maximum registered accounts', free_daily_micros: 'Default daily allowance (USD)',
  extended_daily_micros: 'Extended allowance preset (USD)', global_daily_micros: 'Shared daily allowance (USD)',
  account_requests: 'Inference requests / account / day', global_requests: 'Inference requests / service / day',
  diagnostics_requests: 'Account checks / account / day', global_diagnostics: 'Account checks / service / day',
}
function maskedAccountId(id: string) {
  const separator = id.indexOf(':')
  const prefix = separator < 0 ? '' : id.slice(0, separator + 1)
  const subject = id.slice(separator + 1)
  return prefix + (subject.length > 9 ? `${subject.slice(0, 3)}…${subject.slice(-6)}` : '•••')
}
function accountIdentity(user: User, field: 'email' | 'user ID' = 'email') {
  const masked = field === 'email' ? user.email_label ?? 'Email unavailable' : maskedAccountId(user.id)
  const node = el('div'); node.className = 'account-identity'
  const hint = el('small', `Click to reveal ${field}`)
  const control = button(masked, () => {
    if (control.dataset.revealed) {
      control.textContent = masked; delete control.dataset.revealed
      hint.textContent = `Click to reveal ${field}`
      control.setAttribute('aria-expanded', 'false'); void renderLive(); return
    }
    void run(async () => {
      if (field === 'email') {
        const result = await api<{ identity: { email: string | null } }>(`/admin/api/users/${encodeURIComponent(user.id)}?days=1`)
        control.textContent = result.identity.email ?? 'Email unavailable'
      } else control.textContent = user.id
      hint.textContent = `Click to hide ${field}`
      control.dataset.revealed = 'true'; control.setAttribute('aria-expanded', 'true')
    })
  })
  control.className = 'identity-toggle'; control.setAttribute('aria-expanded', 'false')
  control.title = `Click to reveal ${field}; click again to hide`
  node.append(control, hint)
  return node
}
const accountTable = new ReportTable<User>([
  { key: 'identity', label: 'Account', required: true, value: u => u.email_label ?? u.id, render: u => accountIdentity(u) },
  { key: 'id', label: 'User ID', value: u => u.id, render: u => accountIdentity(u, 'user ID') },
  { key: 'inference', label: 'Last inference request (UTC)', headerLines: ['Last inference', 'request (UTC)'], value: u => timestamp(u.last_inference_at), render: u => dateTime(u.last_inference_at) },
  { key: 'seen', label: 'Last sign-in (UTC)', headerLines: ['Last sign-in', '(UTC)'], value: u => timestamp(u.last_seen), render: u => dateTime(u.last_seen) },
  { key: 'used', label: 'Allowance used · UTC day (USD)', headerLines: ['Allowance used', 'UTC day (USD)'], numeric: true, value: u => u.usage.micros, render: u => money(u.usage.micros) },
  { key: 'rolling-day', label: 'Allowance used · past 24 hours (USD)', headerLines: ['Allowance used', 'Past 24h (USD)'], numeric: true, value: u => u.usage_24_hours_micros, render: u => u.usage_24_hours_micros == null ? 'Unknown' : money(u.usage_24_hours_micros) },
  { key: 'rolling-week', label: 'Allowance used · past 7 days (USD)', headerLines: ['Allowance used', 'Past 7 days (USD)'], numeric: true, value: u => u.usage_7_days_micros, render: u => u.usage_7_days_micros == null ? 'Unknown' : money(u.usage_7_days_micros) },
  { key: 'history', label: 'Allowance used · 90 days (USD)', headerLines: ['Allowance used', '90 UTC days (USD)'], numeric: true, value: u => u.usage_90_days_micros, render: u => money(u.usage_90_days_micros) },
  { key: 'requests', label: 'Inference admissions today', headerLines: ['Inference admissions', 'UTC day'], numeric: true, value: u => number(u.admission.requests), render: u => count(u.admission.requests) },
  { key: 'limit', label: 'Daily allowance (USD)', headerLines: ['Daily allowance', '(USD)'], numeric: true, value: u => limitsEnforced ? u.effective_limit_micros : null, render: u => limitsEnforced ? money(u.effective_limit_micros) : 'Disabled' },
  { key: 'source', label: 'Limit source', value: u => u.daily_limit_micros == null ? 'Default' : 'Custom', render: u => badge(u.daily_limit_micros == null ? 'Default' : 'Custom') },
  { key: 'checks', label: 'Account checks today', numeric: true, hidden: true, value: u => number(u.admission.diagnostics_requests), render: u => count(u.admission.diagnostics_requests) },
  { key: 'check-credit', label: 'Restored account checks', numeric: true, hidden: true, value: u => number(u.admission.diagnostics_requests_credit), render: u => count(u.admission.diagnostics_requests_credit) },
  { key: 'request-credit', label: 'Restored inference requests', numeric: true, hidden: true, value: u => number(u.admission.requests_credit), render: u => count(u.admission.requests_credit) },
  { key: 'allowance-credit', label: 'Restored allowance (USD)', numeric: true, hidden: true, value: u => u.usage.micros_credit, render: u => money(u.usage.micros_credit) },
  { key: 'tokens', label: 'Tokens today', numeric: true, hidden: true, value: u => u.usage.tokens, render: u => count(u.usage.tokens) },
  { key: 'created', label: 'Registered (UTC)', hidden: true, value: u => timestamp(u.created_at), render: u => dateTime(u.created_at) },
  { key: 'version', label: 'Session version', numeric: true, hidden: true, value: u => u.token_version, render: u => count(u.token_version) },
  { key: 'inspect', label: 'Details', required: true, render: u => button('Inspect account', () => void run(() => inspectUser(u))) },
], { label: 'Accounts', scope: 'Sorting and search apply to this loaded page (up to 25 accounts). Dates are UTC. The allowance day resets at 00:00 UTC; past 24h and 7 days roll with report time. Unknown means incomplete reservation coverage (10,000-record read limit or missing amounts). Admissions include restored credit; usage includes holds and estimates.', sort: 'inference', descending: true,
  search: { label: 'Search masked email or account ID', text: u => `${u.email_label ?? ''} ${u.id}` } })
$('users').replaceChildren(accountTable.node)
const eventTable = new ReportTable<Row>([
  { key: 'time', label: 'Time (UTC)', required: true, value: r => timestamp(r.timestamp), render: r => dateTime(r.timestamp) },
  { key: 'event', label: 'Event', required: true, value: r => text(r.event), render: r => badge(text(r.event).replaceAll('_', ' '), String(r.event).endsWith('failed') || r.event === 'provider_error_response' ? 'report-danger' : '') },
  { key: 'duration', label: 'Duration', numeric: true, value: r => number(r.duration_ms), render: r => duration(r.duration_ms) },
  { key: 'route', label: 'Route', value: r => typeof r.route === 'string' ? r.route : null, render: r => r.route },
  { key: 'status', label: 'HTTP status', numeric: true, value: r => number(r.status), render: r => badge(r.status, Number(r.status) >= 400 ? 'report-danger' : '') },
  { key: 'provider', label: 'Provider', hidden: true, value: r => typeof r.provider === 'string' ? r.provider : null, render: r => r.provider },
  { key: 'method', label: 'Method', hidden: true, value: r => typeof r.method === 'string' ? r.method : null, render: r => r.method },
  { key: 'code', label: 'Error code', value: r => typeof r.code === 'string' ? r.code : null, render: r => r.code },
  { key: 'bytes', label: 'Bytes', numeric: true, hidden: true, value: r => number(r.bytes), render: r => count(r.bytes) },
  { key: 'chunks', label: 'Chunks', numeric: true, hidden: true, value: r => number(r.chunks), render: r => count(r.chunks) },
  { key: 'tokens', label: 'Tokens', numeric: true, hidden: true, value: r => number(r.tokens), render: r => count(r.tokens) },
  { key: 'request', label: 'Request ID', hidden: true, value: r => typeof r.request_id === 'string' ? r.request_id : null, render: r => r.request_id },
  { key: 'metadata', label: 'Metadata', required: true, render: metadata },
], { label: 'Request events', scope: 'Sorts loaded events only. Duration measures the named phase: HTTP headers, full request, provider, reservation or settlement. Missing duration is unknown. Search filters this table; the arrival chart uses all loaded events.', sort: 'time', descending: true,
  search: { label: 'Search event, route, code or request ID', text: r => ['event', 'route', 'code', 'request_id', 'provider'].map(k => text(r[k])).join(' ') } })
$('logs').replaceChildren(eventTable.node)
const timelineTable = new ReportTable<Point>([
  { key: 'time', label: 'Interval start (UTC)', required: true, value: p => timestamp(p.time), render: p => dateTime(p.time) },
  { key: 'record', label: 'Record', value: p => p.present ? 'Present' : 'Missing', render: p => p.present ? 'Present' : 'No record' },
  { key: 'usage', label: 'Allowance used (USD)', numeric: true, value: p => p.present ? p.micros : null, render: p => p.present ? money(p.micros) : '—' },
  { key: 'tokens', label: 'Tokens', numeric: true, value: p => p.present ? p.tokens : null, render: p => p.present ? count(p.tokens) : '—' },
  { key: 'requests', label: 'Requests', numeric: true, value: p => p.present ? p.requests : null, render: p => p.present ? count(p.requests) : '—' },
], { label: 'Allowance intervals', scope: 'Sorts intervals in the selected chart window. Missing records are unknown.', sort: 'time', descending: true })
$('usage-table').replaceChildren(timelineTable.node)
const auditTable = new ReportTable<Row>([
  { key: 'time', label: 'Time (UTC)', required: true, value: r => timestamp(r.created_at), render: r => dateTime(r.created_at) },
  { key: 'action', label: 'Action', value: r => text(r.action), render: r => text(r.action).replaceAll('_', ' ') },
  { key: 'target', label: 'Target', value: r => text(r.target), render: r => r.target },
  { key: 'before', label: 'Before', render: r => metadata(r.before) },
  { key: 'after', label: 'After', render: r => metadata(r.after) },
  { key: 'actor', label: 'Administrator', hidden: true, value: r => text(r.actor), render: r => r.actor },
], { label: 'Administrative changes', scope: 'Sorts the latest 100 loaded changes.', sort: 'time', descending: true })
$('audit-table').replaceChildren(auditTable.node)
async function api<T>(path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, { credentials: 'same-origin', cache: 'no-store',
    ...(body === undefined ? {} : { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Admin-Action': '1' }, body: JSON.stringify(body) }) })
  if (!response.ok) {
    const failure = await response.json().catch(() => ({ detail: `HTTP ${response.status}` }))
    throw new Error(`${failure.detail ?? 'Request failed'}\nHTTP ${response.status}${failure.request_id ? ` · Request ${failure.request_id}` : ''}${failure.diagnostics ? `\n${JSON.stringify(failure.diagnostics, null, 2)}` : ''}${response.status === 401 ? '\nSign in again at /admin/login.' : ''}`)
  }
  return response.status === 204 ? undefined as T : await response.json() as T
}
async function run(work: () => Promise<void>) {
  if (busy) return false
  busy = true
  $('error').hidden = true; $('change-error').hidden = true
  document.body.setAttribute('aria-busy', 'true')
  const buttons = [...document.querySelectorAll<HTMLButtonElement>('button')].map(button => ({ button, disabled: button.disabled }))
  buttons.forEach(({ button }) => button.disabled = true)
  try { await work(); return true } catch (error) { const notice = $<HTMLDialogElement>('confirm').open ? $('change-error') : $('error'); notice.textContent = error instanceof Error ? error.message : String(error); notice.hidden = false; notice.scrollIntoView({ block: 'nearest' }); return false }
  finally {
    busy = false; document.body.removeAttribute('aria-busy')
    buttons.forEach(({ button, disabled }) => button.disabled = disabled)
    $<HTMLButtonElement>('next-users').disabled = !overview?.next_cursor
    void renderLive()
  }
}
function table(headers: string[], rows: (unknown | HTMLElement)[][]) {
  const node = el('table'), head = el('thead'), heading = el('tr'), body = el('tbody')
  headers.forEach(value => heading.append(el('th', value))); head.append(heading)
  rows.forEach(values => { const row = el('tr'); values.forEach(value => { const cell = el('td'); if (value instanceof HTMLElement) cell.append(value); else cell.textContent = text(value); row.append(cell) }); body.append(row) })
  node.append(head, body)
  if (!rows.length) { const row = el('tr'), cell = el('td', 'No records in this window.'); cell.colSpan = headers.length; row.append(cell); body.append(row) }
  return node
}
function button(label: string, action: () => void) { const node = el('button', label); node.type = 'button'; node.onclick = action; return node }
function usageView(target: HTMLElement, rows: Usage[]) {
  renderChart(target, rows.map(row => ({ ...row, time: `${row.day}T00:00:00Z` })))
}
type Point = { time: string; present: boolean; micros: number; tokens: number; requests: number }
function renderChart(target: HTMLElement, points: Point[]) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  svg.setAttribute('viewBox', '0 0 1000 260'); svg.setAttribute('role', 'img'); svg.setAttribute('aria-label', 'Allowance used per UTC interval in US dollars')
  svg.classList.add('usage-line')
  const draw = (tag: string, attrs: Record<string, string>, text?: string) => {
    const node = document.createElementNS(svg.namespaceURI, tag)
    Object.entries(attrs).forEach(([key, value]) => node.setAttribute(key, value))
    if (text) node.textContent = text
    svg.append(node); return node
  }
  const max = Math.max(1, ...points.map(p => p.micros)), x = (i: number) => 100 + i / Math.max(1, points.length - 1) * 870
  for (let i = 0; i <= 4; i++) {
    const y = 20 + i * 45
    draw('line', { x1: '100', y1: String(y), x2: '970', y2: String(y), class: 'chart-grid' })
    draw('text', { x: '90', y: String(y + 4), 'text-anchor': 'end' }, money(max * (1 - i / 4)))
  }
  let path = '', connected = false
  points.forEach((p, i) => {
    if (!p.present) { connected = false; return }
    const y = 200 - p.micros / max * 180
    path += `${connected ? 'L' : 'M'}${x(i)},${y} `; connected = true
    const dot = draw('circle', { cx: String(x(i)), cy: String(y), r: '4', class: 'chart-point', tabindex: '0' })
    const title = document.createElementNS(svg.namespaceURI, 'title')
    title.textContent = `${p.time} · ${money(p.micros)} · ${p.requests} requests · ${p.tokens} tokens`; dot.append(title)
  })
  draw('path', { d: path, class: 'chart-line' })
  const ticks = [...new Set(Array.from({ length: Math.min(5, points.length) }, (_, i) => Math.round(i * (points.length - 1) / Math.max(1, Math.min(5, points.length) - 1))))]
  ticks.forEach(i => {
    const date = new Date(points[i].time)
    draw('text', { x: String(x(i)), y: '228', 'text-anchor': i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle' }, date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' }))
    draw('text', { x: String(x(i)), y: '248', 'text-anchor': i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle' }, date.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', timeZone: 'UTC' }) + ' UTC')
  })
  target.replaceChildren(svg)
  if (!points.some(p => p.present)) target.append(el('p', 'No recorded usage in this window. Try a wider range or daily intervals for older records.'))
}
type Timeline = { points: Point[]; scope: string }
async function loadTimeline(pushed?: Timeline) {
  const span = $<HTMLSelectElement>('chart-range').value, interval = $<HTMLSelectElement>('chart-interval').value
  const data = pushed ?? await api<Timeline>(`/admin/api/timeline?span=${span}&interval=${interval}`)
  renderChart($('usage-chart'), data.points)
  $('chart-scope').textContent = data.scope
  timelineTable.update(data.points)
}
function moneyInput(input: HTMLInputElement) {
  input.type = 'text'; input.inputMode = 'decimal'; input.required = true
  const group = el('span'); group.className = 'money-input'
  const adjust = (delta: number) => { const amount = parseNumber(input, true); input.value = String(Math.max(0, amount + delta * 1e6) / 1e6) }
  group.append(button('− $1', () => { try { adjust(-1) } catch (e) { $('error').textContent = String(e); $('error').hidden = false } }), input,
    button('+ $1', () => { try { adjust(1) } catch (e) { $('error').textContent = String(e); $('error').hidden = false } }))
  return group
}
function usageTable(rows: Usage[]) {
  const report = new ReportTable<Usage>([
    { key: 'day', label: 'UTC day', required: true, value: r => r.day, render: r => r.day },
    { key: 'record', label: 'Record', value: r => r.present ? 'Present' : 'Missing', render: r => r.present ? 'Present' : 'Missing' },
    { key: 'usage', label: 'Used (USD)', numeric: true, value: r => r.present ? r.micros : null, render: r => r.present ? money(r.micros) : '—' },
    { key: 'tokens', label: 'Tokens', numeric: true, value: r => r.present ? r.tokens : null, render: r => r.present ? count(r.tokens) : '—' },
    { key: 'requests', label: 'Requests', numeric: true, value: r => r.present ? r.requests : null, render: r => r.present ? count(r.requests) : '—' },
    { key: 'credit', label: 'Restored allowance (USD)', numeric: true, value: r => r.present ? r.micros_credit : null, render: r => r.present ? money(r.micros_credit) : '—' },
  ], { label: 'Account daily usage', scope: 'Sorts the selected account history. Missing records are unknown.', sort: 'day', descending: true })
  report.update(rows); return report.node
}
function review(action: string, target: string, revision: number, values: Row, before: unknown, effect: string) {
  command = { operation_id: crypto.randomUUID(), expected_revision: revision, action, target, values }
  const labels: Record<string, string> = { user_limit: 'Change daily allowance', reset_diagnostics: 'Restore account checks', reset_requests: 'Restore inference requests', reset_allowance: 'Restore spending allowance', revoke_sessions: 'Revoke sessions', policy: 'Change service limits', clear_installations: 'Clear installation history', remove_installations: 'Remove selected installations' }
  const display = (key: string, value: unknown) => value === null || value === undefined ? 'Use service default' : key.endsWith('_micros') ? money(Number(value)) : String(value)
  const previous = (before && typeof before === 'object' ? before : {}) as Row
  const changes = Object.entries(values).map(([key, value]) => `${policyLabels[key] ?? (key === 'daily_limit_micros' ? 'Daily allowance' : key)}: ${display(key, previous[key])} → ${display(key, value)}`)
  $('change-preview').textContent = `${labels[action] ?? action}\n${target}${changes.length ? '\n\n' + changes.join('\n') : ''}`
  $('change-effect').textContent = effect
  $<HTMLDialogElement>('confirm').showModal()
}
function parseNumber(input: HTMLInputElement, dollars: boolean) {
  if (!input.value.trim() || !input.checkValidity()) throw new Error(`Invalid value: ${input.labels?.[0]?.textContent ?? input.id}`)
  if (dollars && !/^\d+(?:\.\d{1,6})?$/.test(input.value.trim())) throw new Error('Enter a USD amount such as 5 or 0.50, with at most six decimal places.')
  if (input.max && Number(input.value) > Number(input.max)) throw new Error(`Maximum allowed value is ${dollars ? '$' : ''}${input.max}.`)
  const scaled = Number(input.value) * (dollars ? 1e6 : 1)
  const number = Math.round(scaled)
  if (!Number.isSafeInteger(number) || number < 0 || Math.abs(number - scaled) > 0.000001) throw new Error('Use a nonnegative whole number or at most six decimal places for USD.')
  return number
}
async function loadOverview(live = false, pushed?: Overview, pushedTimeline?: Timeline) {
  const days = $<HTMLSelectElement>('days').value
  const data = pushed ?? await api<Overview>(`/admin/api/overview?days=${days}&after=${encodeURIComponent(cursor)}`)
  if (!live) overview = data
  const today = data.global_usage.at(-1)!
  $('environment').textContent = data.environment
  $('administrator').textContent = data.administrator
  const limited = data.usage_limits_enforced !== false
  limitsEnforced = limited
  $('status').textContent = `Snapshot ${new Date(data.generated_at).toLocaleString()} · ${limited ? 'UTC daily limits reset at 00:00.' : 'Daily usage limits disabled; usage is still recorded.'}`
  $('revision').textContent = `Revision: ${data.revision}`
  $('summary').replaceChildren(...[
    ['Registered accounts', `${data.account_count} / ${data.policy.max_users}`],
    ['Spending', data.spending_paused ? 'Paused' : 'Admission enabled'],
    ['Allowance used today', money(today.micros)], ['Shared daily limit', limited ? money(data.policy.global_daily_micros) : 'Disabled'],
    ['Inference admissions', limited ? `${data.global_admission.account_requests} / ${data.policy.global_requests}` : String(data.global_admission.account_requests)],
    ['Account checks', limited ? `${data.global_admission.diagnostics_requests} / ${data.policy.global_diagnostics}` : String(data.global_admission.diagnostics_requests)],
  ].map(([label, value]) => { const card = el('div'); card.append(el('span', label), el('strong', value)); return card }))
  await loadTimeline(pushedTimeline)
  if (!live || !document.querySelector('#users [data-revealed]')) accountTable.update(data.users)
  $<HTMLButtonElement>('next-users').disabled = !data.next_cursor
  if (live) return // Keep editable values and their revision even if typing starts during this fetch.
  $('user-detail').hidden = true
  const fields = $('policy-fields'); fields.replaceChildren()
  if (!limited) fields.append(el('p', 'Daily limits below apply only when the local server starts with --enforce-usage-limits.'))
  for (const [key, label] of Object.entries(policyLabels)) {
    const wrapper = el('label', label), input = el('input'), hint = el('small')
    input.id = `policy-${key}`; wrapper.htmlFor = input.id; input.type = 'number'; input.min = '0'; input.required = true
    const dollars = key.endsWith('_micros'); input.step = '1'
    input.value = String(data.policy[key] / (dollars ? 1e6 : 1))
    if (dollars) input.max = key === 'global_daily_micros' ? '10000' : '1000'
    hint.textContent = `Environment default: ${dollars ? money(data.environment_defaults[key]) : data.environment_defaults[key]}${key === 'extended_daily_micros' ? ' · Applied explicitly to individual accounts; existing exceptions keep their values.' : ''}`
    wrapper.append(dollars ? moneyInput(input) : input, hint); fields.append(wrapper)
  }
}
async function inspectUser(user: User) {
  const result = await api<{ user: User; identity: { email: string | null; name: string | null }; usage: Usage[]; devices: Row[]; installation_policy: { maximum_retained: number; inactivity_days: number }; reservations: Row[]; devices_truncated: boolean; reservations_truncated: boolean }>(`/admin/api/users/${encodeURIComponent(user.id)}?days=${$<HTMLSelectElement>('days').value}`)
  user = result.user
  const label = user.email_label ?? 'Account details'
  const pane = $('user-detail'); pane.hidden = false; pane.replaceChildren(el('h3', label), button('Close account details', () => { pane.hidden = true; pane.replaceChildren() }))
  const identity = el('details'); identity.append(el('summary', 'Identity details'), table(['User ID', 'Email', 'Name'], [[user.id, result.identity.email, result.identity.name]])); pane.append(identity)
  const chart = el('div'); usageView(chart, result.usage); pane.append(chart)
  const history = el('details'); history.append(el('summary', 'Daily numeric table'), usageTable(result.usage)); pane.append(history)
  const controls = el('div'); controls.className = 'filters'
  const limitLabel = el('label', 'Custom daily allowance (USD)'), input = el('input'); input.type = 'number'; input.min = '0'; input.id = 'custom-daily-allowance'; limitLabel.htmlFor = input.id; input.step = 'any'; input.max = '1000'; input.value = String(user.effective_limit_micros / 1e6); limitLabel.append(moneyInput(input)); controls.append(limitLabel)
  const limitChange = (limit: number | null) => review('user_limit', user.id, user.admin_revision, { daily_limit_micros: limit }, { daily_limit_micros: user.daily_limit_micros }, 'Applies to subsequent requests. Shared spending and request limits still apply.')
  controls.append(button('Set custom limit', () => { try { limitChange(parseNumber(input, true)) } catch (error) { $('error').textContent = String(error); $('error').hidden = false } }),
    button('Use default', () => limitChange(null)), button('Use extended preset', () => limitChange(overview!.policy.extended_daily_micros)))
  pane.append(controls)
  const resets = el('div'); resets.className = 'filters'
  for (const [action, label, effect] of [
    ['reset_diagnostics', 'Restore account checks', 'Restores today’s personal account-check allowance. Shared request counts remain unchanged.'],
    ['reset_requests', 'Restore inference requests', 'Restores today’s personal inference-request allowance. Shared request counts remain unchanged.'],
    ['reset_allowance', 'Restore spending allowance', 'Grants allowance equal to currently recorded usage, including pending holds. Usage, shared totals, reservations and later settlements remain intact. This can permit additional spending today.'],
    ['revoke_sessions', 'Revoke sessions', 'Invalidates existing app and admin sessions for this account. The user can sign in again; this does not ban the account.'],
  ]) resets.append(button(label, () => review(action, user.id, user.admin_revision, {}, { token_version: user.token_version, today: result.usage.at(-1) }, effect)))
  const reservations = new ReportTable<Row>([
    { key: 'created', label: 'Created (UTC)', required: true, value: r => timestamp(r.created_at), render: r => dateTime(r.created_at) },
    { key: 'state', label: 'State', value: r => text(r.status), render: r => badge(r.status) },
    { key: 'elapsed', label: 'Created → updated', numeric: true, value: r => reservationDuration(r), render: r => duration(reservationDuration(r)) },
    { key: 'reserved', label: 'Reserved (USD)', numeric: true, value: r => number(r.reserved_micros), render: r => money(Number(r.reserved_micros)) },
    { key: 'recorded', label: 'Recorded (USD)', numeric: true, value: r => number(r.actual_micros), render: r => r.actual_micros == null ? 'Unknown' : money(Number(r.actual_micros)) },
    { key: 'basis', label: 'Cost basis', value: r => typeof r.cost_basis === 'string' ? r.cost_basis : null, render: r => r.cost_basis ?? 'Pending / unknown' },
    { key: 'tokens', label: 'Tokens', hidden: true, numeric: true, value: r => number(r.tokens), render: r => count(r.tokens) },
    { key: 'provider', label: 'Provider receipt', hidden: true, value: r => typeof r.provider_id === 'string' ? r.provider_id : null, render: r => r.provider_id },
    { key: 'metadata', label: 'Metadata', required: true, render: metadata },
  ], { label: 'Account reservations', scope: 'Latest 100 reservations. Created → updated is recorded bookkeeping time, not provider execution duration. Unknown cost is not zero.', sort: 'created', descending: true })
  reservations.update(result.reservations)
  const selected = new Set<string>(), installationControls = el('div'); installationControls.className = 'filters'
  const effect = 'Removes installation metadata only. Accounts, usage, allowances and sessions are preserved. An installation is recorded again on its next account check.'
  const remove = button('Remove selected', () => review('remove_installations', user.id, user.admin_revision, { installation_ids: [...selected] }, {}, `${selected.size} selected. ${effect}`))
  remove.disabled = true
  const clear = button('Clear installation history', () => review('clear_installations', user.id, user.admin_revision, {}, {}, `Clears all retained installations for this account at the time of confirmation. ${effect}`))
  clear.disabled = result.devices.length === 0
  installationControls.append(remove, clear)
  const installations = table(['Select', 'Platform', 'App version', 'First seen (UTC)', 'Last seen (UTC)'], result.devices.map(row => {
    const input = el('input'); input.type = 'checkbox'; input.setAttribute('aria-label', `Select installation ${row.id}`)
    input.onchange = () => { if (input.checked) selected.add(String(row.id)); else selected.delete(String(row.id)); remove.disabled = selected.size === 0 }
    return [input, row.platform, row.app_version, dateTime(row.first_seen), dateTime(row.last_seen)]
  }))
  pane.append(resets, el('h4', `Retained installations: ${result.devices.length}${result.devices_truncated ? '+' : ''}`),
    el('p', `Keeps the ${result.installation_policy.maximum_retained} most recently seen installations. Older entries are replaced automatically. Entries expire after ${result.installation_policy.inactivity_days} days without an account check; background deletion may lag. This history does not limit sign-in.`), installationControls, installations,
    el('h4', `Latest reservations${result.reservations_truncated ? ' · limited to 100' : ''}`), reservations.node)
  pane.scrollIntoView({ block: 'start', behavior: 'smooth' })
}
function reservationDuration(row: Row) {
  const start = timestamp(row.created_at), end = timestamp(row.updated_at)
  return start == null || end == null || end < start ? null : end - start
}
function renderLogs() {
  eventTable.update(logRows)
  const counts = new Map<string, number>(), unique = new Set<string>()
  for (const row of logRows) if (row.event === 'request_started' && typeof row.timestamp === 'string' && typeof row.request_id === 'string' && !unique.has(row.request_id)) {
    const date = new Date(row.timestamp); if (!Number.isFinite(date.getTime())) continue
    unique.add(row.request_id); const key = date.toISOString().slice(0, 13); counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  const days = [...new Set([...counts.keys()].map(key => key.slice(0, 10)))].sort(), max = Math.max(1, ...counts.values())
  const heatmap = table(['UTC day', ...Array.from({ length: 24 }, (_, hour) => String(hour).padStart(2, '0'))], days.map(day => [day, ...Array.from({ length: 24 }, (_, hour) => {
    const count = counts.get(`${day}T${String(hour).padStart(2, '0')}`) ?? 0, cell = el('span', String(count)); cell.className = `heat heat-${Math.ceil(count / max * 4)}`; cell.title = `${day} ${hour}:00 UTC: ${count} arrivals in loaded events`; return cell
  })]))
  $('heatmap').replaceChildren(heatmap)
}
async function loadLogs(more = false) {
  if (!more) { const params = new URLSearchParams({ hours: $<HTMLSelectElement>('hours').value, errors: String($<HTMLInputElement>('errors-only').checked), request_id: $<HTMLInputElement>('request-id').value.trim() }); activeLogQuery = params.toString() }
  const data = await api<Logs>(`/admin/api/logs?${activeLogQuery}${more && nextLogPage ? `&page_token=${encodeURIComponent(nextLogPage)}&since=${encodeURIComponent(logSince)}` : ''}`)
  logRows = more ? [...logRows, ...data.entries] : data.entries; nextLogPage = data.next_page_token; logSince = data.since
  $('log-scope').textContent = `${data.scope} Loaded ${logRows.length} events. ${nextLogPage ? 'More events are available; the heat map is incomplete.' : 'No further pages reported.'}`
  $('more-logs').hidden = !nextLogPage; renderLogs()
}
let liveSocket: WebSocket | undefined
let latestLive: { type: string; overview: Overview; timeline: Timeline; logs: Logs; selection: Row } | undefined
function stopLive(message = 'Live is off.') {
  const socket = liveSocket; liveSocket = undefined; latestLive = undefined
  socket?.close()
  $<HTMLInputElement>('live').checked = false
  $('live-status').textContent = message
}
function liveSelection() {
  return { days: Number($<HTMLSelectElement>('days').value), after: cursor,
    span: $<HTMLSelectElement>('chart-range').value, interval: $<HTMLSelectElement>('chart-interval').value,
    hours: Number($<HTMLSelectElement>('hours').value), errors: $<HTMLInputElement>('errors-only').checked,
    request_id: $<HTMLInputElement>('request-id').value.trim() }
}
function subscribeLive() {
  if (liveSocket?.readyState !== WebSocket.OPEN) return
  liveSocket.send(JSON.stringify(liveSelection()))
}
async function renderLive() {
  if (!latestLive || busy || document.hidden || !liveSocket) return
  const packet = latestLive
  if (JSON.stringify(packet.selection) !== JSON.stringify(liveSelection())) return
  await loadOverview(true, packet.overview, packet.timeline)
  if (!document.querySelector('#logs .metadata-details[open]')) {
    const data = packet.logs
    logRows = data.entries; nextLogPage = undefined; logSince = data.since
    $('log-scope').textContent = data.scope
    $('more-logs').hidden = true; renderLogs()
  }
  $('live-status').textContent = `Live · connected · updated ${new Date().toLocaleTimeString()}.`
}
function liveFailure(stage: string, error: unknown) {
  const details = errorDetails(error)
  $('error').textContent = `${stage}\n${JSON.stringify(details, null, 2)}`
  $('error').hidden = false
  stopLive(stage)
}
function startLive() {
  liveSocket?.close(); latestLive = undefined
  const url = new URL('/admin/live', location.href); url.protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const socket = new WebSocket(url); liveSocket = socket
  $('live-status').textContent = 'Live: connecting…'
  socket.onopen = () => { if (liveSocket === socket) { $('live-status').textContent = 'Live · connected'; subscribeLive() } }
  socket.onmessage = event => {
    if (liveSocket !== socket) return
    try {
      const packet = JSON.parse(event.data)
      if (packet.type === 'heartbeat') return
      if (packet.type === 'error') { $('error').textContent = `${packet.detail}${packet.diagnostics ? '\n' + JSON.stringify(packet.diagnostics, null, 2) : ''}`; $('error').hidden = false; stopLive(`Live stopped: ${packet.detail}`); return }
      if (packet.type !== 'snapshot' || !packet.overview || !packet.timeline || !packet.logs) throw new Error('Invalid live update')
      latestLive = packet; void renderLive().catch(error => liveFailure('Live update rendering failed', error))
    } catch (error) { liveFailure('Live message decoding failed', error) }
  }
  socket.onclose = () => { if (liveSocket === socket) stopLive('Live disconnected. Enable Live to reconnect.') }
  socket.onerror = () => { if (liveSocket === socket) stopLive('Live connection failed. Check the server and your admin session, then enable Live again.') }
}
$('live').onchange = () => { if ($<HTMLInputElement>('live').checked) startLive(); else stopLive() }
document.addEventListener('visibilitychange', () => { void renderLive().catch(error => liveFailure('Live update rendering failed', error)) })
$('logs').addEventListener('toggle', () => { void renderLive() }, true)
window.addEventListener('pagehide', () => stopLive())
$('refresh').onclick = () => void run(loadOverview)
$('days').onchange = () => { if (liveSocket) subscribeLive(); else void run(loadOverview) }
const durations: Record<string, number> = { '1m': 60, '5m': 300, '10m': 600, '1h': 3600, '12h': 43200, '1d': 86400, '1w': 604800, '1mo': 2592000, '3mo': 7776000 }
function chartSelection(changed: 'range' | 'interval') {
  const range = $<HTMLSelectElement>('chart-range'), interval = $<HTMLSelectElement>('chart-interval')
  const resolution = durations[interval.value] < 3600 ? 60 : durations[interval.value] < 86400 ? 3600 : 86400
  if (durations[range.value] / resolution > 1440) {
    if (changed === 'interval') range.value = resolution === 60 ? '1d' : '1mo'
    else interval.value = durations[range.value] > 3600 * 1440 ? '1d' : '1h'
  }
  if (durations[interval.value] > durations[range.value]) {
    if (changed === 'interval') range.value = interval.value
    else interval.value = range.value
  }
  if (liveSocket) subscribeLive(); else void run(() => loadTimeline())
}
$('chart-range').onchange = () => chartSelection('range')
$('chart-interval').onchange = () => chartSelection('interval')
$('first-users').onclick = () => { cursor = ''; if (liveSocket) subscribeLive(); else void run(loadOverview) }
$('next-users').onclick = () => { cursor = (liveSocket && latestLive ? latestLive.overview.next_cursor : overview?.next_cursor) ?? ''; if (liveSocket) subscribeLive(); else void run(loadOverview) }
$('load-logs').onclick = () => { stopLive('Live is off while loading historical logs.'); void run(() => loadLogs()) }
for (const id of ['hours', 'errors-only', 'request-id']) $(id).onchange = () => { if (liveSocket) subscribeLive() }
$('more-logs').onclick = () => { stopLive('Live is off while browsing older log pages.'); void run(() => loadLogs(true)) }
$('load-audit').onclick = () => void run(async () => { auditTable.update(await api<Row[]>('/admin/api/audit')) })
$('logout').onclick = () => void run(async () => { stopLive(); await api('/admin/logout', {}); location.assign('/admin') })
$('policy-form').onsubmit = event => { event.preventDefault(); if (!overview) return; try {
  const values: Row = {}, before: Row = {}
  for (const key of Object.keys(policyLabels)) { const value = parseNumber($<HTMLInputElement>(`policy-${key}`), key.endsWith('_micros')); if (value !== overview.policy[key]) { values[key] = value; before[key] = overview.policy[key] } }
  if (Object.keys(values).length) review('policy', 'service', overview.policy.revision, values, before, 'Updates effective service limits. Lowering a limit can immediately prevent subsequent requests or new registrations. No existing usage is erased.')
} catch (error) { $('error').textContent = String(error); $('error').hidden = false } }
$('cancel-change').onclick = () => { command = undefined; $<HTMLDialogElement>('confirm').close() }
$('apply-change').onclick = () => void run(async () => {
  if (!command) return
  // Keep the same operation ID after network ambiguity; never automatically resubmit.
  const target = command.target
  await api('/admin/api/change', command); command = undefined; $<HTMLDialogElement>('confirm').close(); await loadOverview()
  if (target !== 'service' && !$('user-detail').hidden) await inspectUser({ id: target } as User)
  $('status').textContent += ' Change applied and audit record saved.'
})
void run(loadOverview)

export {}
