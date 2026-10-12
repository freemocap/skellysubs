import { useCallback, useState } from 'react'
import type { NodeLevel, Point, WorkspaceState } from './layout'

/// Expansion, dragged offsets and resized cards are the viewer's presentation
/// choices. They are keyed by step name, shared by
/// every run and every workflow, so a step keeps its place and size when the
/// next run arrives even when that run is a different workflow.
const KEY = 'skellyspeak_ai_workspace'

const EMPTY: WorkspaceState = { levels: {}, offsets: {}, sizes: {} }

const isPoint = (entry: unknown): boolean => Array.isArray(entry) && entry.length === 2 && entry.every(value => typeof value === 'number' && Number.isFinite(value))

function parse(raw: string): WorkspaceState {
  const value = JSON.parse(raw) as unknown
  const record = (candidate: unknown, valid: (entry: unknown) => boolean) =>
    !!candidate && typeof candidate === 'object' && !Array.isArray(candidate) && Object.values(candidate).every(valid)
  const state = value as WorkspaceState
  if (!value || typeof value !== 'object'
    || !record(state.levels, entry => entry === 'bubble' || entry === 'card')
    || !record(state.offsets, isPoint)
    || !record(state.sizes, isPoint)) {
    throw new Error('Stored AI workspace layout has an invalid shape.')
  }
  return state
}

function load(): WorkspaceState {
  const raw = localStorage.getItem(KEY)
  return raw === null ? EMPTY : parse(raw)
}

export function useWorkspaceState() {
  const [state, setState] = useState(load)
  const save = useCallback((next: WorkspaceState) => {
    localStorage.setItem(KEY, JSON.stringify(next))
    setState(next)
  }, [])
  const setLevel = useCallback((id: string, level: NodeLevel) => save({ ...state, levels: { ...state.levels, [id]: level } }), [state, save])
  const setAll = useCallback((ids: string[], level: NodeLevel) => save({ ...state, levels: Object.fromEntries(ids.map(id => [id, level])) }), [state, save])
  const setOffset = useCallback((id: string, offset: Point) => save({ ...state, offsets: { ...state.offsets, [id]: offset } }), [state, save])
  /// A resize from a top or start edge moves the item as well as sizing it.
  const setBox = useCallback((id: string, offset: Point, size: Point) => save({ ...state, offsets: { ...state.offsets, [id]: offset }, sizes: { ...state.sizes, [id]: size } }), [state, save])
  /// Forget every dragged offset and resized card.
  const tidy = useCallback(() => save({ ...state, offsets: {}, sizes: {} }), [state, save])
  return { state, setLevel, setAll, setOffset, setBox, tidy }
}
