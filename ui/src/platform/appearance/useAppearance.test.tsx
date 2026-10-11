// @vitest-environment jsdom
import { renderHook } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { DEFAULT_APPEARANCE } from '../../generated/contracts'
import { useAppearance } from './useAppearance'
import type { Settings } from '../../types'

it('applies theme and palette globally', () => {
  const settings = { theme: 'dark', appearance: { ...DEFAULT_APPEARANCE } } as Settings
  const view = renderHook(({ settings }) => useAppearance(settings), { initialProps: { settings } })
  const root = document.documentElement
  expect(root.dataset).toMatchObject({ theme: 'dark', palette: 'cool' })
  view.rerender({ settings: { ...settings, theme: 'light', appearance: { palette: 'warm', motion: 'system' as const } } })
  expect(root.dataset).toMatchObject({ theme: 'light', palette: 'warm' })
})

it('tracks system appearance and removes the listener on unmount', () => {
  const media = { matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() }
  const spy = vi.spyOn(window, 'matchMedia').mockReturnValue(media as unknown as MediaQueryList)
  try {
    const view = renderHook(() => useAppearance({ theme: 'system' } as Settings))
    expect(document.documentElement.dataset.theme).toBe('dark')
    media.matches = false
    media.addEventListener.mock.calls.forEach(([, listener]) => listener())
    expect(document.documentElement.dataset.theme).toBe('light')
    view.unmount()
    expect(media.removeEventListener).toHaveBeenCalledWith('change', media.addEventListener.mock.calls[0][1])
  } finally { spy.mockRestore() }
})
