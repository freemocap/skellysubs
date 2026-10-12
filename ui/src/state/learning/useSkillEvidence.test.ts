// @vitest-environment jsdom
import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { selectSkillSnapshot, useLoadSkillEvidence, useSkillEvidence } from './useSkillEvidence'
import { useSettingsStore } from '../settings/settings'
import { useSkillEvidenceStore } from './skill-evidence'
import { skillDemo } from '../../domain/learning/catalog/skillDemo'
import type { Settings } from '../../types'
import type { SkillSnapshot } from '../../domain/learning/evidence/skills'

const backend = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn() }))
vi.mock('../../platform/ipc/tauri', () => ({ isTauri: true, languageFor: () => null }))
vi.mock('../../platform/ipc/skill-evidence', () => ({ getSkillEvidence: backend.get, saveSkillProfile: backend.save }))

/// Both hooks, composed the way the app composes them: the shell keeps evidence
/// read and the surfaces read it. The read takes its language from the settings
/// store, and test setup resets every store between tests, so it is seeded per
/// test rather than once at module load.
const mount = () => renderHook(() => { useLoadSkillEvidence(); return useSkillEvidence() })
function seed(language: string, revision = 0) {
  useSettingsStore.setState({ settings: { target_language: language } as Settings, revision, evidenceRevision: revision })
}

beforeEach(() => {
  backend.get.mockReset()
  seed(skillDemo.target)
})

it('adopts profile-save evidence without another read', async () => {
  backend.get.mockResolvedValue(skillDemo)
  const view = mount()
  await waitFor(() => expect(view.result.current.snapshot).toEqual(skillDemo))
  const saved = { ...skillDemo, conversation_count: skillDemo.conversation_count + 1 }
  backend.save.mockResolvedValueOnce(saved)
  await act(async () => { await useSkillEvidenceStore.getState().save({ version: 1, revision: 1, learner_id: skillDemo.learner_id, target: skillDemo.target, focus: null, excluded_attempts: [] }) })
  expect(view.result.current.snapshot).toBe(saved)
  expect(backend.get).toHaveBeenCalledTimes(1)
})

it('does not publish a profile-save result into a newly selected language', async () => {
  backend.get.mockResolvedValue(skillDemo)
  mount()
  await waitFor(() => expect(useSkillEvidenceStore.getState().snapshot).toEqual(skillDemo))
  let finish!: (value: SkillSnapshot) => void
  backend.save.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
  const saving = useSkillEvidenceStore.getState().save({ version: 1, revision: 1, learner_id: skillDemo.learner_id, target: skillDemo.target, focus: null, excluded_attempts: [] })
  const other = { ...skillDemo, target: 'french' }
  backend.get.mockResolvedValue(other)
  act(() => seed('french', 1))
  await waitFor(() => expect(useSkillEvidenceStore.getState().snapshot).toBe(other))
  await act(async () => { finish(skillDemo); await saving })
  expect(useSkillEvidenceStore.getState().snapshot?.target).toBe('french')
})

it('reads once for a language and a settings revision, however many surfaces ask', async () => {
  backend.get.mockResolvedValue(skillDemo)
  const views = [mount(), mount(), mount()]
  await waitFor(() => expect(views[2].result.current.snapshot).toEqual(skillDemo))
  expect(backend.get).toHaveBeenCalledTimes(1)
  expect(backend.get).toHaveBeenCalledWith(skillDemo.target)
})

it('coalesces a burst of reloads into one trailing read', async () => {
  let resolve: (value: SkillSnapshot) => void = () => { throw new Error('No request') }
  backend.get.mockImplementationOnce(() => new Promise<SkillSnapshot>(done => { resolve = done })).mockResolvedValue(skillDemo)
  const view = mount()
  await waitFor(() => expect(backend.get).toHaveBeenCalledTimes(1))
  // The conversation advancing is the signal; three of them while a read is in
  // flight must become one more read, not three.
  act(() => {
    useSkillEvidenceStore.getState().reload()
    useSkillEvidenceStore.getState().reload()
    useSkillEvidenceStore.getState().reload()
  })
  expect(backend.get).toHaveBeenCalledTimes(1)
  await act(async () => { resolve(skillDemo) })
  await waitFor(() => expect(backend.get).toHaveBeenCalledTimes(2))
  expect(view.result.current.snapshot).toEqual(skillDemo)
})

it('hides evidence read under settings that no longer apply', async () => {
  backend.get.mockResolvedValueOnce(skillDemo)
  const view = mount()
  await waitFor(() => expect(view.result.current.snapshot).toEqual(skillDemo))
  // The next revision's read never answers, so the only snapshot the store holds
  // is the one read under the revision that no longer applies.
  backend.get.mockImplementationOnce(() => new Promise<SkillSnapshot>(() => {}))
  act(() => { seed(skillDemo.target, 1) })
  expect(view.result.current.snapshot).toBeNull()
  expect(backend.get).toHaveBeenCalledTimes(2)
})

it('ignores a late answer for the language that was left', async () => {
  let finishOld: (value: SkillSnapshot) => void = () => { throw new Error('Request not started') }
  const arabic = { ...skillDemo, target: 'arabic' }
  backend.get.mockImplementationOnce(() => new Promise<SkillSnapshot>(resolve => { finishOld = resolve })).mockResolvedValueOnce(arabic)
  const view = mount()
  await waitFor(() => expect(backend.get).toHaveBeenCalledTimes(1))
  act(() => { seed('arabic', 1) })
  await waitFor(() => expect(view.result.current.snapshot?.target).toBe('arabic'))
  await act(async () => { finishOld(skillDemo) })
  expect(view.result.current.snapshot?.target).toBe('arabic')
})

it('shows a snapshot only for the language and settings revision it was read for', () => {
  const state = { ...useSkillEvidenceStore.getState(), snapshot: skillDemo, scope: 3 }
  expect(selectSkillSnapshot(state, skillDemo.target, 3)).toEqual(skillDemo)
  expect(selectSkillSnapshot(state, 'french', 3)).toBeNull()
  expect(selectSkillSnapshot(state, skillDemo.target, 4)).toBeNull()
  expect(selectSkillSnapshot(state, undefined, 3)).toBeNull()
})

it('returning to cached evidence abandons a different language and reloads the displayed scope', async () => {
  let finish!: (value: SkillSnapshot) => void
  backend.get.mockResolvedValueOnce(skillDemo)
    .mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    .mockResolvedValueOnce(skillDemo)
  useSkillEvidenceStore.getState().load(skillDemo.target, 0)
  await waitFor(() => expect(useSkillEvidenceStore.getState().snapshot).toEqual(skillDemo))
  useSkillEvidenceStore.getState().load('french', 0)
  useSkillEvidenceStore.getState().reload()
  useSkillEvidenceStore.getState().load(skillDemo.target, 0)
  await act(async () => finish({ ...skillDemo, target: 'french' }))
  expect(useSkillEvidenceStore.getState().snapshot).toEqual(skillDemo)
  expect(backend.get).toHaveBeenCalledTimes(2)
  useSkillEvidenceStore.getState().reload()
  expect(backend.get).toHaveBeenLastCalledWith(skillDemo.target)
  await waitFor(() => expect(useSkillEvidenceStore.getState().read?.pending).toBe(false))
})

it('an abandoned request cannot consume the current scope’s trailing reload', async () => {
  let finishOld!: (value: SkillSnapshot) => void
  let finishNew!: (value: SkillSnapshot) => void
  backend.get.mockImplementationOnce(() => new Promise(resolve => { finishOld = resolve }))
    .mockImplementationOnce(() => new Promise(resolve => { finishNew = resolve }))
    .mockResolvedValueOnce({ ...skillDemo, target: 'french' })
  useSkillEvidenceStore.getState().load(skillDemo.target, 0)
  useSkillEvidenceStore.getState().reload()
  useSkillEvidenceStore.getState().load('french', 0)
  useSkillEvidenceStore.getState().reload()
  await act(async () => finishOld(skillDemo))
  expect(backend.get).toHaveBeenCalledTimes(2)
  await act(async () => finishNew({ ...skillDemo, target: 'french' }))
  expect(backend.get).toHaveBeenCalledTimes(3)
  expect(backend.get).toHaveBeenLastCalledWith('french')
  expect(useSkillEvidenceStore.getState().snapshot?.target).toBe('french')
})

it('resets desired scope, requests and queued reloads with the public store state', async () => {
  let finish!: (value: SkillSnapshot) => void
  backend.get.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  useSkillEvidenceStore.getState().load(skillDemo.target, 0)
  useSkillEvidenceStore.getState().reload()
  useSkillEvidenceStore.setState(useSkillEvidenceStore.getInitialState(), true)
  await act(async () => finish(skillDemo))
  useSkillEvidenceStore.getState().reload()
  expect(backend.get).toHaveBeenCalledTimes(1)
  expect(useSkillEvidenceStore.getState().snapshot).toBeNull()
})
