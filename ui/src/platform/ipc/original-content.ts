import { invoke } from './native'
import type { ContentRecord, ContentPayload } from '../../generated/contracts'
export type OriginalSource = { engine: string; execution: string } | { recording: string }
export function listOriginalContent(source: OriginalSource): Promise<ContentRecord[]> {
  return 'recording' in source ? invoke('recording_original_content', source) : invoke('list_original_content', source)
}
export function readOriginalContent(id: string): Promise<ContentPayload> { return invoke('read_original_content', { id }) }
