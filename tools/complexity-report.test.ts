import assert from 'node:assert/strict'
import test from 'node:test'
import { measureFunctions } from './complexity-report.ts'

test('counts syntax decisions without comments/strings or nested function leakage',()=>{
  const rows=measureFunctions('fixture.ts',`function outer(x: boolean) {
    // if for while
    const text = 'if && ||';
    if (x && text) { for (const c of text) { if (c) return 1 } }
    const inner = () => x ? 1 : 0;
    return 0;
  }`)
  assert.equal(rows[0].decisions,4);assert.equal(rows[0].maxNesting,3)
  assert.equal(rows[1].decisions,1);assert.equal(rows[1].name,'inner')
})
test('counts catch, case and logical operators while leaving default and optional access alone',()=>{
  const [row]=measureFunctions('fixture.ts',`const a = (x:any) => { try { switch(x) {case 1: return x?.a ?? 1; default: return 0} } catch {return 2} }`)
  assert.equal(row.decisions,3)
})
