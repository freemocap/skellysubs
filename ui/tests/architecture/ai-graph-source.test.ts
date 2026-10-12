// @vitest-environment node
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, it } from 'vitest'

/// Presentation reads the compiler-exported artifact; application code must not
/// maintain another topology. The workspace projection is tested in domain/activity/graph-projection.test.ts.
const root = fileURLToPath(new URL('../../../', import.meta.url))
const definition = JSON.parse(readFileSync(join(root, 'ui/src/generated/coach-graph.json'), 'utf8'))
const kinds = Object.keys(definition.artifact.definition.nodes)

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return entry.name === 'generated' ? [] : sources(path)
    return /\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name) ? [path] : []
  })
}
const ui = join(root, 'ui/src')
const kindLiteral = new RegExp(`['"\`](${kinds.join('|')})['"\`]`, 'g')

it('reads the plan it guards', () => {
  expect(kinds.length).toBeGreaterThan(0)
  expect(definition.artifact_id).toBeTruthy()
})

it('keeps the AI View, its summary and its hydration kind-agnostic', () => {
  const agnostic = [...sources(join(ui, 'features/activity')), join(ui, 'domain/conversation/activity-summary.ts'), join(ui, 'components/feedback/ActivitySummary.tsx')]
  const found = agnostic.flatMap(path => [...readFileSync(path, 'utf8').matchAll(kindLiteral)].map(match => `${path.slice(root.length)}: ${match[1]}`))
  expect(found).toEqual([])
})


it('declares no collection of operation kinds or dependency relations anywhere in the UI', () => {
  const found: string[] = []
  for (const path of sources(ui)) {
    const text = readFileSync(path, 'utf8')
    for (const literal of text.matchAll(/[[{][^[\]{}]*[\]}]/g)) {
      const named = new Set([...literal[0].matchAll(kindLiteral)].map(match => match[1]))
      if (named.size >= 2) found.push(`${path.slice(root.length)}: ${[...named].join(', ')}`)
    }
    if (/\bdependencies\s*:\s*\[\s*['"]/.test(text)) found.push(`${path.slice(root.length)}: dependencies literal`)
  }
  expect(found).toEqual([])
})
