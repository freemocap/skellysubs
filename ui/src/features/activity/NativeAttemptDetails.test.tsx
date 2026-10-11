// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import type { InspectionSnapshot } from '../../generated/graph-contracts'
import { NativeAttemptDetails } from './NativeAttemptDetails'
const api = vi.hoisted(() => ({ readNativeGraphAttempt: vi.fn(), recordBotInspection: vi.fn().mockResolvedValue(undefined) }))
vi.mock('../../platform/ipc/window', () => api)
vi.mock('../../platform/ipc/effort', () => api)
const graph = { engine: 'engine', run: 'run', artifact_id: 'artifact', revision: '8', attempts: { helper: [{ id: '1' }, { id: '2' }] } } as unknown as InspectionSnapshot
const detail = (attempt: string, revision = '8') => ({ engine: 'engine', run: 'run', artifact: 'artifact', revision, node: 'helper', attempt, execution: 'execution', owner: 'owner', evidence: null, retainedText: 'retained ' + attempt })
beforeEach(() => vi.clearAllMocks())
function open(container: HTMLElement) {
  const details = container.querySelector('details')!
  details.open = true
  fireEvent(details, new Event('toggle'))
}
it('reads only on deliberate opening and preserves explicit attempt selection', async () => {
  api.readNativeGraphAttempt.mockImplementation(async (_e, _r, _v, _n, attempt) => detail(attempt))
  const view = render(<NativeAttemptDetails graph={graph} node="helper" />)
  expect(api.readNativeGraphAttempt).not.toHaveBeenCalled()
  open(view.container)
  await screen.findByText('retained 2')
  expect(api.readNativeGraphAttempt).toHaveBeenLastCalledWith('engine', 'run', '8', 'helper', '2')
  fireEvent.change(screen.getByRole('combobox'), { target: { value: '1' } })
  await screen.findByText('retained 1')
  expect(screen.queryByText('retained 2')).toBeNull()
})
it('rejects mismatched evidence instead of displaying another execution', async () => {
  api.readNativeGraphAttempt.mockResolvedValue({ ...detail('2'), run: 'wrong' })
  const view = render(<NativeAttemptDetails graph={graph} node="helper" />)
  open(view.container)
  await screen.findByText('Graph attempt evidence does not match the selection.')
  expect(screen.queryByText('retained 2')).toBeNull()
  expect(api.recordBotInspection).not.toHaveBeenCalled()
})

it('reopens already loaded evidence at the same exact cut without another native read', async () => {
  api.readNativeGraphAttempt.mockResolvedValue(detail('2'))
  const view = render(<NativeAttemptDetails graph={graph} node="helper" />)
  open(view.container)
  await screen.findByText('retained 2')
  const details = view.container.querySelector('details')!
  details.open = false
  fireEvent(details, new Event('toggle'))
  open(view.container)
  expect(await screen.findByText('retained 2')).toBeVisible()
  expect(api.readNativeGraphAttempt).toHaveBeenCalledOnce()
})
it('discards a delayed response after the historical cut changes', async () => {
  let finish!: (value: ReturnType<typeof detail>) => void
  api.readNativeGraphAttempt.mockImplementationOnce(() => new Promise(resolve => { finish = resolve })).mockResolvedValueOnce({ ...detail('2', '9'), retainedText: 'new cut' })
  const view = render(<NativeAttemptDetails graph={graph} node="helper" />)
  open(view.container)
  await waitFor(() => expect(api.readNativeGraphAttempt).toHaveBeenCalledOnce())
  view.rerender(<NativeAttemptDetails graph={{ ...graph, revision: '9' }} node="helper" />)
  await screen.findByText('new cut')
  await act(async () => finish(detail('2')))
  expect(screen.queryByText('retained 2')).toBeNull()
  expect(screen.getByText('new cut')).toBeVisible()
})

it('can return to an attempt after another attempt read failed', async () => {
  api.readNativeGraphAttempt.mockResolvedValueOnce(detail('2')).mockRejectedValueOnce(new Error('unavailable')).mockResolvedValueOnce(detail('2'))
  const view = render(<NativeAttemptDetails graph={graph} node="helper" />)
  open(view.container)
  await screen.findByText('retained 2')
  fireEvent.change(screen.getByRole('combobox'), { target: { value: '1' } })
  await screen.findByText('unavailable')
  fireEvent.change(screen.getByRole('combobox'), { target: { value: '2' } })
  await screen.findByText('retained 2')
  expect(api.readNativeGraphAttempt).toHaveBeenCalledTimes(3)
})

it('renders evidence while inspection credit is pending and does not repeat it for another cut', async () => {
  api.readNativeGraphAttempt.mockImplementation(async (_e, _r, revision, _n, attempt) => detail(attempt, revision))
  api.recordBotInspection.mockImplementationOnce(() => new Promise(() => {}))
  const view = render(<NativeAttemptDetails graph={graph} node="helper" />)
  open(view.container)
  await screen.findByText('retained 2')
  await waitFor(() => expect(api.recordBotInspection).toHaveBeenCalledOnce())
  view.rerender(<NativeAttemptDetails graph={{ ...graph, revision: '9' }} node="helper" />)
  await waitFor(() => expect(api.readNativeGraphAttempt).toHaveBeenCalledTimes(2))
  expect(api.recordBotInspection).toHaveBeenCalledOnce()
})
