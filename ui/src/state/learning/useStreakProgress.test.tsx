// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { useStreakProgress } from './useStreakProgress'
import { refreshStreaks } from '../../platform/ipc/streaks'
import type { StreakSnapshot } from '../../generated/contracts'
vi.mock('../../platform/ipc/streaks', () => ({ refreshStreaks: vi.fn() }))
const snapshot = (sessionId = 's', count = 1): StreakSnapshot => ({ sessionId, today: 100, app: { count, day: 100, celebration: 'app:100' } })
beforeEach(() => {
  vi.mocked(refreshStreaks).mockReset().mockImplementation(async session => snapshot(session))
  vi.spyOn(document, 'hasFocus').mockReturnValue(true)
})
afterEach(() => { vi.restoreAllMocks(); vi.useRealTimers() })

it('refreshes foreground app activity and exposes only its claimed celebration', async () => {
  const { result } = renderHook(() => useStreakProgress('s', true))
  await waitFor(() => expect(result.current.value?.app.count).toBe(1))
  expect(result.current.value?.app.celebration).toBe('app:100')
  vi.mocked(refreshStreaks).mockResolvedValue(snapshot('s', 2))
  act(() => window.dispatchEvent(new Event('focus')))
  await waitFor(() => expect(result.current.value?.app.count).toBe(2))
  expect(result.current.value?.app.celebration).toBe('app:100')
  expect(refreshStreaks).toHaveBeenLastCalledWith('s', true)
})

it('ignores late responses after workspace switching and exposes read failures', async () => {
  let finish!: (value: StreakSnapshot) => void
  vi.mocked(refreshStreaks).mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const { result, rerender } = renderHook(({ session }) => useStreakProgress(session, true), { initialProps: { session: 's' } })
  rerender({ session: 'new' })
  await waitFor(() => expect(result.current.value?.sessionId).toBe('new'))
  await act(async () => finish(snapshot()))
  expect(result.current.value?.sessionId).toBe('new')
  vi.mocked(refreshStreaks).mockRejectedValue(new Error('Read failed'))
  act(() => window.dispatchEvent(new Event('focus')))
  await waitFor(() => expect(result.current.error).toContain('Read failed'))
  expect(result.current.value?.sessionId).toBe('new')
})

it('coalesces pending events, preserving a foreground signal received during a read', async () => {
  let finish!: (value: StreakSnapshot) => void
  vi.mocked(refreshStreaks).mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  renderHook(() => useStreakProgress('s', true))
  act(() => { window.dispatchEvent(new Event('focus')); window.dispatchEvent(new Event('focus')); window.dispatchEvent(new Event('focus')) })
  expect(refreshStreaks).toHaveBeenCalledTimes(1)
  await act(async () => finish(snapshot()))
  expect(refreshStreaks).toHaveBeenCalledTimes(2)
  expect(refreshStreaks).toHaveBeenLastCalledWith('s', true)
})

it('timers never record activity; effects disabled still consume claims without replay', async () => {
  vi.useFakeTimers()
  const { result, rerender, unmount } = renderHook(({ effects }) => useStreakProgress('s', effects), { initialProps: { effects: false } })
  await act(async () => {})
  await act(async () => vi.advanceTimersByTime(30000))
  expect(refreshStreaks).toHaveBeenLastCalledWith('s', false)
  expect(result.current.value?.app.celebration).toBeNull()
  rerender({ effects: true })
  expect(result.current.value?.app.celebration).toBeNull()
  unmount()
  await act(async () => vi.advanceTimersByTime(30000))
  expect(refreshStreaks).toHaveBeenCalledTimes(2)
})

it('rejects a response from a replacement workspace', async () => {
  vi.mocked(refreshStreaks).mockResolvedValue({ ...snapshot(), sessionId: 'other' })
  const { result } = renderHook(() => useStreakProgress('s', true))
  await waitFor(() => expect(result.current.error).toContain('scope mismatch'))
  expect(result.current.value).toBeNull()
})

it('records the first interaction after midnight but not unattended or hidden polling', async () => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date(2026, 9, 11, 12))
  const { unmount } = renderHook(() => useStreakProgress('s', true))
  await act(async () => {})
  await act(async () => document.dispatchEvent(new Event('pointerdown')))
  const before = vi.mocked(refreshStreaks).mock.calls.length
  await act(async () => document.dispatchEvent(new Event('keydown')))
  expect(refreshStreaks).toHaveBeenCalledTimes(before)
  vi.setSystemTime(new Date(2026, 9, 12, 0, 1))
  await act(async () => vi.advanceTimersByTime(30000))
  expect(refreshStreaks).toHaveBeenLastCalledWith('s', false)
  await act(async () => document.dispatchEvent(new Event('keydown')))
  expect(refreshStreaks).toHaveBeenLastCalledWith('s', true)
  vi.mocked(document.hasFocus).mockReturnValue(false)
  const focusedCalls = vi.mocked(refreshStreaks).mock.calls.length
  await act(async () => { document.dispatchEvent(new Event('pointerdown')); await vi.advanceTimersByTimeAsync(30000) })
  expect(refreshStreaks).toHaveBeenCalledTimes(focusedCalls)
  unmount()
})

it('allows the next genuine interaction to retry a failed activity write', async () => {
  const { result } = renderHook(() => useStreakProgress('s', true))
  await waitFor(() => expect(result.current.value).not.toBeNull())
  vi.mocked(refreshStreaks).mockRejectedValueOnce(new Error('Write failed'))
  act(() => document.dispatchEvent(new Event('pointerdown')))
  await waitFor(() => expect(result.current.error).toContain('Write failed'))
  act(() => document.dispatchEvent(new Event('pointerdown')))
  await waitFor(() => expect(result.current.error).toBeNull())
  expect(refreshStreaks).toHaveBeenLastCalledWith('s', true)
})
