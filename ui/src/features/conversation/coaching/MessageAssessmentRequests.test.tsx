// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { MessageAssessmentRequests } from './MessageAssessmentRequests'
import { MessageFeedback } from './MessageFeedback'
import type { MessageTool } from '../../../components/reading/MessageTools'
const request = vi.hoisted(() => vi.fn())
vi.mock('../../../platform/ipc/message-help', () => ({ requestMessageHelp: request }))
beforeEach(() => {
  request.mockReset().mockResolvedValue(undefined)
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open') }
})
it('requests missing help only when the message card opens', async () => {
  render(<MessageFeedback id={1} text="Hola" feedback={undefined} error={undefined} reviewing={false} onEdit={undefined} onAsk={vi.fn()} reward={null}
    bubble={(tool: MessageTool) => <button onClick={tool.onSelect}>Open analysis</button>}
    requests={<MessageAssessmentRequests messageId="source" assessed={false} coached={false} />} />)
  expect(request).not.toHaveBeenCalled()
  fireEvent.click(screen.getByText('Open analysis'))
  await waitFor(() => expect(request).toHaveBeenCalledTimes(2))
  expect(request).toHaveBeenCalledWith('source', 'assessment')
  expect(request).toHaveBeenCalledWith('source', 'coaching')
  expect(screen.queryByText('No feedback was saved for this message.')).toBeNull()
})
it.each(['ready', 'running', 'waiting_dependencies', 'succeeded', 'failed', 'unknown'])('does not request existing %s work on open', state => {
  render(<MessageAssessmentRequests messageId="source" assessed={state === 'succeeded'} coached={state === 'succeeded'} assessmentState={state} feedbackState={state} />)
  expect(request).not.toHaveBeenCalled()
  if (state === 'failed' || state === 'unknown') {
    fireEvent.click(screen.getAllByRole('button', { name: 'Retry' })[0])
    expect(request).toHaveBeenCalledExactlyOnceWith('source', 'assessment', true)
  }
})
