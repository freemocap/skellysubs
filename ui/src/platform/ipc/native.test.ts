import { afterEach, beforeEach, expect, it, vi } from 'vitest'
const mocks = vi.hoisted(() => ({ call: vi.fn(), diagnostic: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.call }))
vi.mock('../diagnostics/log', () => ({ logDiagnostic: mocks.diagnostic }))
import { invoke } from './native'
beforeEach(() => vi.resetAllMocks())
afterEach(()=>vi.unstubAllGlobals())
it('reports recorder boundaries without arguments or results on success and failure',async()=>{
  const finish=vi.fn()
  const probe=vi.fn(()=>finish)
  vi.stubGlobal('__workflowIpc',probe)
  mocks.call.mockResolvedValueOnce('private result')
  expect(await invoke('get_snapshot',{private:'source'})).toBe('private result')
  expect(probe).toHaveBeenCalledExactlyOnceWith('get_snapshot')
  expect(finish).toHaveBeenCalledOnce()
  const failure={code:'validation',message:'private failure'}
  mocks.call.mockRejectedValueOnce(failure)
  await expect(invoke('get_snapshot')).rejects.toBe(failure)
  expect(finish).toHaveBeenCalledTimes(2)
  expect(JSON.stringify(probe.mock.calls)).not.toContain('private')
})
it('records confirmed caller cancellations as info while retaining failures', async () => {
  const controller = new AbortController()
  controller.abort()
  const failure = { code: 'conflict', diagnostics: { error: { diagnostics: { reason: 'cancelled' } } } }
  mocks.call.mockRejectedValue(failure)
  await expect(invoke('run_reading', { id: 'reading' }, controller.signal)).rejects.toBe(failure)
  expect(mocks.diagnostic).toHaveBeenLastCalledWith('application', failure, undefined, 'native_command_failed', 'info', { command: 'run_reading' })
  await expect(invoke('run_reading', { id: 'reading' })).rejects.toBe(failure)
  expect(mocks.diagnostic).toHaveBeenLastCalledWith('application', failure, undefined, 'native_command_failed', 'error', { command: 'run_reading' })
  const changed = { code: 'conflict', diagnostics: { error: { diagnostics: { reason: 'access_changed' } } } }
  mocks.call.mockRejectedValue(changed)
  await expect(invoke('run_reading', { id: 'reading' }, controller.signal)).rejects.toBe(changed)
  expect(mocks.diagnostic).toHaveBeenLastCalledWith('application', changed, undefined, 'native_command_failed', 'error', { command: 'run_reading' })
})
it('delivers the original rejection while diagnostic persistence is pending', async () => {
  const failure = { code: 'validation', message: 'PRIVATE' }
  mocks.call.mockRejectedValue(failure)
  let ack!: (value: boolean) => void
  mocks.diagnostic.mockReturnValue(new Promise(resolve => { ack = resolve }))
  let caught = false
  const request = invoke('mic_start', { conversationId: 'PRIVATE' }).catch(error => { caught = true; return error })
  await vi.waitFor(() => expect(caught).toBe(true))
  expect(mocks.diagnostic).toHaveBeenCalledWith('microphone', failure, undefined, 'native_command_failed', 'error', { command: 'mic_start' })
  ack(true)
  expect(await request).toBe(failure)
})
