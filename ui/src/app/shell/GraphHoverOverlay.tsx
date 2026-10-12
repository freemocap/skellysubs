import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { matchesTarget, type GraphHoverTarget } from '../../domain/activity/graph-surface'
import { reportFault } from '../../platform/diagnostics/faults'
import { emitGraphHoverReply, onAiWindowEvent, onGraphHover } from '../../platform/ipc/window'
import { useGraphHoverStore } from '../../state/navigation/graph-hover'

interface Box { top: number; left: number; width: number; height: number }

/// The part of an element a learner can actually see: its rectangle clipped by
/// the viewport, by every ancestor that clips overflow, and by the docked AI
/// panel, which sits over the bottom of the app.
export function visibleBox(element: Element): Box | null {
  const own = getComputedStyle(element)
  if (own.display === 'none' || own.visibility === 'hidden' || element.closest('[hidden], details:not([open])')) return null
  const rect = element.getBoundingClientRect()
  let top = Math.max(rect.top, 0), left = Math.max(rect.left, 0)
  let bottom = Math.min(rect.bottom, window.innerHeight), right = Math.min(rect.right, window.innerWidth)
  for (let parent = element.parentElement; parent && parent !== document.body; parent = parent.parentElement) {
    const style = getComputedStyle(parent)
    if (style.display === 'none' || style.visibility === 'hidden') return null
    if (['hidden', 'clip', 'scroll', 'auto'].includes(style.overflowX) || ['hidden', 'clip', 'scroll', 'auto'].includes(style.overflowY)) {
      const clip = parent.getBoundingClientRect()
      if (['hidden', 'clip', 'scroll', 'auto'].includes(style.overflowX)) { left = Math.max(left, clip.left); right = Math.min(right, clip.right) }
      if (['hidden', 'clip', 'scroll', 'auto'].includes(style.overflowY)) { top = Math.max(top, clip.top); bottom = Math.min(bottom, clip.bottom) }
    }
  }
  const dock = document.getElementById('ai-activity')
  if (dock && !dock.contains(element)) {
    const panel = dock.getBoundingClientRect()
    if (panel.width > 0 && panel.height > 0 && panel.right > left && panel.left < right) bottom = Math.min(bottom, panel.top)
  }
  if (bottom - top < 1 || right - left < 1) return null
  return { top, left, width: right - left, height: bottom - top }
}

function matchingElements(target: GraphHoverTarget): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[data-graph-scope]')].filter(element => matchesTarget(element.dataset, target))
}

/// Draws downstream-surface hints in the main window and answers how many of the
/// matching surfaces are visible. Mounted once by the app shell.
export function GraphHoverOverlay() {
  const target = useGraphHoverStore(state => state.target)
  const origin = useGraphHoverStore(state => state.origin)
  const [boxes, setBoxes] = useState<Box[]>([])

  useEffect(() => {
    let active = true
    const stops: (() => void)[] = []
    const keep = (stop: () => void) => { if (active) stops.push(stop); else stop() }
    void onGraphHover(message => useGraphHoverStore.getState().show(message)).then(keep)
      .catch(error => reportFault('Listening for AI graph hover hints', error))
    void onAiWindowEvent(() => useGraphHoverStore.getState().clearAll()).then(keep)
      .catch(error => reportFault('Listening for AI window changes', error))
    return () => { active = false; stops.forEach(stop => stop()) }
  }, [])

  useEffect(() => {
    if (!target || !origin) { setBoxes([]); return }
    let frame = 0
    const observed = new Set<HTMLElement>()
    const measure = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        const elements = matchingElements(target)
        const visible = elements.map(visibleBox).filter((box): box is Box => box !== null)
        setBoxes(previous => JSON.stringify(previous) === JSON.stringify(visible) ? previous : visible)
        for (const element of observed) if (!elements.includes(element)) { resize.unobserve(element); observed.delete(element) }
        for (const element of elements) if (!observed.has(element)) { observed.add(element); resize.observe(element) }
        const reply = { session: origin.session, seq: origin.seq, matched: elements.length, visible: visible.length }
        if (origin.replyTo === 'local') useGraphHoverStore.getState().reply(reply)
        else emitGraphHoverReply(origin.replyTo, reply).catch(error => reportFault('Answering an AI graph hover hint', error))
      })
    }
    measure()
    const resize = new ResizeObserver(measure)
    const mutations = new MutationObserver(records => {
      if (records.some(record => !(record.target instanceof Element && record.target.closest('.graph-hover-layer'))
        && [...record.addedNodes, ...record.removedNodes].some(node => !(node instanceof Element && node.matches('.graph-hover-layer')))
        || record.type === 'attributes' && !(record.target instanceof Element && record.target.closest('.graph-hover-layer')))) measure()
    })
    mutations.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-graph-results', 'data-graph-turn', 'data-graph-run', 'data-graph-conversation', 'data-graph-engine', 'hidden', 'open', 'class', 'style'] })
    window.addEventListener('scroll', measure, true)
    window.addEventListener('resize', measure)
    window.addEventListener('focus', measure)
    return () => {
      cancelAnimationFrame(frame)
      resize.disconnect()
      mutations.disconnect()
      window.removeEventListener('scroll', measure, true)
      window.removeEventListener('resize', measure)
      window.removeEventListener('focus', measure)
    }
  }, [target, origin])

  if (!boxes.length) return null
  return createPortal(<div className="graph-hover-layer" aria-hidden="true">
    {boxes.map((box, index) => <span key={index} className="graph-hover-box" style={box} />)}
  </div>, document.body)
}
