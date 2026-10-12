import { useExecutionPreferences } from './useExecutionPreferences'
// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { AiExecutionSettings } from '../../components/controls/AiExecutionSettings'
import { useSettingsStore } from './settings'
import { DEFAULT_EXECUTION } from '../../generated/contracts'
import type { Settings } from '../../types'

function Controls() { return <AiExecutionSettings {...useExecutionPreferences()} /> }
const save = vi.fn()
const events = vi.hoisted(() => ({ notify: vi.fn().mockResolvedValue(undefined), listen: vi.fn().mockResolvedValue(() => {}) }))
vi.mock('../../platform/ipc/execution-preference-events', () => ({ executionPreferencesChanged: events.notify, onExecutionPreferencesChanged: events.listen }))
beforeEach(() => {
  save.mockReset()
  events.notify.mockClear(); events.listen.mockClear()
  useSettingsStore.setState({ settings: { execution: { ...DEFAULT_EXECUTION } } as Settings, save, load: vi.fn().mockResolvedValue(undefined) })
})
it('shares saved values across both mounted surfaces without writing on open', async () => {
  save.mockImplementation(async (next: Settings) => { useSettingsStore.setState({ settings: next }); return next })
  render(<><Controls /><Controls /></>)
  for (const summary of screen.getAllByText('Automatic AI work')) fireEvent.click(summary)
  expect(save).not.toHaveBeenCalled()
  const controls = screen.getAllByRole('combobox', { name: 'Message assessment' })
  expect(screen.queryByRole('combobox', { name: 'Coaching feedback' })).not.toBeInTheDocument()
  expect(screen.getAllByText('Coaching feedback', { exact: false })).toHaveLength(2)
  expect(controls[0]).toHaveValue('automatic')
  fireEvent.change(controls[0], { target: { value: 'on_demand' } })
  await waitFor(() => expect(controls[1]).toHaveValue('on_demand'))
  expect(save).toHaveBeenCalledOnce()
  expect(events.notify).toHaveBeenCalledOnce()
  expect(save.mock.calls[0][0].execution).toEqual({ ...DEFAULT_EXECUTION, assessment: 'on_demand' })
  expect(save.mock.calls[0][1].execution).toEqual(DEFAULT_EXECUTION)
})
it('re-reads native settings when another window announces a change', async () => {
  const load = vi.fn().mockImplementation(async () => { useSettingsStore.setState({ settings: { execution: { ...DEFAULT_EXECUTION, reading: 'automatic' } } as Settings }) })
  useSettingsStore.setState({ load })
  render(<Controls />)
  expect(events.listen).toHaveBeenCalledOnce()
  await act(async () => { events.listen.mock.calls[0][0]() })
  await waitFor(() => expect(useSettingsStore.getState().settings?.execution?.reading).toBe('automatic'))
  expect(load).toHaveBeenCalledOnce()
})
it('shows a failed save without claiming the requested value was saved', async () => {
  save.mockRejectedValue(new Error('Fixture save failed'))
  render(<Controls />)
  fireEvent.click(screen.getByText('Automatic AI work'))
  const control = screen.getByRole('combobox', { name: 'Reading support' })
  expect(control).toHaveValue(DEFAULT_EXECUTION.reading)
  const requested = DEFAULT_EXECUTION.reading === 'automatic' ? 'on_demand' : 'automatic'
  fireEvent.change(control, { target: { value: requested } })
  await waitFor(() => expect(screen.getByText('Fixture save failed')).toBeVisible())
  expect(control).toHaveValue(DEFAULT_EXECUTION.reading)
  expect(control).not.toBeDisabled()
})
