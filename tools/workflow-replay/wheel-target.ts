/** Wheel events can retain a descendant target while a scroll gesture moves
 * content beneath the pointer. Validate the scroll owner, not that moving child.
 * The event is still delivered to the exact recorded descendant by the runner. */
export function wheelTargetMatches(selector: string, x: number, y: number): boolean {
  const target = document.querySelector(selector)
  const hit = document.elementFromPoint(x, y)
  if (!target || !hit) return false
  if (target === hit || target.contains(hit)) return true
  const owner = (element: Element): Element | null => {
    for (let current: Element | null = element; current; current = current.parentElement) {
      const style = getComputedStyle(current)
      if ((/(auto|scroll)/.test(style.overflowY) && current.scrollHeight > current.clientHeight)
        || (/(auto|scroll)/.test(style.overflowX) && current.scrollWidth > current.clientWidth)) return current
    }
    return null
  }
  const hitOwners = new Set<Element>()
  for (let current = owner(hit); current; current = current.parentElement ? owner(current.parentElement) : null) {
    hitOwners.add(current)
  }
  // A gesture can move past nested text areas while its containing dialog scrolls.
  // Independent scroll surfaces have no shared scrollable ancestor.
  for (let current = owner(target); current; current = current.parentElement ? owner(current.parentElement) : null) {
    if (hitOwners.has(current)) return true
  }
  return false
}
