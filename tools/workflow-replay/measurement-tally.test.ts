import assert from 'node:assert/strict'
import test from 'node:test'
import { summarize } from './measurement-tally.ts'

test('timing tally keeps clocks separate, counts samples and uses nearest-rank percentiles', () => {
  const rows=summarize([1000,2000,3000,100000].map(durationMicros=>({trace:'x',span:'read',clock:'ui' as const,durationMicros})))
  assert.equal(rows[0].count,4);assert.equal(rows[0].p50Ms,2);assert.equal(rows[0].p95Ms,100)
  assert.equal(summarize([{trace:'x',span:'read',clock:'ui',durationMicros:1},{trace:'x',span:'read',clock:'native',durationMicros:2}]).length,2)
  assert.throws(()=>summarize([{trace:'x',span:'read',clock:'ui',durationMicros:-1}]))
  assert.deepEqual(summarize([]),[])
})
