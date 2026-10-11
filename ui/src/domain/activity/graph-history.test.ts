import { describe, expect, it } from 'vitest'
import type { InspectionSnapshot, RunHistory } from '../../generated/graph-contracts'
import { pipelineSnapshot } from '../../../tests/fixtures/graph-artifacts'
import { projectGraph } from './graph-projection'
import { buildTimeline, frameChanges, mergeHistory } from './graph-history'

const identity = { engine: 'engine-1', run: 'run-1', artifactId: 'artifact-pipeline' }
const page = (frames: InspectionSnapshot[], before: string | null, current: string): RunHistory => ({ engine: 'engine-1', run: 'run-1', current_revision: current, frames, before })
const frame = (revision: string, write: InspectionSnapshot['nodes'][string], extra: Parameters<typeof pipelineSnapshot>[0] = {}) =>
  pipelineSnapshot({ revision, ...extra, nodes: { read: 'Adopted', write, ...extra.nodes } })

describe('mergeHistory', () => {
  it('merges an older page and a refresh by decimal revision, keeping the oldest cursor', () => {
    const newest = mergeHistory(null, page([frame('10', 'Running'), frame('9', 'Waiting')], '9', '10'), identity)
    expect(newest.frames.map(f => f.revision)).toEqual(['9', '10'])
    expect(newest.before).toBe('9')
    const older = mergeHistory(newest, page([frame('8', 'Waiting')], null, '10'), identity)
    expect(older.frames.map(f => f.revision)).toEqual(['8', '9', '10'])
    expect(older.before).toBeNull()
    const refreshed = mergeHistory(older, page([frame('10', 'Running'), frame('11', 'Adopted')], '10', '11'), identity)
    expect(refreshed.frames.map(f => f.revision)).toEqual(['8', '9', '10', '11'])
    expect(refreshed.before).toBeNull()
    expect(refreshed.currentRevision).toBe('11')
  })

  it('rejects frames of another run or artifact and conflicting frames at one revision', () => {
    expect(() => mergeHistory(null, page([{ ...frame('1', 'Waiting'), run: 'other' }], null, '1'), identity)).toThrow()
    expect(() => mergeHistory(null, page([{ ...frame('1', 'Waiting'), artifact_id: 'other' }], null, '1'), identity)).toThrow()
    expect(() => mergeHistory(null, { ...page([], null, '1'), engine: 'other' }, identity)).toThrow()
    const loaded = mergeHistory(null, page([frame('3', 'Waiting')], null, '3'), identity)
    expect(() => mergeHistory(loaded, page([frame('3', 'Running')], null, '3'), identity)).toThrow()
  })
})

describe('frameChanges', () => {
  it('reports every category of native fact, not only dispositions', () => {
    const base = frame('1', 'Waiting')
    expect(frameChanges(base, base)).toEqual([])
    expect(frameChanges(base, { ...base, reasons: { write: ['Paused'] } })).toEqual(['reasons'])
    expect(frameChanges(base, { ...base, paused: true, step_available: true })).toEqual(['paused', 'step'])
    expect(frameChanges(base, frame('2', 'Running', { attempts: { write: [{ id: '1', execution: '1', state: 'Running', acquisition: 'Produced' }] } }))).toEqual(['disposition', 'attempts'])
  })
})

describe('buildTimeline', () => {
  const view = projectGraph(pipelineSnapshot())
  const running = (id: string) => ({ id, execution: id, state: 'Running' as const, acquisition: 'Produced' as const })

  it('builds lanes and attempt spans, marking intervals outside loaded history as open', () => {
    const history = mergeHistory(null, page([
      frame('5', 'Running', { attempts: { write: [running('3')] } }),
      frame('6', 'Running', { attempts: { write: [running('3')] } }),
      frame('7', 'Adopted', { attempts: { write: [{ ...running('3'), state: 'Adopted' }] } }),
    ], '5', '9'), identity)
    const timeline = buildTimeline(view, history)
    expect(timeline.revisions).toEqual(['5', '6', '7'])
    expect(timeline.lanes.write).toEqual([
      { disposition: 'Running', tone: 'running', fromIndex: 0, toIndex: 1, openStart: true, openEnd: false },
      { disposition: 'Adopted', tone: 'done', fromIndex: 2, toIndex: 2, openStart: false, openEnd: true },
    ])
    expect(timeline.attempts.write).toEqual([{ id: '3', fromIndex: 0, toIndex: 2, state: 'Adopted', openStart: true, openEnd: false }])
    expect(timeline.changes).toEqual([[], [], ['disposition', 'attempts']])
  })

  it('closes both ends when history is complete and current', () => {
    const history = mergeHistory(null, page([frame('1', 'Waiting'), frame('2', 'Adopted')], null, '2'), identity)
    const lane = buildTimeline(view, history).lanes.write
    expect(lane.every(segment => !segment.openStart && !segment.openEnd)).toBe(true)
  })

  it('rejects history for another artifact', () => {
    expect(() => buildTimeline(view, { ...identity, artifactId: 'other', frames: [], before: null, currentRevision: '1' })).toThrow()
  })

  it('ends a completed attempt at its terminal cut even when later snapshots retain it', () => {
    const attempt = { ...running('1'), state: 'Adopted' as const }
    const history = mergeHistory(null, page([
      frame('1', 'Running', { attempts: { write: [running('1')] } }),
      frame('2', 'Adopted', { attempts: { write: [attempt] } }),
      frame('3', 'Adopted', { attempts: { write: [attempt] } }),
    ], null, '3'), identity)
    expect(buildTimeline(view, history).attempts.write[0]).toMatchObject({ fromIndex: 0, toIndex: 1, openEnd: false })
  })
})
