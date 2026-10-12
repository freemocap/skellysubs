import assert from 'node:assert/strict'
import test from 'node:test'
import { fixtureReply, fixtureServer } from './fixture-server.ts'

test('loopback fixture serves versioned health and exact operation identities with Unicode prose', async () => {
  const fixture = await fixtureServer()
  try {
    const health = await (await fetch(`${fixture.url}/protocol?verify_providers=true`)).json() as { operations_versions: number[] }
    assert.deepEqual(health.operations_versions, [3])
    const response = await fetch(`${fixture.url}/operations`, { method: 'POST', body: JSON.stringify({ version: 3, items: [{ operation_id: 'operation-1', attempt_id: 'attempt-1', deltas: true, request: { model: 'fixture' } }] }) })
    const frames = (await response.text()).trim().split('\n').map(line => JSON.parse(line))
    assert.equal(frames[0].attempt_id, 'attempt-1')
    assert.equal(frames[1].operation_id, 'operation-1')
    assert.equal(frames[1].response.choices[0].message.content, fixtureReply)
    assert.deepEqual(frames[2], { type: 'complete', count: 1 })
    assert.equal((await fetch(`${fixture.url}/unsupported`)).status, 400)
    assert.equal((await fetch(`${fixture.url}/protocol`, { headers: { authorization: 'secret' } })).status, 400)
    assert.equal(fixture.requests.rejected, 2)
  } finally { await fixture.close() }
})
