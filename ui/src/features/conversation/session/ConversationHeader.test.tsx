// @vitest-environment jsdom
import { act, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ConversationHeader, type DifficultyPlace } from './ConversationHeader'

// jsdom has no layout, so the row's widths come from a model: the partner and
// difficulty share the room the counters leave (120px of counters in full,
// 60px compact). The partner needs 100px, and the difficulty select 120px more
// while it is in the header.
let room = 400
let resized: Array<() => void> = []
beforeEach(() => {
  resized = []
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { resized.push(callback) }
    observe() {} unobserve() {} disconnect() {}
  })
  const layout = (element: Element) => element.closest('.chat-head')?.getAttribute('data-fit') ?? ''
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(function (this: Element) {
    return this.matches('.conversation-identity') ? room - (layout(this).includes('compact-counters') ? 60 : 120) : 0
  })
  vi.spyOn(Element.prototype, 'scrollWidth', 'get').mockImplementation(function (this: Element) {
    return this.matches('.conversation-identity') ? 100 + (layout(this).includes('difficulty-in-settings') ? 0 : 120) : 0
  })
})
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

function Header({ onPlace }: { onPlace: (place: DifficultyPlace) => void }) {
  return <ConversationHeader error={null} onDifficultyPlace={onPlace}
    persona={<div className="persona-picker"><button type="button"><span className="partner-identity"><strong>Nóra</strong></span></button></div>}
    difficulty={<select className="chat-language-picker" aria-label="Difficulty"><option>Beginner</option></select>}>
    <div className="chat-heading-actions"><button type="button">Conversation settings</button></div>
  </ConversationHeader>
}
const head = () => document.querySelector('.chat-head')!

it.each([
  [400, 'full', 'header'],
  [300, 'compact-counters', 'header'],
  [250, 'difficulty-in-settings', 'settings'],
  [200, 'difficulty-in-settings compact-counters', 'settings'],
] as const)('with %ipx of room the one-row header takes the "%s" layout, with difficulty in the %s', (width, layout, place) => {
  room = width
  const onPlace = vi.fn()
  render(<Header onPlace={onPlace} />)
  expect(head()).toHaveAttribute('data-fit', layout)
  expect(onPlace).toHaveBeenLastCalledWith(place)
})

it('brings difficulty back into the header as soon as the row has room for it again', async () => {
  room = 200
  const onPlace = vi.fn()
  render(<Header onPlace={onPlace} />)
  expect(onPlace).toHaveBeenLastCalledWith('settings')
  // A side panel closes or the window widens: the header's own box grows.
  room = 400
  act(() => resized.forEach(callback => callback()))
  await waitFor(() => expect(head()).toHaveAttribute('data-fit', 'full'))
  await waitFor(() => expect(onPlace).toHaveBeenLastCalledWith('header'))
})
