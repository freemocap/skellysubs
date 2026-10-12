import { beforeEach, expect, it, vi } from 'vitest'
import { recordBotInspection } from './effort'
const native = vi.hoisted(() => ({ invoke: vi.fn(), publish: vi.fn(), changed: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }))
vi.mock('./reward-origin', () => ({ publishEffortAwards: native.publish }))
vi.mock('./effort-events', () => ({ effortPublished: native.changed }))
beforeEach(() => vi.clearAllMocks())
it('sends only selection identity and keeps an already credited read quiet', async () => {
  native.invoke.mockResolvedValue(null)
  await recordBotInspection({ engine: 'e', run: 'r', revision: '8', node: 'reply', attempt: '2', ...{ retainedText: 'private content', evidence: { observations: [] } } })
  expect(native.invoke).toHaveBeenCalledWith('record_bot_inspection', { engine: 'e', run: 'r', revision: '8', node: 'reply', attempt: '2' })
  expect(native.publish).not.toHaveBeenCalled()
  expect(native.changed).not.toHaveBeenCalled()
})
it('publishes an earned inspection award and surfaces failures', async () => {
  const selection = { engine: 'e', run: 'r', revision: '8', node: 'reply', attempt: '2' }
  native.invoke.mockResolvedValueOnce({ id: 'award' }).mockRejectedValueOnce(new Error('credit failed'))
  await recordBotInspection(selection)
  expect(native.publish).toHaveBeenCalledWith([{ id: 'award' }])
  expect(native.changed).toHaveBeenCalledOnce()
  await expect(recordBotInspection(selection)).rejects.toThrow('credit failed')
  expect(native.changed).toHaveBeenCalledOnce()
})
