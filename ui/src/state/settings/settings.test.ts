// @vitest-environment jsdom
import { beforeEach, expect, it, vi } from 'vitest'
import { useSettingsStore } from './settings'
import { reportFault } from '../../platform/diagnostics/faults'
import { SHORTCUT_DEFAULTS } from '../../domain/input/keyboard'
import type { Settings } from '../../types'

const native = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn() }))
vi.mock('../../platform/ipc/tauri', () => ({ getSettings: native.get, saveSettings: native.save, languageFor: () => null, languages: () => [{ code: 'french', defaultVariety: 'french-france' }, { code: 'german', defaultVariety: 'german-germany' }] }))
vi.mock('../../platform/diagnostics/faults', () => ({ reportFault: vi.fn() }))

/// The record Rust owns. Only the fields a test reads are meaningful; a write
/// sends the whole thing back, which is what the assertions compare.
const record = (over: Partial<Settings> = {}): Settings => ({
  my_languages: [], target_varieties: {},
  provider_mode: 'custom', hosted_email: '',
  custom_base_url: '', custom_model: '', standard_model: '', observer_model: null,
  target_language: 'spanish', target_variety: '', native_language: 'english', native_variety: 'english-united-states', interface_locale: 'english', microphone_device_id: null,
  auto_speak: false, auto_send: false, always_romanize: false, auto_translate: false,
  text_size: 100, text_spacing: 100, always_pronunciation: false, fast_mode: true,
  reward_sounds: 'follow_tts', master_volume: 1, voice_volume: 1, effects_volume: 1,
  tts_rate: 1, shortcuts: SHORTCUT_DEFAULTS,
  ...over,
})

beforeEach(() => {
  native.get.mockReset()
  native.save.mockReset()
  native.save.mockResolvedValue(undefined)
  vi.mocked(reportFault).mockClear()
})

it('reads the record without counting a read as a change', async () => {
  native.get.mockResolvedValue(record())
  await useSettingsStore.getState().load()
  expect(useSettingsStore.getState().settings?.target_language).toBe('spanish')
  // The revision means "a write has landed": a read must not look like one, or
  // every mount would clear the shell's "you changed something" effects.
  expect(useSettingsStore.getState().revision).toBe(0)
})

it('counts a read of something another surface wrote as a change', async () => {
  native.get.mockResolvedValue(record())
  await useSettingsStore.getState().refresh()
  expect(useSettingsStore.getState().revision).toBe(1)
  expect(useSettingsStore.getState().evidenceRevision).toBe(1)
})

it.each([
  { theme: 'dark' as const },
  { appearance: { palette: 'warm' as const, motion: 'system' as const } },
])('retains the evidence scope for appearance-only writes: %j', async change => {
  const scope = { sessionId: 'session', conversationId: 'chat', settingsRevision: 2, learnerRevision: 3, rewardRevision: 4 }
  const current = record({ theme: 'light', appearance: { palette: 'cool', motion: 'system' as const }, scope })
  const saved = record({ ...current, ...change, scope: { ...scope, learnerRevision: 4 } })
  native.get.mockResolvedValue(current)
  await useSettingsStore.getState().load()
  native.get.mockResolvedValueOnce(current).mockResolvedValue(saved)
  await useSettingsStore.getState().update(settings => ({ ...settings, ...change }), 'Changing appearance')
  expect(native.save).toHaveBeenCalledWith({ ...current, ...change }, current)
  expect(useSettingsStore.getState().settings).toEqual(saved)
  expect(useSettingsStore.getState().revision).toBe(1)
  expect(useSettingsStore.getState().evidenceRevision).toBe(0)
})

it.each([
  { target_language: 'french' },
  { fast_mode: false },
  { scope: { sessionId: 'other', conversationId: 'chat', settingsRevision: 2, learnerRevision: 4, rewardRevision: 4 } },
  { scope: { sessionId: 'session', conversationId: 'other', settingsRevision: 2, learnerRevision: 4, rewardRevision: 4 } },
  { scope: { sessionId: 'session', conversationId: 'chat', settingsRevision: 3, learnerRevision: 4, rewardRevision: 4 } },
  { scope: { sessionId: 'session', conversationId: 'chat', settingsRevision: 2, learnerRevision: 4, rewardRevision: 5 } },
])('invalidates evidence when an appearance save also changes its scope: %j', async change => {
  const current = record({ theme: 'light', scope: { sessionId: 'session', conversationId: 'chat', settingsRevision: 2, learnerRevision: 3, rewardRevision: 4 } })
  native.get.mockResolvedValue(current)
  await useSettingsStore.getState().load()
  native.get.mockResolvedValue(record({ ...current, theme: 'dark', ...change }))
  await useSettingsStore.getState().refresh()
  expect(useSettingsStore.getState().evidenceRevision).toBe(1)
})

it('writes a preference against a fresh read and adopts what Rust reports', async () => {
  const current = record()
  const saved = record({ auto_speak: true })
  native.get.mockResolvedValueOnce(current).mockResolvedValue(saved)
  await useSettingsStore.getState().setPreference('auto_speak')
  // The read is the baseline, so a cached copy cannot overwrite another writer.
  expect(native.save).toHaveBeenCalledWith({ ...current, auto_speak: true }, current)
  expect(useSettingsStore.getState().settings).toEqual(saved)
  expect(useSettingsStore.getState().revision).toBe(1)
  expect(useSettingsStore.getState().savingPreference).toBe(false)
})

it('selects the configured variety when the target language changes', async () => {
  const current = record({ target_variety: 'spanish-latin-america' })
  native.get.mockResolvedValue(current)
  await useSettingsStore.getState().setLanguage('target_language', 'french')
  expect(native.save).toHaveBeenCalledWith({ ...current, target_language: 'french', target_variety: 'french-france' }, current)
})

it('keeps the target variety when the native language changes', async () => {
  const current = record({ target_variety: 'spanish-latin-america' })
  native.get.mockResolvedValue(current)
  await useSettingsStore.getState().setLanguage('native_language', 'german')
  expect(native.save).toHaveBeenCalledWith({ ...current, native_language: 'german', native_variety: 'german-germany' }, current)
})

it('runs one language write at a time', async () => {
  let release: (value: Settings) => void = () => { throw new Error('No request') }
  native.get.mockImplementationOnce(() => new Promise<Settings>(done => { release = done })).mockResolvedValue(record())
  const first = useSettingsStore.getState().setLanguage('native_language', 'french')
  expect(useSettingsStore.getState().savingLanguage).toBe(true)
  await useSettingsStore.getState().setLanguage('native_language', 'german')
  expect(native.save).not.toHaveBeenCalled()
  release(record())
  await first
  expect(native.save).toHaveBeenCalledTimes(1)
  expect(useSettingsStore.getState().savingLanguage).toBe(false)
})

it('reports a failed write and leaves the record it had', async () => {
  native.get.mockResolvedValue(record())
  native.save.mockRejectedValue(new Error('locked'))
  await useSettingsStore.getState().setPreference('fast_mode')
  expect(reportFault).toHaveBeenCalledWith('Saving reading preference', expect.any(Error))
  expect(useSettingsStore.getState().revision).toBe(0)
  expect(useSettingsStore.getState().savingPreference).toBe(false)
})

it('writes a text size only when the action changes it', async () => {
  native.get.mockResolvedValue(record({ text_size: 160 }))
  useSettingsStore.getState().changeTextSize('increase')
  await vi.waitFor(() => expect(native.get).toHaveBeenCalled())
  // Already at the limit: there is nothing to write.
  expect(native.save).not.toHaveBeenCalled()
  native.get.mockResolvedValue(record({ text_size: 100 }))
  useSettingsStore.getState().changeTextSize('decrease')
  await vi.waitFor(() => expect(native.save).toHaveBeenCalledTimes(1))
  expect(native.save.mock.calls[0][0].text_size).toBe(95)
})

it('keeps the newest read and UI language when an older read completes last', async () => {
  let finish!: (value: Settings) => void
  native.get.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    .mockResolvedValueOnce(record({ native_language: 'french', native_variety: 'french-france', interface_locale: 'french', target_language: 'french' }))
  const old = useSettingsStore.getState().load()
  await useSettingsStore.getState().load()
  finish(record({ native_language: 'spanish' }))
  await old
  expect(useSettingsStore.getState().settings?.target_language).toBe('french')
  expect(document.documentElement.lang).toBe('fr')
})

it('a post-save read cannot overwrite a later conversation read', async () => {
  let finish!: (value: Settings) => void
  native.get.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    .mockResolvedValueOnce(record({ target_language: 'french' }))
  const save = useSettingsStore.getState().save(record(), record())
  await vi.waitFor(() => expect(native.get).toHaveBeenCalledTimes(1))
  await useSettingsStore.getState().load()
  finish(record())
  await save
  expect(useSettingsStore.getState().settings?.target_language).toBe('french')
  expect(useSettingsStore.getState().revision).toBe(1)
  native.get.mockResolvedValue(record({ target_language: 'french' }))
  await useSettingsStore.getState().load()
  expect(useSettingsStore.getState().revision).toBe(1)
})

it('a store reset invalidates pending reads', async () => {
  let finish!: (value: Settings) => void
  native.get.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
  const old = useSettingsStore.getState().load()
  useSettingsStore.setState(useSettingsStore.getInitialState(), true)
  finish(record())
  await old
  expect(useSettingsStore.getState().settings).toBeNull()
})


it.each([true, false, undefined])('toggles saved XP effects from %s through the shared preference writer', async initial => {
  const current = record({ xp_effects: initial })
  const saved = record({ xp_effects: initial === false })
  native.get.mockResolvedValueOnce(current).mockResolvedValue(saved)
  await useSettingsStore.getState().setPreference('xp_effects')
  expect(native.save).toHaveBeenCalledWith({ ...current, xp_effects: initial === false }, current)
  expect(useSettingsStore.getState().settings).toEqual(saved)
})
