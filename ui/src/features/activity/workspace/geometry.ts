import { createContext, useContext, useEffect, useState } from 'react'
import type { WorkspaceGeometry } from './layout'
import { typeHue } from '../../../domain/activity/graph-types'

/// Stylesheet tokens that define the workspace geometry. The layout uses their
/// resolved pixel values so drawn rows and computed anchors cannot drift.
const TOKENS = {
  cardWidth: '--graph-card-width', bubbleWidth: '--graph-bubble-width', bubbleHeight: '--graph-bubble-height',
  border: '--border-width', header: '--graph-header', row: '--graph-row',
  divider: '--graph-divider', slot: '--graph-slot', footer: '--graph-footer', columnGap: '--graph-column-gap',
  rowGap: '--graph-row-gap', groupHeader: '--graph-group-header', groupPad: '--graph-group-pad',
} as const satisfies Record<keyof WorkspaceGeometry, string>

export function readGeometry(element: Element): WorkspaceGeometry {
  const style = getComputedStyle(element)
  const entries = Object.entries(TOKENS).map(([key, token]) => {
    const raw = style.getPropertyValue(token).trim()
    const match = /^(\d+(?:\.\d+)?)px$/.exec(raw)
    if (!match) throw new Error(`Workspace geometry token ${token} must resolve to pixels, got ${JSON.stringify(raw)}.`)
    return [key, Number(match[1])]
  })
  return Object.fromEntries(entries) as WorkspaceGeometry
}

/// A geometry supplied by a caller (tests, previews). Null reads the stylesheet.
export const WorkspaceGeometryContext = createContext<WorkspaceGeometry | null>(null)

/// Geometry from the document's resolved tokens, re-read when the pointer type
/// changes (coarse pointers enlarge touch targets).
export function useWorkspaceGeometry(): WorkspaceGeometry {
  const supplied = useContext(WorkspaceGeometryContext)
  const [measured, setMeasured] = useState<WorkspaceGeometry | null>(() => supplied ? null : readGeometry(document.documentElement))
  useEffect(() => {
    if (supplied) return
    const query = window.matchMedia('(pointer: coarse)')
    const update = () => setMeasured(readGeometry(document.documentElement))
    query.addEventListener('change', update)
    return () => query.removeEventListener('change', update)
  }, [supplied])
  const geometry = supplied ?? measured
  if (!geometry) throw new Error('Workspace geometry is unavailable.')
  return geometry
}

/// Color tokens for a type contract: a data hue for pins and connections, and an
/// ink for type names.
const TYPE_COLORS = ['var(--graph-type-0)', 'var(--graph-type-1)', 'var(--graph-type-2)', 'var(--graph-type-3)', 'var(--graph-type-4)', 'var(--graph-type-5)', 'var(--graph-type-6)', 'var(--graph-type-7)'] as const
const TYPE_INKS = ['var(--graph-type-ink-0)', 'var(--graph-type-ink-1)', 'var(--graph-type-ink-2)', 'var(--graph-type-ink-3)', 'var(--graph-type-ink-4)', 'var(--graph-type-ink-5)', 'var(--graph-type-ink-6)', 'var(--graph-type-ink-7)'] as const

export function typeColor(contract: string): string {
  return TYPE_COLORS[typeHue(contract)]
}

export function typeInk(contract: string): string {
  return TYPE_INKS[typeHue(contract)]
}
