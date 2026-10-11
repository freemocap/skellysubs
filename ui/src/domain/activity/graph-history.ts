import type { Disposition, InspectionSnapshot, RunHistory } from '../../generated/graph-contracts'
import { compareDecimal, DISPOSITION_TONE, type AttemptStateKind, type Tone } from './graph-overlay'
import type { GraphView } from './graph-projection'

/// Recorded run history as presented on the timeline. Frames are native
/// snapshots; this module only orders, merges and compares them. Intervals that
/// were not loaded stay explicitly open rather than being presented as complete.

export interface HistoryIdentity {
  engine: string
  run: string
  artifactId: string
}

export interface LoadedHistory extends HistoryIdentity {
  /// Ascending by revision.
  frames: InspectionSnapshot[]
  /// Cursor for the next older page; null when the oldest retained frame is loaded.
  before: string | null
  currentRevision: string
}

export type ChangeCategory = 'disposition' | 'activation' | 'attempts' | 'reasons' | 'paused' | 'active' | 'stepping' | 'step'

export interface LaneSegment {
  disposition: Disposition
  tone: Tone
  fromIndex: number
  toIndex: number
  /// The segment may have begun before the oldest loaded frame.
  openStart: boolean
  /// The segment continues past the newest loaded frame.
  openEnd: boolean
}

export interface AttemptSpan {
  id: string
  fromIndex: number
  toIndex: number
  /// State in the last frame that lists this attempt.
  state: AttemptStateKind
  openStart: boolean
  openEnd: boolean
}

export interface Timeline {
  revisions: string[]
  /// Categories that changed at each frame relative to the previous loaded frame.
  changes: ChangeCategory[][]
  lanes: Record<string, LaneSegment[]>
  attempts: Record<string, AttemptSpan[]>
}

function checkIdentity(identity: HistoryIdentity, frame: InspectionSnapshot) {
  if (frame.engine !== identity.engine || frame.run !== identity.run || frame.artifact_id !== identity.artifactId) {
    throw new Error('Graph history does not match the selected run.')
  }
}

/// Merge one fetched page into what is already loaded. A page may be the newest
/// page (a refresh) or an older page fetched with a `before` cursor.
export function mergeHistory(loaded: LoadedHistory | null, page: RunHistory, identity: HistoryIdentity): LoadedHistory {
  if (page.engine !== identity.engine || page.run !== identity.run) throw new Error('Graph history does not match the selected run.')
  if (loaded && (loaded.engine !== identity.engine || loaded.run !== identity.run || loaded.artifactId !== identity.artifactId)) {
    throw new Error('Loaded graph history belongs to a different run.')
  }
  const byRevision = new Map<string, InspectionSnapshot>()
  const add = (frame: InspectionSnapshot) => {
    checkIdentity(identity, frame)
    const existing = byRevision.get(frame.revision)
    if (existing && JSON.stringify(existing) !== JSON.stringify(frame)) throw new Error(`Graph history has conflicting frames at revision ${frame.revision}.`)
    byRevision.set(frame.revision, frame)
  }
  for (const frame of loaded?.frames ?? []) add(frame)
  for (const frame of page.frames) add(frame)
  const frames = [...byRevision.values()].sort((a, b) => compareDecimal(a.revision, b.revision))

  const pageOldest = page.frames.map(frame => frame.revision).sort(compareDecimal)[0]
  const loadedOldest = loaded?.frames[0]?.revision
  const before = !loaded ? page.before
    : pageOldest !== undefined && (loadedOldest === undefined || compareDecimal(pageOldest, loadedOldest) <= 0) ? page.before
    : loaded.before
  const currentRevision = loaded && compareDecimal(loaded.currentRevision, page.current_revision) > 0 ? loaded.currentRevision : page.current_revision
  return { ...identity, frames, before, currentRevision }
}

const sameJson = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b)

export function frameChanges(previous: InspectionSnapshot, next: InspectionSnapshot): ChangeCategory[] {
  const changed: ChangeCategory[] = []
  if (!sameJson(previous.nodes, next.nodes)) changed.push('disposition')
  if (!sameJson(previous.activation, next.activation)) changed.push('activation')
  if (!sameJson(previous.attempts, next.attempts)) changed.push('attempts')
  if (!sameJson(previous.reasons, next.reasons)) changed.push('reasons')
  if (previous.paused !== next.paused) changed.push('paused')
  if (previous.active !== next.active) changed.push('active')
  if (previous.stepping !== next.stepping) changed.push('stepping')
  if (previous.step_available !== next.step_available) changed.push('step')
  return changed
}

function attemptStateOf(state: InspectionSnapshot['attempts'][string][number]['state']): AttemptStateKind {
  if (typeof state === 'string') return state
  if ('Failed' in state) return 'Failed'
  throw new Error('Unrecognized attempt state shape.')
}

export function buildTimeline(view: GraphView, history: LoadedHistory): Timeline {
  if (history.artifactId !== view.artifactId) throw new Error('Graph history belongs to a different artifact.')
  const { frames } = history
  const last = frames.length - 1
  const earlierUnloaded = history.before !== null
  const laterUnloaded = last >= 0 && compareDecimal(frames[last].revision, history.currentRevision) < 0
  const lanes: Record<string, LaneSegment[]> = {}
  const attempts: Record<string, AttemptSpan[]> = {}
  for (const id of view.ids) {
    const segments: LaneSegment[] = []
    const spans = new Map<string, AttemptSpan>()
    frames.forEach((frame, index) => {
      const disposition = frame.nodes[id]
      if (!disposition) throw new Error(`History frame ${frame.revision} has no disposition for ${id}.`)
      const open = segments[segments.length - 1]
      if (open && open.disposition === disposition && open.toIndex === index - 1) open.toIndex = index
      else segments.push({ disposition, tone: DISPOSITION_TONE[disposition], fromIndex: index, toIndex: index, openStart: index === 0 && earlierUnloaded, openEnd: false })
      for (const attempt of frame.attempts[id] ?? []) {
        const span = spans.get(attempt.id)
        const state = attemptStateOf(attempt.state)
        if (span) {
          if (!['Prepared', 'Running', 'Available'].includes(span.state)) continue
          span.toIndex = index
          span.state = state
        } else spans.set(attempt.id, { id: attempt.id, fromIndex: index, toIndex: index, state, openStart: index === 0 && earlierUnloaded, openEnd: false })
      }
    })
    const tail = segments[segments.length - 1]
    if (tail) tail.openEnd = laterUnloaded
    for (const span of spans.values()) if (span.toIndex === last) span.openEnd = ['Prepared', 'Running', 'Available'].includes(span.state)
    lanes[id] = segments
    attempts[id] = [...spans.values()].sort((a, b) => compareDecimal(a.id, b.id))
  }
  return {
    revisions: frames.map(frame => frame.revision),
    changes: frames.map((frame, index) => index === 0 ? [] : frameChanges(frames[index - 1], frame)),
    lanes, attempts,
  }
}
