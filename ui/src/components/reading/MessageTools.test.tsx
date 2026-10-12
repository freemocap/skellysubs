// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { useState } from 'react'
import { MessageTools } from './MessageTools'

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals() })

it('shows tools when they fit and moves only excess tools into the menu as its container resizes', () => {
  // Row tools are icons named by their tooltip; the menu shows the name beside the icon.
  let width = 300
  const callbacks = new Set<() => void>()
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { callbacks.add(callback) }
    observe() {} disconnect() {} unobserve() {}
  })
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (this: HTMLElement) { return width })
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (this: HTMLElement) {
    const size = this.classList.contains('message-tools-primary') ? 100 : this.classList.contains('message-tools-fixed') ? 30 : this.hasAttribute('data-measured-tool') ? 80 : 24
    return size
  })
  // A transformed entrance frame must not shrink the preferred toolbar width.
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: 0 } as DOMRect)
  const select = vi.fn()
  render(<MessageTools play={null} inspect={null} tools={[]} actions={null}
    more={[{ key:'words', label:'Words', icon:'words', onSelect:select }, { key:'analysis', label:'Analysis', icon:'analysis', onSelect:select }]} />)
  expect(screen.queryByRole('button', { name:'More actions' })).toBeNull()
  expect(screen.getByRole('button', { name:'Analysis' })).toBeVisible()
  act(() => { width = 250; callbacks.forEach(callback => callback()) })
  expect(screen.getByRole('button', { name:'Words' })).toBeVisible()
  expect(screen.queryByRole('button', { name:'Analysis' })).toBeNull()
  expect(screen.getByRole('button', { name:'Words' })).toHaveAttribute('title', 'Words')
  expect(screen.getByRole('button', { name:'Words' })).toHaveTextContent('')
  fireEvent.click(screen.getByRole('button', { name:'More actions' }))
  const analysis = screen.getByRole('button', { name:'Analysis' })
  expect(analysis).toHaveTextContent('Analysis')
  expect(analysis.closest('.message-tools-panel')?.parentElement).toBe(document.body)
  fireEvent.keyDown(analysis, { key: 'Escape' })
  expect(screen.queryByRole('button', { name:'Analysis' })).toBeNull()
  expect(screen.getByRole('button', { name:'More actions' })).toHaveFocus()
  fireEvent.click(screen.getByRole('button', { name:'More actions' }))
  fireEvent.click(screen.getByRole('button', { name:'Analysis' }))
  expect(select).toHaveBeenCalledOnce()
  act(() => { width = 300; callbacks.forEach(callback => callback()) })
  expect(screen.queryByRole('button', { name:'More actions' })).toBeNull()
  expect(screen.getAllByRole('button', { name:'Analysis' })).toHaveLength(1)
})

it('announces pending work without exposing the reserved controls', () => {
  render(<MessageTools pending="Sending…" play={{ playing: false, onToggle: vi.fn() }} inspect={null}
    tools={[{ key: 'translate', label: 'Translate', icon: 'translate', onSelect: vi.fn() }]}
    more={[]} actions={<button>Edit</button>} />)
  expect(screen.getByRole('status')).toHaveTextContent('Sending…')
  expect(screen.queryByRole('button')).toBeNull()
})

it('keeps one playback button through preparation and playing, with cancellation available', () => {
  const toggle = vi.fn()
  const view = render(<MessageTools play={{ playing: false, preparing: true, onToggle: toggle }} inspect={null} tools={[]} actions={null} more={[]} />)
  const button = screen.getByRole('button', { name: 'Cancel speech preparation' })
  expect(screen.getByRole('status')).toHaveTextContent('Preparing audio…')
  fireEvent.click(button)
  expect(toggle).toHaveBeenCalledOnce()
  view.rerender(<MessageTools play={{ playing: true, preparing: false, onToggle: toggle }} inspect={null} tools={[]} actions={null} more={[]} />)
  expect(screen.getByRole('button', { name: 'Stop playback' })).toBe(button)
  expect(screen.queryByRole('status')).toBeNull()
})


it('keeps owner actions in a keyboard and outside-tap dismissible menu even when reading tools fit', () => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  render(<MessageTools play={null} inspect={null} tools={[]} actions={null} more={[]}
    menuActions={<button>Explain sentence</button>} />)
  const trigger = screen.getByRole('button', { name: 'More actions' })
  expect(screen.queryByRole('button', { name: 'Explain sentence' })).toBeNull()
  fireEvent.click(trigger)
  const item = screen.getByRole('button', { name: 'Explain sentence' })
  expect(item).toHaveFocus()
  fireEvent.keyDown(item, { key: 'Escape' })
  expect(trigger).toHaveFocus()
  expect(screen.queryByRole('button', { name: 'Explain sentence' })).toBeNull()
  fireEvent.click(trigger)
  fireEvent.pointerDown(document.body)
  expect(screen.queryByRole('button', { name: 'Explain sentence' })).toBeNull()
})

it('keeps an inline reading toggle under the pointer as its result grows, until the learner scrolls', () => {
  const callbacks = new Set<() => void>()
  vi.stubGlobal('ResizeObserver', class {
    constructor(callback: () => void) { callbacks.add(callback) }
    observe() {} disconnect() {} unobserve() {}
  })
  let growth = 0
  let scroller: HTMLDivElement
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    return { top: 100 + growth - (scroller?.scrollTop ?? 0), width: 300 } as DOMRect
  })
  function Example() {
    const [open, setOpen] = useState(false)
    return <div style={{ overflowY: 'auto' }}><div className="msg"><MessageTools play={null} inspect={null} actions={null} more={[]}
      tools={[{ key: 'words', label: 'Words', icon: 'words', pressed: open, onSelect: () => { growth = open ? 0 : 30; setOpen(!open) } }]} /></div></div>
  }
  const view = render(<Example />)
  scroller = view.container.firstElementChild as HTMLDivElement
  scroller.scrollTop = 20
  const button = screen.getByRole('button', { name: 'Words' })
  const top = button.getBoundingClientRect().top
  fireEvent.click(button)
  expect(button.getBoundingClientRect().top).toBe(top)
  act(() => { growth = 45; callbacks.forEach(callback => callback()) })
  expect(button.getBoundingClientRect().top).toBe(top)
  fireEvent.wheel(scroller)
  const stopped = scroller.scrollTop
  act(() => { growth = 60; callbacks.forEach(callback => callback()) })
  expect(scroller.scrollTop).toBe(stopped)
})
