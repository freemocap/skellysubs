// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { DEFAULT_APPEARANCE } from '../../generated/contracts'
import { useSettingsStore } from '../../state/settings/settings'
import type { Settings } from '../../types'
import { ThemeControls } from './ThemeControls'

const update = vi.fn()
beforeEach(() => {
  update.mockReset()
  useSettingsStore.setState({ settings: { theme: 'light', appearance: { ...DEFAULT_APPEARANCE } } as Settings, update })
})

it('switches between light and dark and applies the change to the current settings', () => {
  render(<ThemeControls />)
  const toggle = screen.getByRole('button', { name: 'Dark theme' })
  expect(toggle).toHaveAttribute('aria-pressed', 'false')
  fireEvent.click(toggle)
  const change = update.mock.calls[0][0] as (current: Settings) => Settings
  expect(change({ theme: 'light', appearance: { palette: 'cool', motion: 'system' as const } } as Settings)).toMatchObject({ theme: 'dark' })
})

it('opens Cool/Warm from the arrow beside the switch and changes the palette', () => {
  render(<ThemeControls />)
  expect(screen.queryByRole('menu')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Surface palette' }))
  expect(screen.getByRole('menuitemradio', { name: 'Cool neutral' })).toHaveAttribute('aria-checked', 'true')
  fireEvent.click(screen.getByRole('menuitemradio', { name: 'Warm' }))
  expect(screen.queryByRole('menu')).toBeNull()
  const change = update.mock.calls[0][0] as (current: Settings) => Settings
  expect(change({ theme: 'dark', appearance: { palette: 'cool', motion: 'system' as const } } as Settings)).toMatchObject({ theme: 'dark', appearance: { palette: 'warm', motion: 'system' as const } })
})

it('shows nothing before settings load', () => {
  useSettingsStore.setState({ settings: null })
  const view = render(<ThemeControls />)
  expect(view.container).toBeEmptyDOMElement()
})
