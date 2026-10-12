/** Loopback-only review fixture: synthetic records, no credentials or provider calls.
 * Run from the repository root after `node ui/tools/admin-build.ts`.
 */
import { createServer } from 'node:http'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const assets = resolve(import.meta.dirname, '../../server/app/diagnostics/admin_assets')
const policy = { max_users: 24, free_daily_micros: 1000000, extended_daily_micros: 3000000,
  global_daily_micros: 24000000, account_requests: 2000, global_requests: 12000,
  diagnostics_requests: 480, global_diagnostics: 600, revision: 0 }
const now = new Date(), stamp = (age: number) => new Date(now.getTime() - age).toISOString()
const users = Array.from({ length: 8 }, (_, i) => ({
  id: `preview:account-${i}`, email_label: `${String.fromCharCode(97 + i)}•••r@e•••d`,
  created_at: stamp((i + 1) * 86400000), last_seen: i === 7 ? null : stamp((i + 4) * 86400000),
  last_inference_at: i === 7 ? null : stamp(i * 3600000),
  daily_limit_micros: i === 2 ? 3000000 : null, effective_limit_micros: i === 2 ? 3000000 : 1000000,
  admin_revision: 0, token_version: 0, usage_90_days_micros: (8 - i) * 2500000,
  usage_24_hours_micros: (8 - i) * 92000, usage_7_days_micros: (8 - i) * 620000,
  usage: { day: stamp(0).slice(0, 10), present: true, micros: (8 - i) * 71000, tokens: (8 - i) * 1300, requests: 12, micros_credit: 0 },
  admission: { requests: (8 - i) * 12, requests_credit: 0, diagnostics_requests: 6, diagnostics_requests_credit: 0 },
}))
const points = Array.from({ length: 24 }, (_, i) => ({ time: stamp((23 - i) * 3600000),
  present: i > 2, micros: Math.round((1 + Math.sin(i)) * 150000), tokens: i * 890, requests: i * 2 }))
const overview = { environment: 'Synthetic preview · no connected service', administrator: 'Preview administrator',
  generated_at: stamp(0), revision: 'preview', policy, environment_defaults: policy, spending_paused: false,
  account_count: users.length, global_admission: { account_requests: 432, diagnostics_requests: 48 }, users,
  next_cursor: null, global_usage: users.map(u => u.usage), scope: 'Synthetic review records only.' }
const events = Array.from({ length: 30 }, (_, i) => ({ timestamp: stamp(i * 14000),
  event: ['request_finished', 'provider_finished', 'request_headers', 'request_started', 'provider_failed'][i % 5],
  route: '/v1/operations', provider: 'OPENROUTER', method: 'POST',
  status: i % 5 === 4 ? 502 : i % 5 === 3 ? null : 200,
  duration_ms: i % 5 === 3 ? null : [2450, 18300, 180, 0, 62700][i % 5],
  code: i % 5 === 4 ? 'UPSTREAM_ERROR' : null, request_id: i.toString(16).padStart(32, '0'), bytes: i * 250,
  diagnostics: i % 5 === 4 ? { stage: 'provider_response', code: 'UPSTREAM_ERROR', request_id: 'synthetic-receipt' } : null,
}))
createServer((request, response) => {
  response.setHeader('Cache-Control', 'no-store')
  if (request.method !== 'GET') { response.writeHead(405); response.end('Preview is read-only.'); return }
  const path = new URL(request.url ?? '/', 'http://127.0.0.1').pathname
  let data: unknown
  if (path === '/admin/api/overview') data = overview
  else if (path === '/admin/api/timeline') data = { points, scope: 'Synthetic allowance intervals.' }
  else if (path.startsWith('/admin/api/users/')) {
    const user = users.find(u => u.id === decodeURIComponent(path.split('/').at(-1)!))
    if (!user) { response.writeHead(404); response.end(); return }
    data = { user, identity: { email: 'synthetic-learner@example.invalid', name: 'Synthetic account' }, usage: [user.usage],
      installation_policy: { maximum_retained: 100, inactivity_days: 90 },
      devices: [{ id: 'synthetic-installation', platform: 'Windows', app_version: 'preview', first_seen: user.created_at, last_seen: user.last_seen }],
      reservations: [{ id: 'synthetic-reservation', created_at: stamp(24000), updated_at: stamp(1000),
        status: 'settled', reserved_micros: 8000, actual_micros: 2500, tokens: 420, cost_basis: 'provider_reported', provider_id: 'synthetic-receipt' }] }
  } else if (path === '/admin/api/logs') data = { entries: events, scope: 'Synthetic events only.', since: stamp(86400000) }
  else if (path === '/admin/api/audit') data = [{ created_at: stamp(3600000), action: 'user_limit', target: users[2].id,
    before: { daily_limit_micros: null }, after: { daily_limit_micros: 3000000 }, actor: 'preview-administrator' }]
  if (data !== undefined) { response.setHeader('Content-Type', 'application/json'); response.end(JSON.stringify(data)); return }
  const name = path === '/admin' || path === '/' ? 'index.html' : path.split('/').at(-1)
  if (!['index.html', 'admin.js', 'admin.css'].includes(name ?? '')) { response.writeHead(404); response.end(); return }
  response.setHeader('Content-Type', name!.endsWith('.js') ? 'text/javascript' : name!.endsWith('.css') ? 'text/css' : 'text/html')
  response.end(readFileSync(resolve(assets, name!)))
}).listen(4318, '127.0.0.1', () => console.log('Synthetic admin preview: http://127.0.0.1:4318/admin'))
