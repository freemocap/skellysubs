// @vitest-environment jsdom
import { fireEvent, render } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { ResponseDetails } from './ResponseDetails'

it('serializes metadata only when expanded and reuses it while unchanged', () => {
  const toJSON = vi.fn(() => ({ requestId: 'request' }))
  const value = { toJSON }
  const view = render(<ResponseDetails value={value} />)
  expect(toJSON).not.toHaveBeenCalled()
  const details = view.container.querySelector('details')!
  details.open = true
  fireEvent(details, new Event('toggle'))
  expect(toJSON).toHaveBeenCalledOnce()
  expect(view.container.querySelector('pre')).toHaveTextContent('requestId')
  view.rerender(<ResponseDetails value={value} />)
  expect(toJSON).toHaveBeenCalledOnce()
})
