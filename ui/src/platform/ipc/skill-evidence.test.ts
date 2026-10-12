import { expect, it, vi } from 'vitest'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('./tauri', () => ({ invoke }))
import { getLanguageTotals } from './skill-evidence'

it('three totals consumers share one dispatch, but a later invalidation reads afresh while pending', async () => {
  let finish!: (value: unknown[]) => void
  invoke.mockReturnValueOnce(new Promise(resolve => { finish = resolve })).mockResolvedValue([])
  const first = getLanguageTotals()
  const second = getLanguageTotals()
  const third = getLanguageTotals()
  await Promise.resolve()
  expect(invoke).toHaveBeenCalledTimes(1)
  await getLanguageTotals()
  expect(invoke).toHaveBeenCalledTimes(2)
  finish([])
  expect(await first).toEqual([])
  expect(await second).toEqual([])
  expect(await third).toEqual([])
  invoke.mockRejectedValueOnce(new Error('Read failed'))
  await expect(getLanguageTotals()).rejects.toThrow('Read failed')
  await expect(getLanguageTotals()).resolves.toEqual([])
})
