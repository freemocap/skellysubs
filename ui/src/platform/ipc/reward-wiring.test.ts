// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { executeAction, watchConversation, beginPersonaGeneration } from './workspace'
import { readSelection } from './reading'
import { onActionReward, publishEffortAwards, trackRewardActivations, type ActionReward } from './reward-origin'
import type { EffortAward, ReadingInput } from '../../generated/contracts'
const native = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('./native', () => native)
const observed = vi.fn<(reward: ActionReward) => void>()
let stop: () => void
let unsubscribe: () => void
beforeEach(() => { vi.clearAllMocks(); stop = trackRewardActivations(); unsubscribe = onActionReward(observed) })
afterEach(() => { stop(); unsubscribe() })
const earned = (sourceId: string): EffortAward => ({ id: sourceId, sourceId, dimension: 'explorations', createdAt: new Date().toISOString(), claimed: false, language: 'spanish', variety: 'standard', conversationId: null, policy: 'test' })
function click(x: number) { document.body.dispatchEvent(new MouseEvent('click', { bubbles: true, detail: 1, clientX: x, clientY: 100 })) }
it('links a coach command receipt through its native award source to the original click', async () => {
  native.invoke.mockResolvedValueOnce({ entityId: 'coach-turn' }).mockResolvedValueOnce({ turns: [{ id: 'coach-turn', awardSources: ['graph-effect:coach-result'] }] })
  click(20)
  await executeAction({ sessionId: 's' }, { kind: 'askCoach', conversationId: 'chat', text: 'Explain', expectedRevision: 1 })
  click(80)
  await watchConversation('chat')
  publishEffortAwards([earned('graph-effect:coach-result')])
  expect(observed.mock.calls[0][0].origin.x).toBe(20)
})
it('binds a persona reservation before the later award arrives', async () => {
  native.invoke.mockResolvedValueOnce('persona-reservation')
  click(35); await beginPersonaGeneration('spanish', 'A friend')
  click(90); publishEffortAwards([earned('persona:persona-reservation')])
  expect(observed.mock.calls[0][0].origin.x).toBe(35)
})
it('publishes fresh reading rewards locally and leaves cached reads quiet', async () => {
  const input: ReadingInput = { text: 'Hola', language: 'spanish', variety: null, explanation: 'english', explanationVariety: null, aid: 'translation' }
  let complete!: (value: unknown) => void
  native.invoke.mockImplementation((command: string) => command === 'begin_reading' ? Promise.resolve('reading-request') : new Promise(resolve => { complete = resolve }))
  click(44)
  const result = readSelection(input, new AbortController().signal)
  await Promise.resolve()
  click(88)
  complete({ receipt: { effortAward: earned('reading:fresh') } })
  await result
  expect(observed.mock.calls[0][0].origin.x).toBe(44)
  native.invoke.mockImplementation((command: string) => Promise.resolve(command === 'begin_reading' ? 'cached-request' : { receipt: { response: { cacheHit: true } } }))
  await readSelection(input, new AbortController().signal)
  expect(observed).toHaveBeenCalledTimes(1)
})
