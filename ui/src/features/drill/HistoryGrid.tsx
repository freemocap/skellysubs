import { reducedMotion } from '../../platform/appearance/motion'
import { Component, createRef, type ReactNode } from 'react'

type WordSnapshot = { rect: DOMRect; color: string }
type Snapshot = { id: string | null; words: WordSnapshot[]; rows: Map<string, DOMRect> } | null
type Props = { motionKey?: string; children: ReactNode; expandedId: string | null; arriving: boolean }

/** Capture outgoing word positions before React replaces the expanded card.
 * Only floating word copies move; the completed cards keep their boxes. */
export class HistoryGrid extends Component<Props, Record<string, never>, Snapshot> {
  private root = createRef<HTMLDivElement>()
  private rowAnimations: Animation[] = []
  private flights: { element: HTMLElement; animation: Animation }[] = []

  getSnapshotBeforeUpdate(previous: Props): Snapshot {
    if (previous.motionKey === this.props.motionKey && previous.expandedId === this.props.expandedId) return null
    if (reducedMotion()) return null
    const entries = [...(this.root.current?.querySelectorAll<HTMLElement>('[data-history-id]') ?? [])]
    const rows = new Map(entries.filter(element => element.dataset.historyId !== previous.expandedId)
      .map(element => [element.dataset.historyId!, element.getBoundingClientRect()]))
    const changed = previous.expandedId !== this.props.expandedId
    const card = changed ? entries.find(element => element.dataset.historyId === previous.expandedId) : null
    const words = [...(card?.querySelectorAll<HTMLElement>('.drill-word-pair:not([data-outcome="extra"]) > .drill-word-reference') ?? [])]
      .map(element => ({ rect: element.getBoundingClientRect(), color: getComputedStyle(element).color }))
    return { id: changed ? previous.expandedId : null, words, rows }
  }

  componentDidUpdate(_previous: Props, _state: Record<string, never>, snapshot: Snapshot) {
    if (!snapshot) return
    const entries = [...(this.root.current?.querySelectorAll<HTMLElement>('[data-history-id]') ?? [])]
    const moved = entries.filter(element => {
      if (element.dataset.historyId === this.props.expandedId) return false
      const before = snapshot.rows.get(element.dataset.historyId!)
      return !before || before.top !== element.getBoundingClientRect().top
    })
    if (!moved.length && !snapshot.words.length) return
    this.clearFlights()
    const start = document.timeline?.currentTime
    moved.forEach(element => {
      if (!element.animate) return
      const rect = element.getBoundingClientRect()
      const before = snapshot.rows.get(element.dataset.historyId!)
      const offset = before ? before.top - rect.top : this.props.expandedId ? -rect.height : 0
      const animation = element.animate([
        { transform: `translateY(${offset}px)`, opacity: before ? 1 : 0 },
        { transform: 'none', opacity: 1 },
      ], { duration: 800, easing: 'ease-in-out' })
      if (start != null) animation.startTime = start
      this.rowAnimations.push(animation)
    })
    const row = [...(this.root.current?.querySelectorAll<HTMLElement>('[data-history-id]') ?? [])]
      .find(element => element.dataset.historyId === snapshot.id)
    const cells = row?.querySelectorAll<HTMLElement>('.drill-word-cell')
    snapshot.words.forEach((word, index) => {
      const cell = cells?.[index]
      if (!cell || !cell.animate || !word.rect.width) return
      const to = cell.getBoundingClientRect()
      if (!to.width) return
      const copy = document.createElement('span')
      copy.className = 'drill-word-flight'
      copy.textContent = cell.textContent
      copy.setAttribute('aria-hidden', 'true')
      Object.assign(copy.style, { left: `${word.rect.left}px`, top: `${word.rect.top}px`, color: word.color })
      document.body.append(copy)
      const transform = `translate(${to.left - word.rect.left}px, ${to.top - word.rect.top}px)`
      const animation = copy.animate([
        { transform: 'none', opacity: 1 },
        { transform, opacity: 1, offset: 0.85 },
        { transform, opacity: 0 },
      ], { duration: 800, easing: 'ease-in-out' })
      if (document.timeline?.currentTime != null) animation.startTime = document.timeline.currentTime
      animation.onfinish = () => copy.remove()
      this.flights.push({ element: copy, animation })
    })
  }

  componentWillUnmount() { this.clearFlights() }

  private clearFlights() {
    this.flights.forEach(({ element, animation }) => { animation.cancel(); element.remove() })
    this.flights = []
    this.rowAnimations.forEach(animation => animation.cancel())
    this.rowAnimations = []
  }

  render() {
    return <div ref={this.root} className="drill-word-grid" data-arriving={this.props.arriving ? '' : undefined}>{this.props.children}</div>
  }
}
