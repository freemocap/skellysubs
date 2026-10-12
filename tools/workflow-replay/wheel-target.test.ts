import assert from 'node:assert/strict'
import test from 'node:test'
import { createRequire } from 'node:module'
import { wheelTargetMatches } from './wheel-target.ts'
const require = createRequire(new URL('../../ui/package.json', import.meta.url))
const { JSDOM } = require('jsdom')

test('wheel hit validation permits moving descendants only within the same scroll owner', () => {
  const dom = new JSDOM('<div id="panel" style="overflow-y:auto"><div id="target"></div><div id="sibling"></div></div><div id="other" style="overflow-y:auto"><div id="foreign"></div></div>', { runScripts: 'outside-only' })
  const window = dom.window
  try {
    for (const id of ['panel', 'other']) {
      Object.defineProperties(window.document.getElementById(id), { scrollHeight: { value: 500 }, clientHeight: { value: 100 } })
    }
    const matches = window.eval(`(${wheelTargetMatches.toString()})`)
    let hit = window.document.getElementById('sibling')
    window.document.elementFromPoint = () => hit
    assert.equal(matches('#target', 10, 20), true)
    const nested = window.document.getElementById('target')
    nested.style.overflowY = 'auto'
    Object.defineProperties(nested, { scrollHeight: { value: 102 }, clientHeight: { value: 100 } })
    assert.equal(matches('#target', 10, 20), true)
    hit.style.overflowY = 'auto'
    Object.defineProperties(hit, { scrollHeight: { value: 102 }, clientHeight: { value: 100 } })
    assert.equal(matches('#target', 10, 20), true)
    hit = window.document.getElementById('foreign')
    assert.equal(matches('#target', 10, 20), false)
    hit = null
    assert.equal(matches('#target', 10, 20), false)
    assert.equal(matches('#missing', 10, 20), false)
  } finally { window.close() }
})
