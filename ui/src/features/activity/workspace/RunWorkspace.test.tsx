// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import type { RunHistory } from '../../../generated/graph-contracts'
import { pipelineArtifact, pipelineSnapshot } from '../../../../tests/fixtures/graph-artifacts'
import { RunWorkspace, type RunWorkspaceProps } from './RunWorkspace'

beforeEach(() => localStorage.clear())

function setup(overrides: Partial<RunWorkspaceProps> = {}) {
  const props: RunWorkspaceProps = {
    graph: pipelineSnapshot({ nodes: { read: 'Available', write: 'Running' } }),
    preview: null, loadHistory: vi.fn<(before: string | null) => Promise<RunHistory>>().mockResolvedValue({ engine: 'engine-1', run: 'run-1', current_revision: '1', frames: [], before: null }),
    controls: <button type="button">Pause</button>, orientation: 'across', compact: false, popover: null,
    onPopover: vi.fn(), onHoverResults: vi.fn(), ...overrides,
  }
  return { props, view: render(<RunWorkspace {...props} />) }
}

it('says what is working now and shows controls only while live', () => {
  setup()
  expect(screen.getByRole('button', { name: 'Write' })).toHaveClass('is-running')
  expect(screen.getByText('1 of 6 steps done')).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Pause' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Map only' }))
  expect(screen.getByText('Map of the steps, with no run overlaid.')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Pause' })).toBeNull()
})

it('hides local steps by default and folds them into the connections', () => {
  setup()
  expect(screen.queryByRole('button', { name: /^Read[,.] / })).toBeNull()
  expect(screen.getByRole('button', { name: /^Write\. Op:write → t:text/ })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Show 4 local steps' }))
  expect(screen.getByRole('button', { name: /^Read\. Op:read → / })).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Hide local steps' })).toBeInTheDocument()
})

it('reads run history only when the history is opened', async () => {
  const { props } = setup()
  expect(props.loadHistory).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'History' }))
  await waitFor(() => expect(props.loadHistory).toHaveBeenCalledWith(null))
})

it('expands a bubble into a card in place, opens the details from the card and shrinks back', () => {
  const { props } = setup()
  fireEvent.click(screen.getByRole('button', { name: /^Write\. / }))
  fireEvent.click(screen.getByRole('button', { name: '+2' }))
  expect(screen.getByRole('list', { name: 'Inputs fixed when the run started' })).toHaveTextContent('extramode')
  fireEvent.click(screen.getByRole('button', { name: 'Everything recorded about Write' }))
  expect(props.onPopover).toHaveBeenCalledWith('write')
  fireEvent.click(screen.getByRole('button', { name: 'Shrink Write' }))
  expect(screen.getByRole('button', { name: /^Write\. / })).toBeInTheDocument()
})

it('focuses a step from the now line by growing it into a card', () => {
  setup()
  fireEvent.click(screen.getByRole('button', { name: 'Write' }))
  expect(screen.getByRole('button', { name: 'Shrink Write' })).toBeInTheDocument()
})

it('reports the results downstream of a hovered step while live, and nothing in map view', () => {
  const { props } = setup()
  fireEvent.pointerEnter(screen.getByRole('button', { name: /^Write\. / }))
  expect(props.onHoverResults).toHaveBeenLastCalledWith(['packed', 'written'])
  fireEvent.pointerLeave(screen.getByRole('button', { name: /^Write\. / }))
  expect(props.onHoverResults).toHaveBeenLastCalledWith(null)
  fireEvent.click(screen.getByRole('button', { name: 'Map only' }))
  fireEvent.pointerEnter(screen.getByRole('button', { name: /^Write\. / }))
  expect(props.onHoverResults).toHaveBeenLastCalledWith(null)
})

it('shows a definition without a run as a map, with no history', () => {
  setup({ graph: { protocol: 1, artifact_id: 'artifact-pipeline', artifact: pipelineArtifact() } as RunWorkspaceProps['graph'], loadHistory: null })
  expect(screen.getByText('Map of the steps, with no run overlaid.')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'History' })).toBeNull()
})

it('remembers dragged offsets and offers to tidy them away', () => {
  localStorage.setItem('skellyspeak_ai_workspace', JSON.stringify({ levels: {}, folded: {}, localSteps: false, offsets: { write: [40, 10] }, sizes: {} }))
  setup()
  const write = screen.getByRole('button', { name: /^Write\. / })
  expect(write.style.left).not.toBe('')
  fireEvent.click(screen.getByRole('button', { name: 'Tidy up' }))
  expect(screen.queryByRole('button', { name: 'Tidy up' })).toBeNull()
  expect(JSON.parse(localStorage.getItem('skellyspeak_ai_workspace')!).offsets).toEqual({})
})

it('refuses a stored layout with an invalid shape', () => {
  localStorage.setItem('skellyspeak_ai_workspace', JSON.stringify({ levels: { write: 'huge' }, folded: {} }))
  expect(() => setup()).toThrow('invalid shape')
})

it('restores a revision from earlier pages and returns to live when the parent clears it', async () => {
  const loadHistory = vi.fn().mockResolvedValueOnce({ engine: 'engine-1', run: 'run-1', current_revision: '3', frames: [pipelineSnapshot({ revision: '3' })], before: '3' })
    .mockResolvedValueOnce({ engine: 'engine-1', run: 'run-1', current_revision: '3', frames: [pipelineSnapshot({ revision: '1' })], before: null })
  const { props, view } = setup({ loadHistory, initialRevision: { kind: 'revision', revision: '1' } })
  await screen.findByText('Replaying revision 1.')
  expect(loadHistory.mock.calls.map(args => args[0])).toEqual([null, '3'])
  expect(screen.queryByRole('button', { name: 'Pause' })).toBeNull()
  view.rerender(<RunWorkspace {...props} initialRevision={undefined} />)
  expect(await screen.findByRole('button', { name: 'Pause' })).toBeInTheDocument()
})
