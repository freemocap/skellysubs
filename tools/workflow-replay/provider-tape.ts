import type { TimedEvent } from './session.ts'

export type ProviderEvent = Extract<TimedEvent, { kind: 'provider' }>

/** Match stable workflow request keys, not provider arrival order. The application
 * adapter assigns keys and hashes the credential-free semantic request. There is
 * deliberately no live transport callback or fallback. */
export class ProviderTape {
  private readonly pending = new Map<string, ProviderEvent[]>()
  private readonly active = new Set<string>()
  private failed = false

  constructor(events: readonly TimedEvent[]) {
    for (const event of events) {
      if (event.kind !== 'provider') continue
      if (!event.request || !/^[a-f0-9]{64}$/.test(event.fingerprint)) throw new Error('Invalid provider request identity.')
      const entries = this.pending.get(event.request) ?? []
      const previous = entries.at(-1)
      if ((!previous && event.phase !== 'request') || (previous && (!['request', 'chunk'].includes(previous.phase) || event.phase === 'request' || previous.fingerprint !== event.fingerprint || previous.elapsedMicros > event.elapsedMicros))) throw new Error('Invalid provider response sequence.')
      entries.push(structuredClone(event))
      this.pending.set(event.request, entries)
    }
    for (const entries of this.pending.values()) {
      if (['request', 'chunk'].includes(entries.at(-1)!.phase)) throw new Error('Provider response has no terminal outcome.')
    }
  }

  async replay(request: string, fingerprint: string, deliver: (event: ProviderEvent) => Promise<void>, wait: (micros: number) => Promise<void> = async () => {}): Promise<void> {
    if (this.failed) throw new Error('Replay session has failed.')
    const entries = this.pending.get(request)
    if (!entries || entries[0].fingerprint !== fingerprint) {
      this.failed = true
      throw new Error('No matching recorded provider request.')
    }
    // Consume before awaiting: concurrent requests cannot take the same tape.
    this.pending.delete(request)
    this.active.add(request)
    try {
      let previous = entries[0].elapsedMicros
      for (const event of entries.slice(1)) {
        await wait(event.elapsedMicros - previous)
        await deliver(structuredClone(event))
        previous = event.elapsedMicros
      }
    } catch (error) { this.failed = true; throw error }
    finally { this.active.delete(request) }
  }

  assertConsumed(): void {
    if (this.failed || this.pending.size || this.active.size) throw new Error('Replay has failed, unconsumed or active provider responses.')
  }
}
