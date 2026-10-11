import { DEFAULT_EXECUTION, DEFAULT_APPEARANCE } from '../../generated/contracts'
// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AccessSettings, Command, ConnectionConfig, Snapshot } from '../../generated/contracts'
import type { Settings } from '../../types'
const backend = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => backend)
vi.mock('../diagnostics/log', () => ({ logDebug: vi.fn(), logDiagnostic: vi.fn(), logError: vi.fn(), logInfo: vi.fn(), logWarn: vi.fn() }))
import { getSettings, saveSettings } from './tauri'
import { validateAudioVolumes } from '../../domain/audio/audio-settings'

function directory(): Snapshot {
  return {
    savedTopics: [], sessionId: 'session', revision: 20,
    learner: { id: 'learner', name: 'Learner', revision: 9, preferences: { execution: { ...DEFAULT_EXECUTION }, appearance: { ...DEFAULT_APPEARANCE }, explanationVarietyId: 'english-united-states', interfaceLocale: 'english', myLanguages: [], targetVarieties: {}, theme: 'dark', explanationLanguage: 'english', textSize: 125, textSpacing: 3, highContrast: true, onboarding: 'completed', onboardingRequired: false, onboardingLanguage: null, onboardingHelp: false } },
    personas: [], contacts: [], languages: [], languageProfiles: [],
    conversations: ['a', 'b'].map((id, index) => ({
      id, contactId: 'contact', languageId: index ? 'french' : 'spanish', title: id,
      archived: false, revision: 5, settingsRevision: index + 6, lastUsed: 10 - index, createdAt: '2026-09-10',
      settings: { direction: { topic: null, timeReference: 'any', usePersonaDetails: true }, difficulty: 'advanced', explanationLanguage: 'english', varietyId: index ? 'french-france' : 'spanish-mexico', explanationVarietyId: 'english-united-states', composingHelp: 'generous', coachProactivity: 'occasional', translation: true, pronunciation: false, romanization: true, autoSend: true, readAloud: true, speechVoice: 'alloy' },
    })),
  }
}
it('shares a settings projection requested by multiple surfaces in the same burst', async () => {
  await Promise.all([getSettings(), getSettings(), getSettings()])
  for (const command of ['get_snapshot', 'get_connection', 'get_access_settings', 'get_reward_settings', 'get_playback_rate', 'get_microphone']) {
    expect(backend.invoke.mock.calls.filter(([name]) => name === command)).toHaveLength(1)
  }
  await getSettings()
  expect(backend.invoke.mock.calls.filter(([name]) => name === 'get_snapshot')).toHaveLength(2)
})

let workspace: Snapshot
let connection: ConnectionConfig
let access: AccessSettings
let microphone: string | null
function commands(): Command[] {
  return backend.invoke.mock.calls.filter(([name]) => name === 'execute_command').map(([, args]) => args.command as Command)
}

it('saves execution choices only on the learner and preserves concurrent changes to another choice', async () => {
  const baseline = await getSettings()
  workspace.learner.preferences.execution.reading = 'automatic'
  workspace.learner.revision++
  await saveSettings({ ...baseline, execution: { ...baseline.execution!, assessment: 'on_demand' } }, baseline)
  expect(commands()).toHaveLength(1)
  expect(commands()[0].action).toEqual({ kind: 'updateLearner', name: workspace.learner.name, expectedRevision: 10,
    preferences: { ...workspace.learner.preferences, execution: { ...DEFAULT_EXECUTION, reading: 'automatic', assessment: 'on_demand' } } })
})
beforeEach(() => {
  backend.invoke.mockReset()
  workspace = directory()
  microphone = null
  connection = { route: 'hosted', revision: 4, signedIn: true, email: 'person@example.invalid', configured: true, assessmentAdapter: 'jev_choice' as const, standardModel: 'configured-model', fastModel: 'fast-model', audio: { transcription: { model: 'whisper-large-v3' }, speech: { model: 'openai/gpt-audio-mini' } }, paused: false }
  access = { customUrlIsUnsavedDefault: false, revision: 4, customKeyConfigured: true, custom: { baseUrl: 'https://example.invalid/v1', bearerAuth: true } }
  backend.invoke.mockImplementation(async (name: string, args?: { command: Command }) => {
    if (name === 'get_snapshot') return workspace
    if (name === 'get_connection') return connection
    if (name === 'get_access_settings') return access
    if (name === 'get_playback_rate') return 1
    if (name === 'save_playback_rate') return undefined
    if (name === 'get_microphone') return microphone
    if (name === 'save_microphone') { microphone = (args as unknown as { device: string | null }).device; return undefined }
    if (name === 'get_reward_settings') return { revision: 0, fastMode: true, rewardSounds: 'follow_tts', masterVolume: 100, voiceVolume: 100, effectsVolume: 100 }
    if (name === 'save_reward_settings') return undefined
    if (name === 'execute_command') return { actionId: args!.command.actionId, entityId: 'updated', revision: 21 }
    throw new Error(`Unexpected native command: ${name}`)
  })
})

describe('native settings projection', () => {
  it.each([['hosted', 'hosted'], ['custom', 'custom']] as const)('maps %s from three read-only snapshots with no credentials', async (route, projected) => {
    connection.route = route
    const settings = await getSettings()
    expect(backend.invoke.mock.calls.map(([name]) => name).sort()).toEqual(['get_access_settings', 'get_connection', 'get_microphone', 'get_playback_rate', 'get_reward_settings', 'get_snapshot'])
    expect(settings).toMatchObject({
      scope: { sessionId: 'session', conversationId: 'a', settingsRevision: 6, learnerRevision: 9 },
      provider_mode: projected, hosted_email: 'person@example.invalid', standard_model: 'configured-model',
      custom_base_url: 'https://example.invalid/v1', custom_model: 'configured-model',
      target_language: 'spanish', target_variety: 'spanish-mexico', native_language: 'english', native_variety: 'english-united-states', interface_locale: 'english',
      auto_translate: true, always_pronunciation: false, always_romanize: true, text_size: 125, text_spacing: 3,
    })
    for (const field of ['hosted_token', 'install_id', 'openrouter_key', 'groq_key', 'custom_api_key'] as const) expect(settings).not.toHaveProperty(field)
    expect(commands()).toEqual([])
  })

  it('skips archived conversations and fails when none remain', async () => {
    workspace.conversations[0].archived = true
    expect((await getSettings()).scope?.conversationId).toBe('b')
    workspace.conversations[1].archived = true
    await expect(getSettings()).rejects.toThrow('No active conversation')
    expect(commands()).toEqual([])
  })

  it.each(['get_snapshot', 'get_connection', 'get_access_settings'])('propagates %s failure without returning invented settings', async command => {
    const normal = backend.invoke.getMockImplementation()!
    backend.invoke.mockImplementation((name, args) => name === command ? Promise.reject(new Error('Native read failed')) : normal(name, args))
    await expect(getSettings()).rejects.toThrow('Native read failed')
    expect(commands()).toEqual([])
  })
})

describe('scoped native settings writes', () => {
  it('projects voice defaults and saves explicit opt-out on the captured conversation', async () => {
    const settings = await getSettings()
    expect(settings).toMatchObject({ auto_send: true, auto_speak: true, tts_rate: 1 })
    await saveSettings({ ...settings, auto_send: false, auto_speak: false })
    expect(commands()).toHaveLength(1)
    expect(commands()[0].action).toMatchObject({ kind: 'updateSettings', conversationId: 'a', settings: { autoSend: false, readAloud: false, speechVoice: 'alloy' } })
  })

  it('sends only updateSettings to the originally captured conversation after recency changes', async () => {
    const settings = await getSettings()
    workspace.conversations[1].lastUsed = 100
    await saveSettings({ ...settings, auto_translate: false })
    expect(commands()).toEqual([{ sessionId: 'session', actionId: expect.any(String), action: {
      kind: 'updateSettings', conversationId: 'a', expectedRevision: 6,
      settings: { ...workspace.conversations[0].settings, translation: false },
    } }])
  })

  it('sends only updateLearner for display edits and preserves unrelated native preferences', async () => {
    const settings = await getSettings()
    await saveSettings({ ...settings, text_size: 150, text_spacing: 4 })
    expect(commands()).toHaveLength(1)
    expect(commands()[0].action).toEqual({ kind: 'updateLearner', expectedRevision: 9, name: 'Learner', preferences: { ...workspace.learner.preferences, textSize: 150, textSpacing: 4 } })
  })

  it('saves the device-local microphone choice without a workspace command', async () => {
    const settings = await getSettings()
    expect(settings.microphone_device_id).toBeNull()
    await saveSettings({ ...settings, microphone_device_id: 'Microphone (Yeti X)' })
    expect(backend.invoke).toHaveBeenCalledWith('save_microphone', { device: 'Microphone (Yeti X)' })
    expect(commands()).toEqual([])
    const saved = await getSettings()
    expect(saved.microphone_device_id).toBe('Microphone (Yeti X)')
    await saveSettings({ ...saved, microphone_device_id: null })
    expect(microphone).toBeNull()
    expect(backend.invoke).toHaveBeenCalledWith('save_microphone', { device: null })
  })

  it('makes no write when projected values are unchanged', async () => {
    const settings = await getSettings()
    await saveSettings(settings)
    expect(commands()).toEqual([])
  })

  it('saves combined practice and display edits to their respective owners', async () => {
    const settings = await getSettings()
    await saveSettings({ ...settings, auto_translate: false, text_size: 140 })
    expect(commands().map(command => command.action.kind)).toEqual(['updateSettings', 'updateLearner'])
  })

  it('opens the existing target language without rewriting the old conversation', async () => {
    const settings = await getSettings()
    await saveSettings({ ...settings, target_language: 'french' })
    expect(commands().map(c => c.action)).toEqual([{ kind: 'openConversation', conversationId: 'b' }])
  })

  it.each(['session', 'practice'] as const)('rejects stale %s scope with zero command submissions', async changed => {
    const settings = await getSettings()
    if (changed === 'session') workspace.sessionId = 'replacement-session'
    if (changed === 'practice') workspace.conversations[0].settingsRevision++
    await expect(saveSettings({ ...settings, auto_translate: false })).rejects.toThrow(/changed/i)
    expect(commands()).toEqual([])
  })

  it('rejects a deleted captured conversation with zero writes', async () => {
    const settings = await getSettings()
    workspace.conversations = workspace.conversations.filter(c => c.id !== 'a')
    await expect(saveSettings({ ...settings, auto_translate: false })).rejects.toThrow('unavailable')
    expect(commands()).toEqual([])
  })

  it.each(['hosted_token', 'openrouter_key', 'groq_key', 'custom_api_key'] as const)('rejects %s on the preference save path', async field => {
    const settings = await getSettings()
    await expect(saveSettings({ ...settings, [field]: 'synthetic-secret-sentinel', auto_translate: false })).rejects.toThrow('Credentials must use')
    expect(commands()).toEqual([])
    expect(JSON.stringify(backend.invoke.mock.calls)).not.toContain('synthetic-secret-sentinel')
  })

  it('propagates a native revision conflict without retry or a second command', async () => {
    const settings = await getSettings()
    const normal = backend.invoke.getMockImplementation()!
    backend.invoke.mockImplementation((name, args) => name === 'execute_command' ? Promise.reject({ code: 'conflict', message: 'Revision changed' }) : normal(name, args))
    await expect(saveSettings({ ...settings, auto_translate: false })).rejects.toEqual({ code: 'conflict', message: 'Revision changed' })
    expect(commands()).toHaveLength(1)
    expect(commands()[0].action.kind).toBe('updateSettings')
  })
})

describe('independent audio validation', () => {
  it('accepts zero and independent valid channel volumes', () => {
    expect(() => validateAudioVolumes({ master_volume: 0, voice_volume: 45, effects_volume: 90 })).not.toThrow()
    expect(backend.invoke).not.toHaveBeenCalled()
  })

  it.each([undefined, null, NaN, -1, 101, 25.5])('rejects invalid volume %s before saving to native storage', async volume => {
    const settings = await getSettings()
    backend.invoke.mockClear()
    // Keep valid scope: failure must come from audio validation, not a missing scope.
    await expect(saveSettings({ ...settings, master_volume: volume } as Settings)).rejects.toThrow(/master_volume/)
    expect(backend.invoke).not.toHaveBeenCalled()
  })
})


it('rebases an explicit edit over changed learner and conversation revisions', async () => {
  const baseline = await getSettings()
  workspace.learner.revision++
  workspace.conversations[0].settingsRevision++
  workspace.conversations[0].settings.readAloud = false
  await saveSettings({ ...baseline, auto_translate: false }, baseline)
  const sent = commands().find(command => command.action.kind === 'updateSettings')?.action
  expect(sent).toMatchObject({ expectedRevision: workspace.conversations[0].settingsRevision,
    settings: { translation: false, readAloud: false } })
})


it('refreshes and retries a native preference revision conflict', async () => {
  const baseline = await getSettings()
  const original = backend.invoke.getMockImplementation()!
  let attempts = 0
  backend.invoke.mockImplementation(async (name, args) => {
    if (name === 'execute_command' && ++attempts === 1) throw { code: 'conflict', message: 'Revision changed' }
    return original(name, args)
  })
  await saveSettings({ ...baseline, auto_translate: false }, baseline)
  expect(attempts).toBe(2)
})

it.each(['light', 'dark', 'system'] as const)('saves %s appearance to the learner without changing conversation settings', async theme => {
  const settings = await getSettings()
  await saveSettings({ ...settings, theme })
  if (theme === 'dark') expect(commands()).toEqual([])
  else expect(commands().map(command => command.action)).toEqual([{
    kind: 'updateLearner', expectedRevision: 9, name: 'Learner',
    preferences: { ...workspace.learner.preferences, theme },
  }])
})


it('saves explanation variety to the conversation and future defaults without changing interface locale', async () => {
  const settings = await getSettings()
  await saveSettings({ ...settings, native_variety: 'english-united-kingdom' })
  const actions = commands().map(command => command.action)
  expect(actions).toHaveLength(2)
  expect(actions[0]).toMatchObject({ kind: 'updateSettings', settings: { varietyId: 'spanish-mexico', explanationLanguage: 'english', explanationVarietyId: 'english-united-kingdom' } })
  expect(actions[1]).toMatchObject({ kind: 'updateLearner', preferences: { execution: { ...DEFAULT_EXECUTION }, appearance: { ...DEFAULT_APPEARANCE }, explanationVarietyId: 'english-united-kingdom', interfaceLocale: 'english' } })
})
it('saves an explicit target variety as the per-language default', async () => {
  const settings = await getSettings()
  await saveSettings({ ...settings, target_variety: 'spanish-spain' })
  const actions = commands().map(command => command.action)
  expect(actions[0]).toMatchObject({ kind: 'updateSettings', settings: { varietyId: 'spanish-spain', explanationVarietyId: 'english-united-states' } })
  expect(actions[1]).toMatchObject({ kind: 'updateLearner', preferences: { targetVarieties: { spanish: 'spanish-spain' } } })
})

it('saves appearance on the learner without changing conversation settings', async () => {
  const settings = await getSettings()
  const appearance = { ...DEFAULT_APPEARANCE, palette: 'warm' as const }
  await saveSettings({ ...settings, appearance })
  expect(commands()).toHaveLength(1)
  expect(commands()[0].action).toEqual({
    kind: 'updateLearner', expectedRevision: 9, name: 'Learner',
    preferences: { ...workspace.learner.preferences, appearance },
  })
})

it('persists the XP effects switch through the native reward settings contract', async () => {
  const settings = await getSettings()
  await saveSettings({ ...settings, xp_effects: false }, settings)
  expect(backend.invoke).toHaveBeenCalledWith('save_reward_settings', {
    settings: expect.objectContaining({ effectsEnabled: false, effectsVolume: 100, voiceVolume: 100 }),
  })
})
