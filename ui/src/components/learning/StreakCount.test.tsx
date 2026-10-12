// @vitest-environment jsdom
import { act, render, screen } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { StreakCount } from './StreakCount'
import { ProgressCounters } from './ProgressCounters'

afterEach(() => vi.useRealTimers())
it('shows one app-wide baseline before any data arrives, underneath larger XP totals', () => {
  const { container } = render(<ProgressCounters xp={12} global={40} effort={null} streaks={null} />)
  expect(container.querySelectorAll('.streak-count')).toHaveLength(1)
  expect(screen.getByLabelText('App streak: 1')).toHaveTextContent('🔥1')
  expect(container.querySelector('[data-streaks]')).not.toBeNull()
  expect(container.querySelector('.streak-confetti')).toBeNull()
})
it('rolls a claimed increment upward and removes the confetti without replaying it', () => {
  vi.useFakeTimers()
  const initial = { count: 4, day: 100, celebration: null }
  const { container, rerender } = render(<StreakCount value={initial} />)
  const increment = { count: 5, day: 101, celebration: 'app:101' }
  rerender(<StreakCount value={increment} />)
  expect(screen.getByLabelText('App streak: 5')).toHaveAttribute('data-celebrating')
  expect(container.querySelector('.streak-roll')).toHaveTextContent('45')
  expect(container.querySelectorAll('.streak-confetti i')).toHaveLength(6)
  rerender(<StreakCount value={{ ...increment, celebration: null }} />)
  expect(container.querySelectorAll('.streak-confetti i')).toHaveLength(6)
  act(() => vi.advanceTimersByTime(1600))
  expect(container.querySelector('.streak-confetti')).toBeNull()
  rerender(<StreakCount value={{ ...increment }} />)
  expect(container.querySelector('.streak-confetti')).toBeNull()
})
it('never rolls from zero, and enabling effects does not replay a disabled claim', () => {
  const value = { count: 1, day: 100, celebration: 'app:100' }
  const { container, rerender } = render(<StreakCount value={value} />)
  expect(container.querySelector('.streak-roll')).toBeNull()
  expect(screen.getByLabelText('App streak: 1')).toHaveTextContent('🔥1')
  rerender(<StreakCount value={value} effects={false} />)
  rerender(<StreakCount value={value} effects />)
  expect(container.querySelector('.streak-confetti')).toBeNull()
})
