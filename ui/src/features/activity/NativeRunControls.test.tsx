// @vitest-environment jsdom
import { beforeEach, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { TurnView } from '../../generated/contracts'
import { NativeRunControls } from './NativeRunControls'

const api = vi.hoisted(() => ({ readWorkspace: vi.fn(), executeAction: vi.fn() }))
vi.mock('../../platform/ipc/workspace', () => ({ ...api, nativeError: String }))
const turn = { id: 'run', state: 'pending', paused: true, hold: null, operations: [], attempts: [],
  nativeGraph: { step_available: true, stepping: null } } as unknown as TurnView

beforeEach(() => {
  vi.resetAllMocks()
  api.readWorkspace.mockResolvedValue({ sessionId: 'workspace' })
  api.executeAction.mockResolvedValue({})
})

it('sends a single native step command without deriving readiness from operations', async () => {
  render(<NativeRunControls turn={turn} globallyPaused={false} />)
  fireEvent.click(screen.getByRole('button', { name: 'Step' }))
  await waitFor(() => expect(api.executeAction).toHaveBeenCalledWith({ sessionId: 'workspace' }, { kind: 'controlTurn', turnId: 'run', control: 'step' }))
  expect(api.executeAction).toHaveBeenCalledTimes(1)
})

it('uses native eligibility and respects global pause and refusal holds', () => {
  const { rerender } = render(<NativeRunControls turn={turn} globallyPaused />)
  expect(screen.getByRole('button', { name: 'Step' })).toBeDisabled()
  rerender(<NativeRunControls turn={{ ...turn, paused: false }} globallyPaused={false} />)
  expect(screen.getByRole('button', { name: 'Step' })).toBeDisabled()
  rerender(<NativeRunControls turn={{ ...turn, nativeGraph: { ...turn.nativeGraph!, step_available: false, stepping: 'first' } }} globallyPaused={false} />)
  expect(screen.getByRole('button', { name: 'Step' })).toBeDisabled()
  rerender(<NativeRunControls turn={{ ...turn, hold: { code: 'conflict', message: 'held', refusal: null } }} globallyPaused={false} />)
  expect(screen.getByRole('button', { name: 'Step' })).toBeDisabled()
})

it('keeps command failures visible and permits retrying the command', async () => {
  api.executeAction.mockRejectedValue(new Error('Rejected step'))
  render(<NativeRunControls turn={turn} globallyPaused={false} />)
  fireEvent.click(screen.getByRole('button', { name: 'Step' }))
  await waitFor(() => expect(screen.getByText('Error: Rejected step')).toBeVisible())
  expect(screen.getByRole('button', { name: 'Step' })).toBeEnabled()
})

it('does not offer execution controls for retained graphs without executable capabilities', () => {
  render(<NativeRunControls turn={{ ...turn, nativeExecutionAvailable: false }} globallyPaused={false} />)
  expect(screen.queryByRole('button', { name: 'Step' })).toBeNull()
})


it('offers explicit retry for a failed native branch even after the exchange stopped', async () => {
  render(<NativeRunControls turn={{ ...turn, state: 'failed', nativeGraph: { ...turn.nativeGraph!, nodes: { helper: 'Failed' } } }} globallyPaused={false} />)
  expect(api.executeAction).not.toHaveBeenCalled()
  expect(screen.queryByRole('button', { name: 'Step' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
  await waitFor(() => expect(api.executeAction).toHaveBeenCalledExactlyOnceWith({ sessionId: 'workspace' }, { kind: 'controlTurn', turnId: 'run', control: 'retry' }))
})


it.each(['cancelled', 'invalidated'])('does not offer retry for a %s source', state => {
  render(<NativeRunControls turn={{ ...turn, state, nativeGraph: { ...turn.nativeGraph!, nodes: { helper: 'Failed' } } }} globallyPaused={false} />)
  expect(screen.queryByRole('button')).toBeNull()
  expect(api.executeAction).not.toHaveBeenCalled()
})
