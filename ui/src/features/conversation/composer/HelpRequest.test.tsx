// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { ChatMessage, TurnView } from '../../../generated/contracts'
import { replyHelp } from '../../../domain/conversation/reply-help'
import { useHelpRequest } from './HelpRequest'
it('ends native helper progress when the saved result arrives without legacy attempts', async () => {
  const turn = { operations: [], attempts: [], state: 'succeeded' } as unknown as TurnView
  const message = { explanationsState: null } as unknown as ChatMessage
  const request = vi.fn().mockResolvedValue(undefined)
  const view = renderHook(({ message }) => useHelpRequest(replyHelp(message, turn).lanes.grammar, request), { initialProps: { message } })
  await act(async () => view.result.current.toggle())
  expect(view.result.current.pending).toBe(true)
  view.rerender({ message: { ...message, explanationsState: 'running' } })
  expect(view.result.current.pending).toBe(true)
  view.rerender({ message: { ...message, explanationsState: 'succeeded', replyExplanations: { cards: [] } } })
  expect(view.result.current.pending).toBe(false)
  expect(view.result.current.shown).toBe(true)
  expect(request).toHaveBeenCalledOnce()
})
