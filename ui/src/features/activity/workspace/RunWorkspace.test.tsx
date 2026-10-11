// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import type { RunHistory } from '../../../generated/graph-contracts'
import { pipelineArtifact, pipelineSnapshot } from '../../../../tests/fixtures/graph-artifacts'
import { RunWorkspace, type RunWorkspaceProps } from './RunWorkspace'

beforeEach(() => localStorage.clear())

it('plays recorded revisions, stops at the end and keeps execution controls out of replay', async () => {
  const frames = [pipelineSnapshot({ revision: '1', nodes: { write: 'Running' } }), pipelineSnapshot({ revision: '2', nodes: { write: 'Available' } })]
  const { view } = setup({ loadHistory: vi.fn().mockResolvedValue({ engine: 'engine-1', run: 'run-1', current_revision: '2', frames, before: null }) })
  fireEvent.click(screen.getByRole('button', { name: 'History' }))
  await waitFor(() => expect(screen.getByRole('button', { name: 'Play' })).toBeEnabled())
  vi.useFakeTimers()
  try {
    fireEvent.click(screen.getByRole('button', { name: 'Play' }))
    expect(screen.getByText('Replaying revision 1.')).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Pause' })).toBeNull()
    expect(view.container.querySelector('[data-state="flowing"]')).not.toBeNull()
    act(() => vi.advanceTimersByTime(900))
    expect(screen.getByText('Replaying revision 2.')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Play' })).toBeEnabled()
    expect(view.container.querySelector('[data-state="flowing"]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Live' }))
    expect(screen.getByRole('button', { name: 'Pause' })).toBeVisible()
  } finally { vi.useRealTimers() }
})

function setup(overrides: Partial<RunWorkspaceProps> = {}) {
  const props: RunWorkspaceProps = {
    graph: pipelineSnapshot({ nodes: { read: 'Available', write: 'Running' } }),
    preview: null, stations: [], turn: null, messages: [], loadHistory: vi.fn<(before: string | null) => Promise<RunHistory>>().mockResolvedValue({ engine: 'engine-1', run: 'run-1', current_revision: '1', frames: [], before: null }),
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

it('draws every step open, local ones included, and lists inputs read in place', () => {
  setup()
  expect(screen.getByRole('button', { name: 'Everything recorded about Read' })).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Everything recorded about Pack inner deep' })).toBeInTheDocument()
  expect(screen.getByTitle('Graph input seed · t:text')).toHaveTextContent('seed')
  expect(screen.getAllByTitle('t:text').find(row => row.classList.contains('is-mark'))).toHaveTextContent('modeconstant')
})

it('draws stations outside the graph with their facts', () => {
  setup({ stations: [{ id: 'station:message', side: 'before', name: 'Your message', tone: 'none', ports: ['seed'], rows: [{ label: 'text', value: 'Hola' }] }] })
  expect(screen.getByRole('article', { name: 'Your message' })).toHaveTextContent('textHola')
})

it('reads run history only when the history is opened', async () => {
  const { props } = setup()
  expect(props.loadHistory).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'History' }))
  await waitFor(() => expect(props.loadHistory).toHaveBeenCalledWith(null))
})

it('opens details directly from cards and compact nodes without per-node collapse controls', () => {
  const { props } = setup()
  fireEvent.click(screen.getByRole('button', { name: '+1' }))
  expect(screen.getByRole('list', { name: 'Inputs fixed when the run started' })).toHaveTextContent('extra')
  fireEvent.click(screen.getByRole('button', { name: 'Everything recorded about Write' }))
  expect(props.onPopover).toHaveBeenCalledWith('write')
  fireEvent.click(screen.getByRole('button', { name: 'Compact' }))
  fireEvent.click(screen.getByRole('button', { name: /^Write\. Op:write → t:text/ }))
  expect(props.onPopover).toHaveBeenLastCalledWith('write')
  expect(screen.queryByRole('button', { name: 'Shrink Write' })).toBeNull()
})

it('focuses a step from the now line by growing it into a card', () => {
  localStorage.setItem('skellyspeak_ai_workspace', JSON.stringify({ levels: { write: 'bubble' }, offsets: {}, sizes: {} }))
  setup()
  expect(screen.queryByRole('button', { name: 'Everything recorded about Write' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Write' }))
  expect(screen.getByRole('button', { name: 'Everything recorded about Write' })).toBeInTheDocument()
})

it('reports the results downstream of a hovered step while live, and nothing in map view', () => {
  const { props } = setup()
  const write = () => screen.getByRole('button', { name: 'Everything recorded about Write' }).closest('article')!
  fireEvent.pointerEnter(write())
  expect(props.onHoverResults).toHaveBeenLastCalledWith(['packed', 'written'])
  fireEvent.pointerLeave(write())
  expect(props.onHoverResults).toHaveBeenLastCalledWith(null)
  fireEvent.click(screen.getByRole('button', { name: 'Map only' }))
  fireEvent.pointerEnter(write())
  expect(props.onHoverResults).toHaveBeenLastCalledWith(null)
})

it('shows a definition without a run as a map, with no history', () => {
  setup({ graph: { protocol: 1, artifact_id: 'artifact-pipeline', artifact: pipelineArtifact() } as RunWorkspaceProps['graph'], loadHistory: null })
  expect(screen.getByText('Map of the steps, with no run overlaid.')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'History' })).toBeNull()
})

it('remembers dragged offsets and offers to tidy them away', () => {
  localStorage.setItem('skellyspeak_ai_workspace', JSON.stringify({ levels: {}, offsets: { write: [40, 10] }, sizes: {} }))
  setup()
  expect(screen.getByRole('button', { name: 'Everything recorded about Write' }).closest('article')!.dataset.moved).toBe('true')
  fireEvent.click(screen.getByRole('button', { name: 'Tidy up' }))
  expect(screen.queryByRole('button', { name: 'Tidy up' })).toBeNull()
  expect(JSON.parse(localStorage.getItem('skellyspeak_ai_workspace')!).offsets).toEqual({})
})

it('refuses a stored layout with an invalid shape', () => {
  localStorage.setItem('skellyspeak_ai_workspace', JSON.stringify({ levels: { write: 'huge' }, offsets: {} }))
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

it('follows an upstream data connection from the selected operation inspector', () => {
  const { props } = setup({ popover: 'write' })
  fireEvent.click(screen.getByRole('button', { name: /^From Read\./ }))
  expect(props.onPopover).toHaveBeenCalledWith('read')
})
