import { reducedMotion } from '../../../platform/appearance/motion'
import { useEffect, useRef } from 'react'
import { useNodesInitialized, useUpdateNodeInternals, useReactFlow, useStore, type Viewport } from '@xyflow/react'
import type { TreeLayout } from '../../../domain/learning/catalog/skillTree'
import type { SkillCatalog } from '../../../domain/learning/catalog/skill-index'

export type CameraRequest = { sequence: number; target: string; action: 'focus' | 'whole' | 'back' }
/** Navigation changes the viewport only; every node remains in the same scene. */
export function TreeCamera({ catalog, request, layout, onRestore, onCanBackChange }: { catalog: SkillCatalog; request: CameraRequest; layout: TreeLayout; onRestore: (id: string) => void; onCanBackChange: (available: boolean) => void }) {
  const { descendants, node: treeNode, displayed: displayedTree } = catalog
  const initialized = useNodesInitialized()
  const updateNodeInternals = useUpdateNodeInternals()
  const { fitView, getViewport, setViewport } = useReactFlow()
  const width = useStore((state) => state.width)
  const height = useStore((state) => state.height)
  const history = useRef<{ viewport: Viewport; selected: string }[]>([])
  const previous = useRef<{ layout: TreeLayout; sequence: number; selected: string } | null>(null)
  useEffect(() => {
    if (width > 0 && height > 0) updateNodeInternals(displayedTree.map((node) => node.id))
  }, [width, height, layout, updateNodeInternals, displayedTree])
  useEffect(() => {
    if (!initialized || width <= 0 || height <= 0) return
    // Wait for the inspector's resize to settle before computing the viewport.
    const frame = requestAnimationFrame(() => {
      const last = previous.current
      const duration = reducedMotion() ? 0 : 420
      if (!last || last.layout !== layout) {
        history.current = []
        onCanBackChange(false)
        previous.current = { layout, sequence: request.sequence, selected: request.target }
        const node = treeNode(request.target)
        const branch = new Set([...descendants(node.id), ...(node.parent ? [node.parent] : [])])
        void fitView({ nodes: displayedTree.filter(item => branch.has(item.id)).map(item => ({ id: item.id })), padding: 0.16, duration: last ? duration : 0, minZoom: 0.08, maxZoom: request.target === 'experience' ? 1 : 2.6 })
        return
      }
      if (last.sequence === request.sequence) return
      previous.current = { layout, sequence: request.sequence, selected: request.target }
      if (request.action === 'back') {
        const view = history.current.pop()
        onCanBackChange(history.current.length > 0)
        if (view) {
          previous.current.selected = view.selected
          onRestore(view.selected)
          void setViewport(view.viewport, { duration })
        }
        return
      }
      history.current.push({ viewport: getViewport(), selected: last.selected })
      if (history.current.length > 30) history.current.shift()
      onCanBackChange(true)
      if (request.action === 'whole' || request.target === 'experience') {
        void fitView({ padding: 0.16, duration, minZoom: 0.08, maxZoom: 1 })
      } else {
        const node = treeNode(request.target)
        // Include the parent junction as an orientation anchor.
        const branch = new Set([...descendants(node.id), ...(node.parent ? [node.parent] : [])])
        const ids = displayedTree.filter((item) => branch.has(item.id)).map((item) => item.id)
        void fitView({ nodes: ids.map((id) => ({ id })), padding: 0.22, duration, minZoom: 0.08, maxZoom: 2.6 })
      }
    })
    return () => cancelAnimationFrame(frame)
  }, [initialized, width, height, layout, request, fitView, getViewport, setViewport, onRestore, onCanBackChange, descendants, treeNode, displayedTree])
  return null
}
