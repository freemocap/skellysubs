// @vitest-environment jsdom
import { fireEvent, render } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { StableTurn } from './StableTurn'

afterEach(() => vi.restoreAllMocks())

it('does not measure finished message geometry when there is no temporary footprint to maintain', () => {
  const measure = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect')
  const content = (text: string) => <StableTurn><div className="partner-turn"><div className="msg chat-message bot">{text}</div></div></StableTurn>
  const view = render(content('Saved reply'))
  view.rerender(content('Saved translation'))
  expect(measure).not.toHaveBeenCalled()
})

it('keeps allocated height during hydration, then fits finished content and responds to resizing and interaction', () => {
  let height = 120, width = 300
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    return { width, height: Math.max(height, Number.parseFloat(this.style.minBlockSize) || 0) } as DOMRect
  })
  const content = (key: string, hydrating = true) => <StableTurn className="turn-stack"><div className="partner-turn">
    <div key={key} className="msg chat-message bot" aria-busy={hydrating}><button>Translate</button></div>
  </div></StableTurn>
  const view = render(content('pending'))
  const bubble = () => view.container.querySelector<HTMLElement>('.msg')!
  expect(bubble().style.minBlockSize).toBe('120px')
  height = 90
  view.rerender(content('saved'))
  expect(bubble().style.minBlockSize).toBe('120px')
  height = 180
  view.rerender(content('saved'))
  expect(bubble().style.minBlockSize).toBe('180px')
  width = 500; height = 100
  view.rerender(content('saved'))
  expect(bubble().style.minBlockSize).toBe('100px')
  height = 80
  view.rerender(content('complete', false))
  expect(bubble().style.minBlockSize).toBe('')
  view.rerender(content('complete', true))
  expect(bubble().style.minBlockSize).toBe('80px')
  fireEvent.click(view.getByRole('button'))
  height = 70
  view.rerender(content('complete', true))
  expect(bubble().style.minBlockSize).toBe('70px')
})
