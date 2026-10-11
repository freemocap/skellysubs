// @vitest-environment jsdom
import { StrictMode } from 'react'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { AnalysisContent, type AnalysedTurn } from './AnalysisContent'
const help = vi.hoisted(() => ({ requestReplyHelp: vi.fn() }))
vi.mock('../composer/TurnReplyHelp', () => help)
beforeEach(() => { help.requestReplyHelp.mockReset().mockResolvedValue(undefined) })
const turn = (state: string | null = null) => ({ id: 1, user: null, turnId: 'turn', analysisState: null, assistant: {
  messageId: 'reply', reply: 'Hola.', tokens: [], user_tokens: [], errors: [], mechanics: [],
  help: { lanes: { grammar: { state } }, grammar: null },
} }) as unknown as AnalysedTurn
const view = (value: AnalysedTurn) => <StrictMode><AnalysisContent conversationId="chat" turn={value} nativeLanguageName="English" showRomanization={false} rtl={false} /></StrictMode>
it('opening analysis requests grammar once, including strict-effect replay and rerenders', async () => {
  const { rerender } = render(view(turn()))
  await waitFor(() => expect(help.requestReplyHelp).toHaveBeenCalledExactlyOnceWith('reply', 'grammar'))
  rerender(view(turn()))
  expect(help.requestReplyHelp).toHaveBeenCalledTimes(1)
  expect(screen.queryByRole('button', { name: 'Explain grammar' })).toBeNull()
  expect(screen.getByText('Working out the grammar…')).toBeVisible()
})
it('reuses saved grammar and does not automatically retry a failed request', async () => {
  const { rerender } = render(view(turn('succeeded')))
  expect(help.requestReplyHelp).not.toHaveBeenCalled()
  rerender(view(turn('failed')))
  expect(help.requestReplyHelp).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Retry' }))
  await waitFor(() => expect(help.requestReplyHelp).toHaveBeenCalledExactlyOnceWith('reply', 'grammar', true))
})


it('renders saved grammar immediately when message analysis opens without any request', () => {
  const saved = turn('succeeded')
  const card = { title: 'Saved construction', quote: '', body: 'Already available explanation.', example: '', contrast: '', cefr: null }
  saved.assistant!.mechanics = [card]
  render(view(saved))
  expect(screen.getByText('Already available explanation.')).toBeVisible()
  expect(screen.queryByText('Working out the grammar…')).toBeNull()
  expect(help.requestReplyHelp).not.toHaveBeenCalled()
})
