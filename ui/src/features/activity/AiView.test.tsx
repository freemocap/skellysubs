// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { useNavigationStore } from '../../state/navigation/navigation'
import { AiView } from './AiView'

const api = vi.hoisted(() => ({
  readWorkspace: vi.fn(), watchConversation: vi.fn(), listTurnHistory: vi.fn(), getAiViewSelection: vi.fn(), setAiViewSelection: vi.fn(),
  readGraphHistory: vi.fn(), listNativeWorkspaceRuns: vi.fn(), readNativeWorkspaceGraph: vi.fn(), readNativeRunHistory: vi.fn(),
  original: vi.fn(), currentWindowLabel: vi.fn(), emitGraphHover: vi.fn(), onGraphHoverReply: vi.fn(), workspace: vi.fn(), workspaceRun: vi.fn(),
}))
vi.mock('../../platform/ipc/workspace', () => ({ ...api, selectedConversation: (workspace: { selected: string | null }) => workspace.selected ? { id: workspace.selected } : null, nativeError: String }))
vi.mock('../../platform/ipc/window', () => api)
vi.mock('../../components/feedback/OriginalContent', () => ({ OriginalContent: (props: unknown) => { api.original(props); return <div data-testid="original-content" /> } }))
vi.mock('./workspace/RunWorkspace', () => ({ RunWorkspace: (props: { graph: { run: string }; popover: string | null; onPopover: (node: string) => void; orientation: string; onHoverResults: (results: string[] | null) => void }) => {
  api.workspace(props)
  return <div data-testid="run" data-orientation={props.orientation}>{props.graph.run}<span>{props.popover}</span>
    <button onClick={() => props.onPopover('reply')}>Inspect reply</button>
    <button onClick={() => props.onHoverResults(['text'])}>Hover reply</button></div>
} }))
vi.mock('./WorkspaceRunView', () => ({ WorkspaceRunView: (props: { entry: { run: string } }) => { api.workspaceRun(props); return <div data-testid="workspace-run">{props.entry.run}</div> } }))

function turn(id: string) {
  return { id, channel: 'coach', state: 'pending', paused: false, route: 'hosted', hold: null, nativeGraph: { engine: 'engine', run: id, artifact_id: 'a', revision: '1', artifact: { definition: { inputs: {}, results: {} } }, nodes: { reply: 'Running', helper: 'Unrequested' }, attempts: {} } }
}
function snapshot(conversationId: string, revision: number, turns = [turn('run')]) {
  return { conversationId, revision, turns, messages: [], connection: { paused: false }, transcriptionAttempts: [] }
}
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
const openPicker = () => {
  const details = document.querySelector('.ai-run-picker') as HTMLDetailsElement
  details.open = true
  fireEvent(details, new Event('toggle'))
}
beforeEach(() => {
  vi.resetAllMocks()
  useNavigationStore.setState({ aiInspection: null })
  api.readWorkspace.mockResolvedValue({ selected: 'chat' })
  api.watchConversation.mockResolvedValueOnce(snapshot('chat', 1)).mockImplementation(() => new Promise(() => {}))
  api.getAiViewSelection.mockResolvedValue(null)
  api.setAiViewSelection.mockResolvedValue(undefined)
  api.onGraphHoverReply.mockResolvedValue(() => {})
  api.currentWindowLabel.mockResolvedValue('main')
  api.emitGraphHover.mockResolvedValue(undefined)
})

it('shows the selected executable run and persists the inspected operation', async () => {
  render(<AiView mode="docked" actions={null} />)
  expect(await screen.findByTestId('run')).toHaveTextContent('run')
  expect(screen.getByText('Automatic AI work').closest('details')).not.toHaveAttribute('open')
  fireEvent.click(screen.getByRole('button', { name: 'Inspect reply' }))
  await waitFor(() => expect(api.setAiViewSelection).toHaveBeenLastCalledWith({ conversationId: 'chat', turnId: null, operationKind: 'reply' }))
  expect(api.watchConversation).toHaveBeenLastCalledWith('chat', 1)
})

it('reads history only through the selected run identity', async () => {
  api.readGraphHistory.mockResolvedValue({ engine: 'engine', run: 'run', current_revision: '1', frames: [], before: null })
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  const props = api.workspace.mock.lastCall![0] as { loadHistory: (before: string | null) => Promise<unknown> }
  expect(api.readGraphHistory).not.toHaveBeenCalled()
  await props.loadHistory('4')
  expect(api.readGraphHistory).toHaveBeenCalledWith('chat', 'run', '4')
})

it.each(['screen', 'tray'] as const)('stacks the graph down the screen in %s mode', async mode => {
  render(<AiView mode={mode} actions={null} />)
  expect(await screen.findByTestId('run')).toHaveAttribute('data-orientation', 'down')
})

it('pins the explicitly selected run when another run arrives, and follows live again on request', async () => {
  const next = deferred<ReturnType<typeof snapshot>>()
  api.watchConversation.mockReset().mockResolvedValueOnce(snapshot('chat', 1, [turn('first')])).mockReturnValueOnce(next.promise).mockImplementation(() => new Promise(() => {}))
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  openPicker()
  fireEvent.click(screen.getByRole('button', { name: /coach · first/ }))
  await act(async () => next.resolve(snapshot('chat', 2, [turn('second'), turn('first')])))
  expect(screen.getByTestId('run')).toHaveTextContent('first')
  openPicker()
  fireEvent.click(screen.getByRole('button', { name: 'Follow live' }))
  expect(screen.getByTestId('run')).toHaveTextContent('second')
})

it('restores a run outside the live page before displaying it', async () => {
  const recent = Array.from({ length: 50 }, (_, i) => turn(`run-${50 - i}`))
  api.watchConversation.mockReset().mockResolvedValueOnce(snapshot('chat', 1, recent)).mockImplementation(() => new Promise(() => {}))
  api.getAiViewSelection.mockResolvedValue({ conversationId: 'chat', turnId: 'run-0', operationKind: 'reply' })
  api.listTurnHistory.mockResolvedValue({ turns: [turn('run-0')], hasOlder: false })
  render(<AiView mode="window" actions={null} />)
  await waitFor(() => expect(screen.getByTestId('run')).toHaveTextContent('run-0'))
  expect(api.listTurnHistory).toHaveBeenCalledWith('chat', 'run-1', 40)
  expect(screen.getByTestId('run')).toHaveTextContent('reply')
})

it('reports restoration failure without replacing the pinned selection', async () => {
  api.watchConversation.mockReset().mockResolvedValueOnce(snapshot('chat', 1, Array.from({ length: 50 }, (_, i) => turn(String(i))))).mockImplementation(() => new Promise(() => {}))
  api.getAiViewSelection.mockResolvedValue({ conversationId: 'chat', turnId: 'missing', operationKind: null })
  api.listTurnHistory.mockRejectedValue(new Error('History read failed'))
  render(<AiView mode="window" actions={null} />)
  expect(await screen.findByRole('alert')).toHaveTextContent('History read failed')
  expect(screen.queryByTestId('run')).toBeNull()
  expect(api.listTurnHistory).toHaveBeenCalledTimes(1)
})

it('rejects a response from an abandoned conversation and follows the selected scope', async () => {
  const first = deferred<ReturnType<typeof snapshot>>()
  api.watchConversation.mockReset().mockReturnValueOnce(first.promise).mockResolvedValueOnce(snapshot('other', 2, [turn('other-run')])).mockImplementation(() => new Promise(() => {}))
  api.readWorkspace.mockResolvedValueOnce({ selected: 'chat' }).mockResolvedValue({ selected: 'other' })
  render(<AiView mode="window" actions={null} />)
  await waitFor(() => expect(api.watchConversation).toHaveBeenCalledWith('chat', -1))
  await act(async () => first.resolve(snapshot('chat', 1, [turn('abandoned')])))
  await waitFor(() => expect(screen.getByTestId('run')).toHaveTextContent('other-run'))
  expect(screen.queryByText('abandoned')).toBeNull()
})

it('accepts a direct inspection link while mounted', async () => {
  api.watchConversation.mockReset().mockResolvedValueOnce(snapshot('chat', 1, [turn('new'), turn('selected')])).mockImplementation(() => new Promise(() => {}))
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  act(() => useNavigationStore.setState({ aiInspection: { conversationId: 'chat', turnId: 'selected', operationKind: 'reply' } }))
  await waitFor(() => expect(screen.getByTestId('run')).toHaveTextContent('selected'))
  expect(screen.getByTestId('run')).toHaveTextContent('reply')
})

it('lists runs elsewhere in the app only on request and shows the chosen one', async () => {
  api.listNativeWorkspaceRuns.mockResolvedValue({ runs: [{ engine: 'engine', run: 'reading-1', artifact: 'artifact', kind: 'translation' }], before: null })
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  openPicker()
  expect(api.listNativeWorkspaceRuns).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Show runs from the rest of the app' }))
  fireEvent.click(await screen.findByRole('button', { name: 'translation · reading-1' }))
  expect(screen.getByTestId('workspace-run')).toHaveTextContent('reading-1')
  expect(screen.queryByTestId('run')).toBeNull()
})

it('publishes downstream-surface hints for the selected turn in the main window', async () => {
  const { useGraphHoverStore } = await import('../../state/navigation/graph-hover')
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback => { callback(0); return 1 })
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  fireEvent.click(screen.getByRole('button', { name: 'Hover reply' }))
  expect(useGraphHoverStore.getState().target).toEqual({ key: { scope: 'conversation', conversationId: 'chat', turnId: 'run' }, results: ['text'] })
  expect(api.emitGraphHover).not.toHaveBeenCalled()
})

it('does not resume reads after unmount', async () => {
  const next = deferred<ReturnType<typeof snapshot>>()
  api.watchConversation.mockReset().mockReturnValue(next.promise)
  const view = render(<AiView mode="docked" actions={null} />)
  await waitFor(() => expect(api.watchConversation).toHaveBeenCalledTimes(1))
  view.unmount()
  await act(async () => next.resolve(snapshot('chat', 1)))
  expect(api.readWorkspace).toHaveBeenCalledTimes(1)
})

it('restores workspace selection and its recorded revision from the native handoff', async () => {
  const elsewhere = { kind: 'workspace', entry: { engine: 'engine', run: 'reading-1', artifact: 'artifact', kind: 'translation' } }
  const revision = { kind: 'revision', revision: '4' }
  api.getAiViewSelection.mockResolvedValue({ conversationId: 'chat', turnId: null, operationKind: null, elsewhere, revision })
  render(<AiView mode="window" actions={null} />)
  expect(await screen.findByTestId('workspace-run')).toHaveTextContent('reading-1')
  expect(api.workspaceRun.mock.lastCall![0]).toMatchObject({ entry: elsewhere.entry, initialRevision: revision })
  await waitFor(() => expect(api.setAiViewSelection).toHaveBeenLastCalledWith({ conversationId: 'chat', turnId: null, operationKind: null, elsewhere, revision }))
})

it('pins a replayed revision to its turn and clears replay when following live', async () => {
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  act(() => api.workspace.mock.lastCall![0].onRevision({ kind: 'revision', revision: '1' }))
  await waitFor(() => expect(api.setAiViewSelection).toHaveBeenLastCalledWith({ conversationId: 'chat', turnId: 'run', operationKind: null, revision: { kind: 'revision', revision: '1' } }))
  openPicker()
  fireEvent.click(screen.getByRole('button', { name: 'Follow live' }))
  await waitFor(() => expect(api.setAiViewSelection).toHaveBeenLastCalledWith({ conversationId: 'chat', turnId: null, operationKind: null }))
})

it('opens the exact recording source when selecting a transcription', async () => {
  api.watchConversation.mockReset().mockResolvedValueOnce({ ...snapshot('chat', 1), transcriptionAttempts: [{ id: 'recording-source', model: 'recognizer', state: 'succeeded', diagnostics: null }] }).mockImplementation(() => new Promise(() => {}))
  render(<AiView mode="docked" actions={null} />)
  await screen.findByTestId('run')
  openPicker()
  fireEvent.click(screen.getByRole('button', { name: 'recognizer · succeeded' }))
  expect(await screen.findByTestId('original-content')).toBeInTheDocument()
  expect(api.original).toHaveBeenLastCalledWith(expect.objectContaining({ source: { recording: 'recording-source' } }))
})
