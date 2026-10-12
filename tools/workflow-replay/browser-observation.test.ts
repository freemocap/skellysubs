import assert from 'node:assert/strict'
import test from 'node:test'
import vm from 'node:vm'
import { installBrowserObservation } from './browser-observation.ts'

function observation() {
  const listeners = new Map<string, (event: unknown) => void>()
  const host: Record<string, any> = { addEventListener: (name: string, listener: (event: unknown) => void) => listeners.set(name, listener) }
  class Media { play() {} }
  class Source { start() {} }
  class Worklet {}
  host.AudioWorkletNode = Worklet
  const context = vm.createContext({ window: host, HTMLMediaElement: Media, AudioBufferSourceNode: Source,
    performance, setTimeout, document: { getElementById: () => ({ innerText: 'fixture' }), querySelectorAll: () => [] } })
  vm.runInContext(`(${installBrowserObservation.toString()})()`, context)
  host.__workflowObserve(true)
  return { checkpoint: () => host.__workflowCheckpoint(), error: (message: string) => listeners.get('error')!({ message }) }
}

test('resize delivery notices do not reject a checkpoint; genuine errors still do', async () => {
  const notice = observation()
  notice.error('ResizeObserver loop completed with undelivered notifications.')
  await assert.doesNotReject(notice.checkpoint())
  const failure = observation()
  failure.error('Uncaught TypeError')
  await assert.rejects(failure.checkpoint(), /Unhandled browser error/)
})
