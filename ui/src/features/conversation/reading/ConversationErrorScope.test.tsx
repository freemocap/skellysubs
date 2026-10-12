// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import type { TurnView } from '../../../generated/contracts'
import { ErrorDetails } from '../../../components/feedback/ErrorDetails'
import { useNavigationStore } from '../../../state/navigation/navigation'
import { ConversationErrorScope } from './ConversationErrorScope'

it('opens the originating exchange without guessing a node from error text', () => {
  const turn = { id: 'older-turn', state: 'failed', nativeGraph: { nodes: { gloss: 'Failed', help: 'Failed' } } } as unknown as TurnView
  const close = vi.fn()
  render(<ConversationErrorScope conversationId="conversation" turn={turn} onInspect={close}>
    <ErrorDetails label="Reply ideas" errorKey='["romanization is not in Latin script"]'>romanization is not in Latin script</ErrorDetails>
  </ConversationErrorScope>)
  fireEvent.click(screen.getByText('Technical details'))
  fireEvent.click(screen.getByRole('button', {name:'Open AI activity'}))
  expect(close).toHaveBeenCalledOnce()
  expect(useNavigationStore.getState().overlay).toBe('activity')
  expect(useNavigationStore.getState().aiInspection).toEqual({conversationId:'conversation',turnId:'older-turn',operationKind:null})
})

it.each(['engine', 'another-engine'])('selects a node only when diagnostic ownership matches: %s', engine => {
  const turn = { id: 'turn', state: 'succeeded', nativeGraph: { engine: 'engine', run: 'run', nodes: { help: 'Failed' } } } as unknown as TurnView
  const error = JSON.stringify({ diagnostics: { engine, run: 'run', node: 'help' } })
  render(<ConversationErrorScope conversationId="conversation" turn={turn}>
    <ErrorDetails label="Help failure" errorKey={error}>{error}</ErrorDetails>
  </ConversationErrorScope>)
  fireEvent.click(screen.getByText('Technical details'))
  fireEvent.click(screen.getByRole('button', { name: 'Open AI activity' }))
  expect(useNavigationStore.getState().aiInspection?.operationKind).toBe(engine === 'engine' ? 'help' : null)
})
