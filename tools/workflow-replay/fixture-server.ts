import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'

export const fixtureReply = 'Workflow fixture reply: café 𠮷.'

/** Synthetic prose only. No outbound network, credentials, content logs or
 * general schema fabrication. Unsupported requests fail visibly. */
export async function fixtureServer() {
  const requests = { protocol: 0, operations: 0, items: 0, rejected: 0 }
  const server = createServer(async (request, response) => {
    try {
      if (request.headers.authorization) throw new Error('Fixture accepts no credentials.')
      if (request.method === 'GET' && request.url?.split('?')[0] === '/v1/protocol') {
        requests.protocol++
        response.setHeader('content-type', 'application/json')
        response.end(JSON.stringify({ protocol: 'skellyspeak', version: 1, operations_versions: [3], max_items: 8, audio: { version: 3, routing: { version: 1, available_models: ['whisper-large-v3-turbo'], accepts_custom_transcription_models: true } }, providers: ['OPENROUTER', 'GROQ'].map(provider => ({ provider, state: 'accepted', status: 200, durationMs: 0 })) }))
        return
      }
      if (request.method !== 'POST' || request.url !== '/v1/operations') throw new Error('Unsupported fixture request.')
      const chunks: Buffer[] = []
      let size = 0
      for await (const chunk of request) {
        const bytes = Buffer.from(chunk as Uint8Array)
        size += bytes.byteLength
        if (size > 1024 * 1024) throw new Error('Fixture request exceeds limit.')
        chunks.push(bytes)
      }
      const value = JSON.parse(Buffer.concat(chunks).toString('utf8')) as { version?: number; items?: { operation_id?: string; attempt_id?: string; deltas?: boolean; request?: { model?: string; response_format?: unknown } }[] }
      if (value.version !== 3 || !Array.isArray(value.items) || value.items.length < 1 || value.items.length > 8) throw new Error('Invalid fixture operation group.')
      for (const item of value.items) {
        if (!item || typeof item.operation_id !== 'string' || typeof item.attempt_id !== 'string' || typeof item.request?.model !== 'string' || item.request.response_format) throw new Error('Fixture only supports prose operations.')
      }
      requests.operations++
      response.setHeader('content-type', 'application/x-ndjson')
      for (const item of value.items) {
        requests.items++
        const identity = { operation_id: item.operation_id, attempt_id: item.attempt_id }
        if (item.deltas) response.write(`${JSON.stringify({ type: 'frames', ...identity, offset: 0, frames: [{ choices: [{ delta: { content: fixtureReply } }] }] })}\n`)
        response.write(`${JSON.stringify({ type: 'result', ...identity, response: { id: `synthetic-${requests.items}`, model: item.request!.model, choices: [{ finish_reason: 'stop', message: { content: fixtureReply } }], usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } } })}\n`)
      }
      response.end(`${JSON.stringify({ type: 'complete', count: value.items.length })}\n`)
    } catch {
      requests.rejected++
      response.statusCode = 400
      response.end(JSON.stringify({ error: 'Unsupported or malformed synthetic fixture request.' }))
    }
  })
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  return {
    url: `http://127.0.0.1:${(server.address() as AddressInfo).port}/v1`,
    requests,
    close: () => new Promise<void>((resolve, reject) => { server.close(error => error ? reject(error) : resolve()); server.closeAllConnections() }),
  }
}
