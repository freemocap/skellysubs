import { errorDetails } from './error-details'
import { invoke } from '@tauri-apps/api/core'
import { diagnosticCommands } from '../../generated/contracts'
import { isTauri } from '../ipc/tauri'

type Level = 'debug' | 'info' | 'warn' | 'error'
export interface LogEntry { ts: number; level: Level; message: string }
const LOG_BUFFER: LogEntry[] = []
function record(level: Level, message: string) {
  LOG_BUFFER.push({ ts: Date.now(), level, message })
  if (LOG_BUFFER.length > 400) LOG_BUFFER.shift()
}
export function getLogs(): LogEntry[] { return LOG_BUFFER }
export function clearLogs(): void { LOG_BUFFER.length = 0 }

type DiagnosticContext = 'application' | 'conversation' | 'speech' | 'microphone' | 'settings' | 'audio' | 'navigation' | 'other'
type DiagnosticCode = 'ui_fault' | 'native_command_failed' | 'unhandled_error' | 'unhandled_rejection' | 'diagnostic_bridge_failed' | 'ui_event'
type Cause = 'custom_transcription_unconfigured' | 'playback_denied' | 'playback_failed' | 'network_failure' | 'resize_observer_loop' | 'resource_load_failed' | 'type_error' | 'reference_error' | 'syntax_error' | 'abort_error' | 'unknown'
type EventName = 'speech_requested' | 'speech_suppressed' | 'speech_playing' | 'speech_finished' | 'console' | 'ipc_started' | 'ipc_succeeded' | 'ipc_failed' | 'settings_opened' | 'settings_loaded' | 'settings_saving' | 'microphone_autosend' | 'microphone_empty' | 'application_mounted' | 'language_registry_loaded' | 'other'
const nativeCodes = new Set(['validation', 'conflict', 'not_found', 'session_expired', 'storage', 'provider', 'admission_held', 'unknown_outcome', 'credential', 'internal'])
const commands = new Set<string>(diagnosticCommands)
let bridgeFailureReported = false
let deliveryFailures = 0
let pendingDeliveries = 0
let consoleBypass = false
let disposeCapture: (() => void) | null = null

function diagnosticContext(context: string): DiagnosticContext {
  if (/microphone|^\[mic\]/i.test(context)) return 'microphone'
  if (/speech/i.test(context)) return 'speech'
  if (/audio|sound/i.test(context)) return 'audio'
  if (/settings|language|^\[lang\]/i.test(context)) return 'settings'
  if (/conversation|^\[guided\]/i.test(context)) return 'conversation'
  if (/startup|application/i.test(context)) return 'application'
  if (/navigation/i.test(context)) return 'navigation'
  return 'other'
}
function field(value: unknown, key: string): unknown {
  if (typeof value !== 'object' || value === null) return undefined
  try { return Reflect.get(value, key) } catch { return undefined }
}
function causeOf(error: unknown): Cause {
  const text = typeof error === 'string' ? error : field(error, 'message')
  if (text === 'This custom endpoint is configured for chat only. Enable transcription and set its model in AI access settings.') return 'custom_transcription_unconfigured'
  if (text === 'Audio playback failed.') return 'playback_failed'
  if (text === 'Failed to fetch' || text === 'Network request failed') return 'network_failure'
  // The browser's fixed notice that observer callbacks were deferred to the next frame.
  if (text === 'ResizeObserver loop completed with undelivered notifications.' || text === 'ResizeObserver loop limit exceeded') return 'resize_observer_loop'
  const name = field(error, 'name')
  if (name === 'NotAllowedError') return 'playback_denied'
  if (name === 'TypeError') return 'type_error'
  if (name === 'ReferenceError') return 'reference_error'
  if (name === 'SyntaxError') return 'syntax_error'
  if (name === 'AbortError') return 'abort_error'
  return 'unknown'
}

/** Preserve scrubbed error details before console, memory and durable IPC delivery. */
export async function logDiagnostic(context: string, error: unknown, faultId?: number, code: DiagnosticCode = 'ui_fault', level: Level = 'error', metadata: { command?: string; eventName?: EventName; redactedArgs?: number; cause?: Cause } = {}): Promise<boolean> {
  const candidate = field(error, 'code')
  const event = { context: diagnosticContext(context), code, level,
    nativeCode: typeof candidate === 'string' && nativeCodes.has(candidate) ? candidate : null,
    faultId: faultId ?? null, command: metadata.command && commands.has(metadata.command) ? metadata.command : null,
    diagnostics: errorDetails(error),
    cause: metadata.cause ?? causeOf(error), eventName: metadata.eventName ?? 'other',
    redactedArgs: metadata.redactedArgs ?? (error == null ? 0 : 1) }
  const summary = JSON.stringify(event)
  record(level, summary)
  if (!isTauri) return false
  pendingDeliveries++
  try {
    await invoke('record_frontend_diagnostic', { event })
    bridgeFailureReported = false
    return true
  } catch (error) {
    deliveryFailures++
    const message = `[diagnostics] Durable delivery failed (${deliveryFailures} failures); event is not confirmed persisted. ${JSON.stringify(errorDetails(error))}`
    record('error', message)
    consoleBypass = true
    try { console.error(message) } finally { consoleBypass = false }
    if (!bridgeFailureReported) {
      bridgeFailureReported = true
      window.dispatchEvent(new CustomEvent('diagnostic-bridge-failed', { detail: errorDetails(error) }))
    }
    return false
  } finally { pendingDeliveries-- }
}
export function diagnosticDeliveryState() { return { pending: pendingDeliveries, failed: deliveryFailures } }

function authoredEvent(args: unknown[]): { context: string; eventName: EventName; command?: string } {
  const text = typeof args[0] === 'string' ? args[0] : ''
  const ipc = /^\[ipc\] ([a-z_]+) ([→✓✗])(?: \d+ms)?$/.exec(text)
  if (ipc && commands.has(ipc[1])) return { context: ipc[1], command: ipc[1], eventName: ipc[2] === '→' ? 'ipc_started' : ipc[2] === '✓' ? 'ipc_succeeded' : 'ipc_failed' }
  const known: Record<string, EventName> = {
    '[guided] page mounted, isTauri =': 'application_mounted',
    '[settings] modal opened': 'settings_opened', '[settings] loaded': 'settings_loaded', '[settings] autosaving': 'settings_saving',
    '[mic] auto-send enabled — sending transcription': 'microphone_autosend', '[mic] transcription was empty (silence?)': 'microphone_empty',
  }
  return { context: text, eventName: known[text] ?? (text.startsWith('[lang] registry loaded:') ? 'language_registry_loaded' : 'other') }
}
function write(level: Level, args: unknown[], directConsole = false) {
  const known = authoredEvent(args)
  const error = args.find(value => typeof field(value, 'message') === 'string') ?? (level === 'error' || level === 'warn' ? args.find(value => typeof value === 'string') : undefined)
  const details = errorDetails(error, args.filter(value => value !== error))
  const retained = { ...details, diagnostics: details }
  void logDiagnostic(known.context, retained, undefined, 'ui_event', level, { ...known, eventName: directConsole && known.eventName === 'other' ? 'console' : known.eventName, redactedArgs: args.length })
  {
    consoleBypass = true
    try { console[level === 'info' ? 'log' : level](`[${known.eventName}] ${JSON.stringify(details)}`) }
    finally { consoleBypass = false }
  }
}
export function logDebug(...args: unknown[]) { write('debug', args) }
export function logInfo(...args: unknown[]) { write('info', args) }
export function logWarn(...args: unknown[]) { write('warn', args) }
export function logError(...args: unknown[]) { write('error', args) }

/** Idempotent, disposable capture. Internal diagnostics bypass these wrappers. */
export function installDiagnosticCapture(): () => void {
  if (disposeCapture) return disposeCapture
  const restore: (() => void)[] = []
  for (const method of ['log', 'info', 'debug', 'warn', 'error', 'trace', 'table', 'dir', 'dirxml', 'group', 'groupCollapsed', 'groupEnd', 'count', 'countReset', 'time', 'timeLog', 'timeEnd', 'clear', 'assert'] as const) {
    const original = console[method]
    const wrapped = (...args: unknown[]) => {
      if (consoleBypass) { Reflect.apply(original, console, args); return }
      if (method === 'assert' && args[0]) return
      const level: Level = method === 'error' || method === 'assert' ? 'error' : method === 'warn' ? 'warn' : method === 'debug' ? 'debug' : 'info'
      write(level, args, true)
    }
    if (!Reflect.set(console, method, wrapped)) throw new Error('Console diagnostic capture could not be installed.')
    restore.push(() => { if (console[method] === wrapped && !Reflect.set(console, method, original)) throw new Error('Console diagnostic capture could not be restored.') })
  }
  // Capture phase also receives element load failures, which do not bubble.
  // Everything except the browser's resize-observer notice and aborted requests
  // is also put on screen; `unhandled-ui-error` carries it to the fault bar.
  const surface = (detail: unknown) => window.dispatchEvent(new CustomEvent('unhandled-ui-error', { detail }))
  const onError = (event: ErrorEvent) => {
    if (event.target instanceof Element) {
      void logDiagnostic('application', null, undefined, 'unhandled_error', 'error', { cause: 'resource_load_failed' })
      surface(`A ${event.target.tagName.toLowerCase()} element failed to load its resource.`)
      return
    }
    const error = event.error ?? { name: 'Error', message: event.message, stack: event.filename ? `at browser (${event.filename}:${event.lineno}:${event.colno})` : undefined }
    void logDiagnostic('application', error, undefined, 'unhandled_error')
    if (causeOf(error) !== 'resize_observer_loop') surface(error)
  }
  const onRejection = (event: PromiseRejectionEvent) => {
    void logDiagnostic('application', event.reason, undefined, 'unhandled_rejection')
    if (causeOf(event.reason) !== 'abort_error') surface(event.reason)
  }
  window.addEventListener('error', onError, true)
  window.addEventListener('unhandledrejection', onRejection)
  disposeCapture = () => {
    restore.forEach(fn => fn())
    window.removeEventListener('error', onError, true); window.removeEventListener('unhandledrejection', onRejection)
    disposeCapture = null
  }
  return disposeCapture
}
