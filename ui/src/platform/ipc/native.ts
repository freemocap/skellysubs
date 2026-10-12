import { invoke as callNative } from '@tauri-apps/api/core'
import { logDiagnostic } from '../diagnostics/log'

/** Record each rejection immediately; durable delivery does not delay the caller. */
export async function invoke<T>(command: string, args?: Parameters<typeof callNative>[1], signal?: AbortSignal): Promise<T> {
  // Installed only by an explicitly attached development recorder. The probe
  // receives a command name and completion boundary, never arguments/content.
  const finish = (globalThis as typeof globalThis & { __workflowIpc?: (command: string) => (() => void) | undefined }).__workflowIpc?.(command)
  try { return await callNative<T>(command, args) }
  catch (error) {
    const context = command.startsWith('mic_') ? 'microphone'
      : command.includes('speech') ? 'speech'
      : /settings|access|connection|credential/.test(command) ? 'settings'
      : /conversation|snapshot|execute_command/.test(command) ? 'conversation' : 'application'
    const failure = error as { code?: string; diagnostics?: { error?: { diagnostics?: { reason?: string } } } } | null
    const cancelled = command === 'run_reading' && signal?.aborted
      && failure?.code === 'conflict' && failure.diagnostics?.error?.diagnostics?.reason === 'cancelled'
    void logDiagnostic(context, error, undefined, 'native_command_failed', cancelled ? 'info' : 'error', { command })
    throw error
  }
  finally { finish?.() }
}
