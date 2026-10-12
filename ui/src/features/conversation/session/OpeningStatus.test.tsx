// @vitest-environment jsdom
import { render, screen } from '@testing-library/react'
import { expect, it } from 'vitest'
import type { ConversationSnapshot } from '../../../generated/contracts'
import { OpeningStatus } from './OpeningStatus'

it('keeps a pending bubble while native opening admission and execution arrive', () => {
  const snapshot = { opening: { kind: 'partner' }, turns: [], connection: { paused: false } } as unknown as ConversationSnapshot
  const view = render(<OpeningStatus snapshot={snapshot} onActivity={() => {}} />)
  expect(screen.getByText('Replying…')).toBeInTheDocument()
  expect(screen.queryByRole('alert')).toBeNull()
  const turn = { replacesTurnId: null, replacedBy: null, route: 'hosted', hold: null, id: 'opening', channel: 'persona_opening', state: 'pending', paused: false, operations: [], attempts: [], nativeExecutionAvailable: true }
  view.rerender(<OpeningStatus snapshot={{ ...snapshot, turns: [turn] } as ConversationSnapshot} onActivity={() => {}} />)
  expect(screen.getByText('Replying…')).toBeInTheDocument()
  view.rerender(<OpeningStatus snapshot={{ ...snapshot, turns: [{ ...turn, state: 'failed' }] } as ConversationSnapshot} onActivity={() => {}} />)
  expect(screen.getByText('Conversation opening failed.')).toBeVisible()
  expect(screen.queryByText('Replying…')).toBeNull()
})
