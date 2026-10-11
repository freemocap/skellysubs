// @vitest-environment jsdom
import { act, render, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { GraphHoverOverlay, visibleBox } from './GraphHoverOverlay'
import { useGraphHoverStore } from '../../state/navigation/graph-hover'
import { graphTargetAttributes } from '../../domain/activity/graph-surface'

vi.mock('../../platform/ipc/window', () => ({ onGraphHover: async () => () => {}, onAiWindowEvent: async () => () => {}, emitGraphHoverReply: vi.fn().mockResolvedValue(undefined) }))
const rect = (left: number, top: number, width: number, height: number) => ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) })
afterEach(() => { vi.unstubAllGlobals(); useGraphHoverStore.getState().clearAll(); document.querySelectorAll('[data-fixture]').forEach(element => element.remove()) })

it('clips each scroll axis independently and rejects hidden surfaces', () => {
  const parent = document.createElement('div'), child = document.createElement('div')
  parent.dataset.fixture = ''; parent.style.overflowY = 'hidden'; parent.style.overflowX = 'visible'
  parent.append(child); document.body.append(parent)
  parent.getBoundingClientRect = () => rect(40, 40, 20, 20)
  child.getBoundingClientRect = () => rect(20, 20, 80, 80)
  expect(visibleBox(child)).toEqual({ left: 20, top: 40, width: 80, height: 20 })
  child.style.visibility = 'hidden'
  expect(visibleBox(child)).toBeNull()
})

it('highlights mounted targets, detects removal and settles without observing its own rendering forever', async () => {
  // Browsers deliver an initial notification on observe(), even without a resize.
  vi.stubGlobal('ResizeObserver', class {
    constructor(private callback: () => void) {}
    observe() { queueMicrotask(this.callback) }
    unobserve() {}
    disconnect() {}
  })
  const key = { scope: 'conversation' as const, conversationId: 'chat', turnId: 'turn' }
  const element = document.createElement('div'); element.dataset.fixture = ''
  for (const [name, value] of Object.entries(graphTargetAttributes(key, ['text']))) element.setAttribute(name, value)
  element.getBoundingClientRect = vi.fn(() => rect(20, 20, 80, 40))
  render(<GraphHoverOverlay />)
  act(() => useGraphHoverStore.getState().show({ session: 'overlay-test', seq: 1, replyTo: 'local', target: { key, results: ['text'] } }))
  await waitFor(() => expect(useGraphHoverStore.getState().replies['overlay-test']?.visible).toBe(0))
  act(() => document.body.append(element))
  await waitFor(() => expect(document.querySelector('.graph-hover-box')).not.toBeNull())
  await new Promise(resolve => setTimeout(resolve, 80))
  const calls = vi.mocked(element.getBoundingClientRect).mock.calls.length
  await new Promise(resolve => setTimeout(resolve, 80))
  expect(vi.mocked(element.getBoundingClientRect).mock.calls.length).toBe(calls)
  act(() => element.remove())
  await waitFor(() => expect(document.querySelector('.graph-hover-box')).toBeNull())
})
