import { loopbackEndpoint } from './desktop-preflight.ts'

type Reply = { id?: number; result?: unknown; error?: { message: string }; method?: string; params?: unknown }
/** One explicit local desktop target. No discovery by title or user content. */
export class DesktopConnection {
  private sequence = 0
  private readonly pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>()
  private readonly socket: WebSocket
  private readonly observe: (method: string, params: unknown) => void
  private constructor(socket: WebSocket, observe: (method: string, params: unknown) => void) {
    this.socket = socket; this.observe = observe
    socket.addEventListener('message', event => {
      let reply: Reply
      try { reply = JSON.parse(String(event.data)) as Reply }
      catch { this.close(); return }
      if (reply.method) this.observe(reply.method, reply.params)
      if (reply.id === undefined) return
      const pending = this.pending.get(reply.id)
      if (!pending) return
      this.pending.delete(reply.id); clearTimeout(pending.timer)
      if (reply.error) pending.reject(new Error(`Desktop command failed: ${reply.error.message}`))
      else pending.resolve(reply.result)
    })
    socket.addEventListener('close', () => this.rejectPending())
    socket.addEventListener('error', () => this.rejectPending())
  }
  static async connect(endpoint: string, observe: (method: string, params: unknown) => void = () => {}): Promise<DesktopConnection> {
    const origin = loopbackEndpoint(endpoint)
    const response = await fetch(new URL('/json/list', origin), { redirect: 'error', signal: AbortSignal.timeout(3000) })
    if (!response.ok) throw new Error('Desktop target discovery failed.')
    const reader = response.body?.getReader()
    if (!reader) throw new Error('Desktop discovery returned no body.')
    const chunks: Uint8Array[] = []; let size = 0
    try {
      while (true) {
        const next = await reader.read(); if (next.done) break
        size += next.value.byteLength
        if (size > 65536) throw new Error('Desktop discovery exceeded its bound.')
        chunks.push(next.value)
      }
    } finally { await reader.cancel(); reader.releaseLock() }
    const text = Buffer.concat(chunks).toString('utf8')
    const targets: unknown = JSON.parse(text)
    if (!Array.isArray(targets)) throw new Error('Invalid desktop target list.')
    const pages = targets.filter(target => target?.type === 'page')
    if (pages.length !== 1) throw new Error('Expected exactly one isolated desktop page.')
    const url = new URL(pages[0].webSocketDebuggerUrl)
    if (url.protocol !== 'ws:' || url.hostname !== origin.hostname || url.port !== origin.port || url.username || url.password) throw new Error('Desktop socket must use the selected loopback origin.')
    const socket = new WebSocket(url)
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => { socket.close(); reject(new Error('Desktop attachment timed out.')) }, 3000)
      socket.addEventListener('open', () => { clearTimeout(timer); resolve() }, { once: true })
      socket.addEventListener('error', () => { clearTimeout(timer); reject(new Error('Desktop attachment failed.')) }, { once: true })
    })
    return new DesktopConnection(socket, observe)
  }
  command<T = unknown>(method: string, params: object = {}): Promise<T> {
    const id = ++this.sequence
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`Desktop command timed out: ${method}`)) }, 15000)
      this.pending.set(id, { resolve: value => resolve(value as T), reject, timer })
      try { this.socket.send(JSON.stringify({ id, method, params })) }
      catch { clearTimeout(timer); this.pending.delete(id); reject(new Error('Desktop connection is closed.')) }
    })
  }
  async evaluate<T>(expression: string): Promise<T> {
    const result = await this.command<{ result: { value: T }; exceptionDetails?: unknown }>('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (result.exceptionDetails) throw new Error('Desktop evaluation failed; capture is not complete.')
    return result.result.value
  }
  private rejectPending(): void {
    for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(new Error('Desktop connection closed.')) }
    this.pending.clear()
  }
  close(): void { this.rejectPending(); this.socket.close() }
}
