// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { ConversationSnapshot, TurnView } from '../../../generated/contracts'
import { replyState } from '../../../domain/conversation/reply-state'
import { ReplyStatus } from './ReplyStatus'
function execution(state: string, turnState = state): TurnView {
  return { id: 'turn', state: turnState, paused: false, hold: state === 'failed' ? { message: 'Provider rejected reply' } : null, operations: [], attempts: [] } as unknown as TurnView
}
function project(turn: TurnView, paused = false) {
  return replyState(turn, { turns: [turn], connection: { paused } } as ConversationSnapshot)
}
it.each(['failed', 'unknown', 'cancelled', 'invalidated', 'succeeded'])('does not show progress for terminal reply %s', state => {
  render(<ReplyStatus reply={project(execution(state))} />)
  expect(screen.queryByText('Replying…')).toBeNull()
  expect(screen.queryByText('Independent coach error')).toBeNull()
})
it('treats holds and both pause gates as stopped, even while turn state remains pending', () => {
  const held = execution('ready', 'pending'); held.hold = { message: 'Budget held' } as TurnView['hold']
  expect(project(held)).toMatchObject({ state: 'held', error: 'Budget held', control: 'resume' })
  const paused = execution('ready', 'pending'); paused.paused = true
  expect(project(paused)).toMatchObject({ state: 'paused', control: 'resume' })
  expect(project(paused, true)).toMatchObject({ state: 'paused', control: null })
  expect(project(execution('running', 'pending'))).toMatchObject({ state: 'pending', control: null })
  expect(replyState(undefined, { turns: [], connection: { paused: false } } as unknown as ConversationSnapshot)).toMatchObject({ state: 'unavailable', control: null })
})
it('explains a hold for spent credits in plain words, with the recorded reason folded and adding credits marked as coming', () => {
  const recorded = 'Your remaining daily allowance cannot cover this request. Wait for pending requests to finish or for the 00:00 UTC reset.'
  const held = execution('ready', 'pending'); held.hold = { message: recorded } as TurnView['hold']
  render(<ReplyStatus reply={project(held)} />)
  expect(screen.getByText('Partner reply is held.')).toBeVisible()
  expect(screen.getByText("You're out of credits for today")).toBeVisible()
  expect(screen.getByText(/^You get new credits at \d{1,2}:\d{2}/)).toBeVisible()
  expect(screen.getByText(recorded)).not.toBeVisible()
  expect(screen.getByRole('button', { name: 'Add credits · Coming soon' })).toBeDisabled()
  fireEvent.click(screen.getByText('Technical details'))
  expect(screen.getByText(recorded)).toBeVisible()
})
it('shows a readable failure reason as recorded, with nothing folded', () => {
  render(<ReplyStatus reply={project(execution('failed'))} />)
  expect(screen.getByText('Provider rejected reply')).toBeVisible()
  expect(screen.queryByText('Technical details')).toBeNull()
})
it('does not offer retry for an earlier exchange or cancelled work', () => {
  const turn = execution('failed')
  expect(replyState(turn, { turns: [execution('running', 'pending'), turn].map((item, i) => ({ ...item, id: i ? 'turn' : 'later' })), connection: { paused: false } } as ConversationSnapshot).control).toBeNull()
  expect(project(execution('cancelled')).control).toBeNull()
})
it('retries explicitly once, surfaces admission rejection, and only displays progress from a subsequent native state', async () => {
  let reject!: (reason: Error) => void
  const control = vi.fn(() => new Promise<void>((_, no) => { reject = no }))
  const view = render(<ReplyStatus reply={project(execution('failed'))} onControl={control} />)
  expect(control).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Retry exchange' }))
  fireEvent.click(screen.getByRole('button', { name: 'Retry exchange' }))
  expect(control).toHaveBeenCalledExactlyOnceWith('retry')
  expect(screen.queryByText('Replying…')).toBeNull()
  await act(async () => reject(new Error('Connection changed')))
  await waitFor(() => expect(screen.getByText('Connection changed')).toBeVisible())
  view.rerender(<ReplyStatus reply={project(execution('running', 'pending'))} onControl={control} />)
  expect(screen.getByText('Replying…')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Retry exchange' })).toBeNull()
})
it('gives a pending reply the landed bubble shape: a reading line and an action footer', () => {
  const view = render(<ReplyStatus reply={project(execution('running', 'pending'))} />)
  const bubble = view.container.querySelector('.msg.bot') as HTMLElement
  expect(bubble).toHaveClass('with-actions')
  expect(bubble.querySelector('.reply-placeholder')).not.toBeNull()
  expect(bubble.querySelector('.message-actions.reply-activity')).toHaveTextContent('Replying…')
  view.rerender(<ReplyStatus reply={project(execution('running', 'pending'))} stream={{ text: 'Hola' } as never} />)
  expect(view.container.querySelector('.msg.bot')).toBe(bubble)
  expect(bubble.querySelector('.reply-received.target-text')).toHaveTextContent('Hola')
  expect(screen.getByRole('status')).toHaveTextContent('Receiving reply…')
  expect(screen.queryByText('Replying…')).toBeNull()
})


it('shows a loading bubble until the native reply settles', () => {
  const turn = { ...execution('running', 'pending'), nativeExecutionAvailable: true, channel: 'persona_reply', operations: [], attempts: [] }
  const view = render(<ReplyStatus reply={project(turn)} />)
  expect(screen.getByText('Replying…')).toBeInTheDocument()
  expect(screen.queryByText('Partner reply is unavailable.')).toBeNull()
  view.rerender(<ReplyStatus reply={project({ ...turn, state: 'failed', hold: { message: 'Provider rejected reply' } as TurnView['hold'] })} />)
  expect(screen.getByText('Partner reply failed.')).toBeVisible()
  expect(screen.getByText('Provider rejected reply')).toBeVisible()
  expect(screen.queryByText('Replying…')).toBeNull()
})
