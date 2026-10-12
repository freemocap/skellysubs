import { describe, expect, it } from 'vitest'
import type { Disposition } from '../../generated/graph-contracts'
import { pipelineSnapshot } from '../../../tests/fixtures/graph-artifacts'
import { projectGraph } from './graph-projection'
import { compareDecimal, DISPOSITION_TONE, groupOverlay, overlayGraph, previewNode, TONES } from './graph-overlay'

const ALL_DISPOSITIONS: Disposition[] = ['Disabled', 'Unrequested', 'Skipped', 'Waiting', 'Blocked', 'Ready', 'Paused', 'Held', 'Prepared', 'Running', 'Available', 'Adopted', 'Failed', 'Unknown', 'Cancelled']
const fault = { code: { Known: 'invalid_value' as const }, path: { Omitted: 'Unclassified' as const } }

describe('DISPOSITION_TONE', () => {
  it('maps every native disposition to one of the seven tones', () => {
    expect(Object.keys(DISPOSITION_TONE).sort()).toEqual([...ALL_DISPOSITIONS].sort())
    for (const disposition of ALL_DISPOSITIONS) expect(TONES).toContain(DISPOSITION_TONE[disposition])
  })
})

describe('overlayGraph', () => {
  it('copies dispositions, activation, run flags and tone counts', () => {
    const snapshot = pipelineSnapshot({ revision: '7', nodes: { read: 'Adopted', write: 'Running', note: 'Unrequested' }, paused: true })
    const overlay = overlayGraph(projectGraph(snapshot), snapshot)
    expect(overlay).toMatchObject({ engine: 'engine-1', run: 'run-1', revision: '7', paused: true, active: true, stepAvailable: false })
    expect(overlay.nodes.write).toMatchObject({ disposition: 'Running', tone: 'running', activation: 'Automatic', retrying: false })
    expect(overlay.nodes.note).toMatchObject({ disposition: 'Unrequested', tone: 'ask', activation: 'OnDemand' })
    expect(overlay.status.tones).toEqual({ done: 1, running: 1, waiting: 3, ask: 1, held: 0, problem: 0, ended: 0 })
    expect(overlay.status.running).toEqual(['write'])
  })

  it('orders attempts by decimal id and marks a retry after a failure', () => {
    const snapshot = pipelineSnapshot({
      nodes: { write: 'Running' },
      attempts: { write: [
        { id: '21', execution: '31', state: 'Running', acquisition: 'Produced' },
        { id: '9', execution: '30', state: { Failed: fault }, acquisition: 'Produced' },
      ] },
    })
    const overlay = overlayGraph(projectGraph(snapshot), snapshot)
    expect(overlay.nodes.write.attempts.map(attempt => attempt.id)).toEqual(['9', '21'])
    expect(overlay.nodes.write.attempts[0]).toEqual({ id: '9', execution: '30', state: 'Failed', acquisition: 'Produced', fault })
    expect(overlay.nodes.write.retrying).toBe(true)
    expect(overlay.status.retrying).toEqual(['write'])
  })

  it('resolves input reasons against the artifact binding, naming the consumer port and producer port', () => {
    const snapshot = pipelineSnapshot({
      nodes: { note: 'Waiting', write: 'Waiting' },
      reasons: {
        write: [{ Input: { port: 'text', producer: 'read', availability: 'Waiting' } }],
        note: [{ Input: { port: 'guard', producer: 'read', availability: 'Waiting' } }, { Control: { parent: 'read', state: 'Running' } }, 'DemandRequired'],
      },
    })
    const overlay = overlayGraph(projectGraph(snapshot), snapshot)
    expect(overlay.nodes.write.reasons).toEqual([{ kind: 'input', port: 'text', guard: false, producer: 'read', producerPort: 'text', availability: 'Waiting' }])
    expect(overlay.nodes.note.reasons).toEqual([
      { kind: 'input', port: 'guard', guard: true, producer: 'read', producerPort: 'allowed', availability: 'Waiting' },
      { kind: 'control', parent: 'read', state: 'Running' },
      { kind: 'demand-required' },
    ])
  })

  it('keeps reasons for terminal dispositions, with fault disclosure intact', () => {
    const snapshot = pipelineSnapshot({
      nodes: { write: 'Failed', note: 'Skipped' },
      reasons: { write: [{ Attempt: { id: '4', state: { Failed: fault } } }], note: ['GuardFalse'] },
    })
    const overlay = overlayGraph(projectGraph(snapshot), snapshot)
    expect(overlay.nodes.write.reasons).toEqual([{ kind: 'attempt', id: '4', state: 'Failed', fault }])
    expect(overlay.nodes.note.reasons).toEqual([{ kind: 'guard-false' }])
  })

  it('maps every simple reason and admission', () => {
    const snapshot = pipelineSnapshot({ reasons: { read: ['PolicyDisabled', 'Paused', 'Revoked', 'GuardUnavailable', { Admission: 'Provider' }] } })
    expect(overlayGraph(projectGraph(snapshot), snapshot).nodes.read.reasons.map(reason => reason.kind))
      .toEqual(['policy-disabled', 'paused', 'revoked', 'guard-unavailable', 'admission'])
  })

  describe('fails loudly', () => {
    it('on a snapshot from another artifact', () => {
      const snapshot = pipelineSnapshot()
      const view = projectGraph(snapshot)
      expect(() => overlayGraph(view, { ...snapshot, artifact_id: 'other' })).toThrow()
    })
    it('on missing or extra node facts', () => {
      const snapshot = pipelineSnapshot()
      const view = projectGraph(snapshot)
      const missing = structuredClone(snapshot); delete missing.nodes.read
      const extra = structuredClone(snapshot); extra.nodes.ghost = 'Waiting'
      expect(() => overlayGraph(view, missing)).toThrow()
      expect(() => overlayGraph(view, extra)).toThrow()
    })
    it('on an input reason the artifact does not bind to that producer', () => {
      const snapshot = pipelineSnapshot({ reasons: { write: [{ Input: { port: 'text', producer: 'note', availability: 'Waiting' } }] } })
      expect(() => overlayGraph(projectGraph(snapshot), snapshot)).toThrow()
      const unknown = pipelineSnapshot({ reasons: { write: [{ Input: { port: 'ghost', producer: null, availability: 'Absent' } }] } })
      expect(() => overlayGraph(projectGraph(unknown), unknown)).toThrow()
    })
    it('on a control reason for a node that is not a prerequisite', () => {
      const snapshot = pipelineSnapshot({ reasons: { write: [{ Control: { parent: 'read', state: 'Running' } }] } })
      expect(() => overlayGraph(projectGraph(snapshot), snapshot)).toThrow()
    })
  })
})

describe('groupOverlay', () => {
  it('lists member facts without inventing a group state', () => {
    const snapshot = pipelineSnapshot({ nodes: { 'pack/first': 'Adopted', 'pack/second': 'Running' } })
    const view = projectGraph(snapshot)
    expect(groupOverlay(view, overlayGraph(view, snapshot), 'pack')).toEqual([
      { id: 'pack/first', disposition: 'Adopted', tone: 'done' },
      { id: 'pack/inner/deep', disposition: 'Waiting', tone: 'waiting' },
      { id: 'pack/second', disposition: 'Running', tone: 'running' },
    ])
  })
})

describe('previewNode', () => {
  const preview = { attempt: '5', execution: '8', capture: { session: 's', sequence: '1', text: 'x', failure: null }, retained_sequence: null, uncommitted: true, live: true, complete: false }
  it('finds the node listing the preview attempt, or none', () => {
    const snapshot = pipelineSnapshot({ attempts: { write: [{ id: '5', execution: '8', state: 'Running', acquisition: 'Produced' }] } })
    expect(previewNode(snapshot, preview)).toBe('write')
    expect(previewNode(pipelineSnapshot(), preview)).toBeNull()
  })
})

describe('compareDecimal', () => {
  it('orders beyond the safe integer range and rejects non-decimal ids', () => {
    expect(compareDecimal('9007199254740993', '9007199254740992')).toBe(1)
    expect(compareDecimal('2', '10')).toBe(-1)
    expect(compareDecimal('7', '7')).toBe(0)
    expect(() => compareDecimal('1e3', '2')).toThrow()
  })
})
