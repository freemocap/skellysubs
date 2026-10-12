import { isTauri } from './tauri'
import { invoke } from './native'
import type { AiViewSelection, AiWindowState } from '../../generated/contracts'
import type { RunHistory } from '../../generated/graph-contracts'
import { parseGraphHoverMessage, parseGraphHoverReply, type GraphHoverMessage, type GraphHoverReply } from '../../domain/activity/graph-surface'

/// The label of the webview this bundle runs in, or null outside Tauri.
///
/// The popped-out observability window runs the same bundle as the main window
/// and is told apart by its label, which the Rust dev command sets. Routing on
/// the label rather than a URL query keeps `?` out of the PathBuf that
/// `WebviewUrl::App` wants.
export async function currentWindowLabel(): Promise<string | null> {
  if (!isTauri) return null
  const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
  return getCurrentWebviewWindow().label
}

/// Whether this build can pop the AI View out, and whether that window exists.
/// Outside Tauri there is no second window to open.
export function aiWindowState(): Promise<AiWindowState> {
  if (!isTauri) return Promise.resolve({ supported: false, open: false })
  return invoke<AiWindowState>('ai_window_state')
}

export function openAiWindow(): Promise<void> {
  return invoke<void>('open_ai_window')
}

/// From the AI window: return the view to the main window's panel.
export function dockAiWindow(): Promise<void> {
  return invoke<void>('dock_ai_window')
}

export function setAiViewSelection(selection: AiViewSelection | null): Promise<void> {
  if (!isTauri) return Promise.resolve()
  return invoke<void>('set_ai_view_selection', { selection })
}

export function getAiViewSelection(): Promise<AiViewSelection | null> {
  if (!isTauri) return Promise.resolve(null)
  return invoke<AiViewSelection | null>('get_ai_view_selection')
}

export type AiWindowEvent = 'docked' | 'changed'

/// Hints from native that the AI window moved. They only prompt a re-read of
/// `aiWindowState`; the state itself is never inferred from an event.
export async function onAiWindowEvent(handler: (event: AiWindowEvent) => void): Promise<() => void> {
  if (!isTauri) return () => {}
  const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow')
  const window = getCurrentWebviewWindow()
  const unlisten = await Promise.all([
    window.listen('ai-view-docked', () => handler('docked')),
    window.listen('ai-window-changed', () => handler('changed')),
  ])
  return () => unlisten.forEach(stop => stop())
}

export function readGraphHistory(conversationId: string, runId: string, before: string | null = null): Promise<RunHistory> {
  return invoke<RunHistory>('read_graph_history', { conversationId, runId, before, limit: 32 })
}

/** A reading question is a local draft, never a coach request sent automatically. */
export async function sendReadingQuestion(question: string): Promise<void> {
  const { emitTo } = await import('@tauri-apps/api/event')
  await emitTo('main', 'reading-coach-question', question)
  await dockAiWindow()
}
export async function onReadingQuestion(handler: (question: string) => void): Promise<() => void> {
  if (!isTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<string>('reading-coach-question', event => {
    if (typeof event.payload !== 'string' || event.payload.length > 16000) throw new Error('Invalid reading question.')
    handler(event.payload)
  })
}


/** Deliberate read of one native attempt at an exact historical revision. */
export function readNativeGraphAttempt(engine: string, run: string, revision: string, node: string, attempt: string): Promise<import('../../generated/contracts').NativeAttemptInspection> {
  return invoke('read_native_graph_attempt', { engine, run, revision, node, attempt })
}

export function listNativeWorkspaceRuns(before: string | null = null): Promise<import('../../generated/contracts').NativeRunPage> {
  return invoke('list_native_workspace_runs', { before })
}
export function readNativeRunHistory(engine: string, run: string, before: string | null = null): Promise<RunHistory> {
  return invoke('read_native_run_history', { engine, run, before })
}

export function readNativeWorkspaceGraph(engine: string, run: string, after: string | null): Promise<import('../../generated/graph-contracts').InspectionSnapshot | null> {
  return invoke('read_native_workspace_graph', { engine, run, after })
}

/** Downstream-surface hints travel from the popped-out AI view to the main
 * window, which owns the app surfaces; it answers how many are visible. */
export async function emitGraphHover(message: GraphHoverMessage): Promise<void> {
  if (!isTauri) throw new Error('Graph hover messages need the desktop app.')
  const { emitTo } = await import('@tauri-apps/api/event')
  await emitTo('main', 'ai-graph-hover', message)
}
export async function onGraphHover(handler: (message: GraphHoverMessage) => void): Promise<() => void> {
  if (!isTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<unknown>('ai-graph-hover', event => handler(parseGraphHoverMessage(event.payload)))
}
export async function emitGraphHoverReply(window: string, reply: GraphHoverReply): Promise<void> {
  if (!isTauri) throw new Error('Graph hover replies need the desktop app.')
  const { emitTo } = await import('@tauri-apps/api/event')
  await emitTo(window, 'ai-graph-hover-reply', reply)
}
export async function onGraphHoverReply(handler: (reply: GraphHoverReply) => void): Promise<() => void> {
  if (!isTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<unknown>('ai-graph-hover-reply', event => handler(parseGraphHoverReply(event.payload)))
}
