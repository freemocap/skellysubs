// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { useAppearance } from './useAppearance'
import { reducedMotion, useReducedMotion } from './motion'
import { useReplyReveal } from '../../features/conversation/messages/useReplyReveal'
import { DEFAULT_APPEARANCE, type MotionPreference } from '../../generated/contracts'
import type { Settings } from '../../types'

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers() })

it('shares live system and explicit preferences between CSS and animation consumers', () => {
  let matches = true
  const listeners = new Set<() => void>()
  vi.spyOn(window, 'matchMedia').mockImplementation(query => ({
    get matches() { return query.includes('reduced-motion') && matches },
    addEventListener: (_event: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_event: string, listener: () => void) => listeners.delete(listener),
  }) as unknown as MediaQueryList)
  const appearance = renderHook(({ motion }: { motion: MotionPreference }) => useAppearance({
    appearance: { ...DEFAULT_APPEARANCE, motion },
  } as Settings), { initialProps: { motion: 'system' as MotionPreference } })
  const consumer = renderHook(useReducedMotion)
  const check = (reduced: boolean) => {
    expect(consumer.result.current).toBe(reduced)
    expect(reducedMotion()).toBe(reduced)
    expect(document.documentElement.dataset.motion).toBe(reduced ? 'reduced' : 'full')
  }
  check(true)
  appearance.rerender({ motion: 'full' }); check(false)
  act(() => { matches = false; listeners.forEach(listener => listener()) }); check(false)
  appearance.rerender({ motion: 'reduced' }); check(true)
  appearance.rerender({ motion: 'system' }); check(false)
  act(() => { matches = true; listeners.forEach(listener => listener()) }); check(true)
  consumer.unmount(); appearance.unmount()
  expect(listeners.size).toBe(0)
})

it('full animation overrides the OS for text reveal and reduced motion releases the backlog', () => {
  vi.useFakeTimers()
  vi.spyOn(window, 'matchMedia').mockReturnValue({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as MediaQueryList)
  const appearance = renderHook(({ motion }: { motion: MotionPreference }) => useAppearance({ appearance: { ...DEFAULT_APPEARANCE, motion } } as Settings), { initialProps: { motion: 'full' as MotionPreference } })
  const reveal = renderHook(({ text }) => useReplyReveal(text, true), { initialProps: { text: '' } })
  const text = 'Words '.repeat(100)
  reveal.rerender({ text })
  expect(reveal.result.current.revealing).toBe(true)
  appearance.rerender({ motion: 'reduced' })
  expect(reveal.result.current).toEqual({ text, revealing: false })
  expect(vi.getTimerCount()).toBe(0)
})
