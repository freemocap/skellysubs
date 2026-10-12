// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { RunHistory } from '../../../generated/graph-contracts'
import { pipelineSnapshot } from '../../../../tests/fixtures/graph-artifacts'
import { useRunHistory } from './useRunHistory'

const page = (revisions: string[], before: string | null, current = revisions.at(-1)!): RunHistory => ({
  engine: 'engine-1', run: 'run-1', current_revision: current, before,
  frames: revisions.map(revision => pipelineSnapshot({ revision })),
})

it('fills intervening history before joining a refresh to previously loaded frames', async () => {
  const load = vi.fn().mockResolvedValueOnce(page(['1', '2'], null))
    .mockResolvedValueOnce(page(['5', '6'], '5'))
    .mockResolvedValueOnce(page(['3', '4'], '3', '6'))
    .mockResolvedValueOnce(page(['1', '2'], null, '6'))
  const { result, rerender } = renderHook(({ revision }) => useRunHistory('engine-1', 'run-1', 'artifact-pipeline', load, true, true, revision), { initialProps: { revision: '2' } })
  await waitFor(() => expect(result.current.history?.frames).toHaveLength(2))
  rerender({ revision: '6' })
  await waitFor(() => expect(result.current.history?.frames.map(frame => frame.revision)).toEqual(['1', '2', '3', '4', '5', '6']))
  expect(load.mock.calls.map(args => args[0])).toEqual([null, null, '5', '3'])
})

it('starts the new scope immediately and ignores an abandoned response', async () => {
  let resolve!: (page: RunHistory) => void
  const load = vi.fn().mockReturnValueOnce(new Promise<RunHistory>(done => { resolve = done }))
    .mockResolvedValueOnce({ ...page(['2'], null), run: 'other', frames: [{ ...pipelineSnapshot({ revision: '2' }), run: 'other' }] })
  const { result, rerender } = renderHook(({ run }) => useRunHistory('engine-1', run, 'artifact-pipeline', load, true, true, '2'), { initialProps: { run: 'run-1' } })
  rerender({ run: 'other' })
  await waitFor(() => expect(result.current.history?.run).toBe('other'))
  await act(async () => resolve(page(['1'], null)))
  expect(result.current.history?.run).toBe('other')
  expect(result.current.loading).toBe(false)
})

it('reports an invalid page before following its cursor', async () => {
  const load = vi.fn().mockResolvedValueOnce(page(['1'], null))
    .mockResolvedValueOnce({ ...page(['4'], '4'), run: 'wrong' })
  const { result, rerender } = renderHook(({ revision }) => useRunHistory('engine-1', 'run-1', 'artifact-pipeline', load, true, true, revision), { initialProps: { revision: '1' } })
  await waitFor(() => expect(result.current.history?.frames).toHaveLength(1))
  rerender({ revision: '4' })
  await waitFor(() => expect(result.current.error).toMatch(/selected run/))
  expect(load).toHaveBeenCalledTimes(2)
  expect(result.current.history?.frames).toHaveLength(1)
})
