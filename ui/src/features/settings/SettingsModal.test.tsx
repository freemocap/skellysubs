// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import type { Settings } from '../../types'
import { SettingsModal } from './SettingsModal'
import { I18nProvider } from '../../components/localization/i18n'

const backend = vi.hoisted(() => ({ getSettings: vi.fn(), saveSettings: vi.fn(), invoke: vi.fn() }))
vi.mock('../../platform/ipc/tauri', () => ({ ...backend, isTauri: false, logInfo: vi.fn(), languages: () => [], languageFor: () => null }))
vi.mock('./models/SettingsModels', () => ({ SettingsModels: () => <p>Shared model editor</p> }))
vi.mock('./access/SettingsAccess', () => ({ SettingsAccess: () => <p>AI access</p> }))
vi.mock('./language/VarietyField', () => ({ VarietyField: () => null }))
vi.mock('../../platform/audio/speech', () => ({ setVoiceVolume: vi.fn() }))
const microphones = vi.hoisted(() => ({ listMicrophones: vi.fn() }))
vi.mock('../../platform/audio/microphones', () => microphones)
vi.mock('../../platform/updates/updater', async original => ({ ...await original<typeof import('../../platform/updates/updater')>(), getUpdateChannel: async () => 'stable' }))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '0.13.4' }))
const SETTINGS: Settings = {
  my_languages: [], target_varieties: {},
  provider_mode: 'hosted',

  hosted_email: 'me@example.com',


  custom_base_url: '',

  custom_model: '',

  standard_model: 'google/gemini-2.5-flash',
  observer_model: null,
  target_language: 'spanish-spain',
  target_variety: '',
  native_language: 'english', native_variety: 'english-united-states', interface_locale: 'english',
  microphone_device_id: null,
  auto_speak: false,
  auto_send: false,
  always_romanize: false,
  auto_translate: false,
  always_pronunciation: false,
  text_size: 100,
  text_spacing: 2,
  fast_mode: true, reward_sounds: 'follow_tts',
  master_volume: 100, voice_volume: 100, effects_volume: 100,
  tts_rate: 1,
  shortcuts: { mic: 'ctrl+m', speak: 'ctrl+l', panel: 'ctrl+b', settings: 'ctrl+,' },
}

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.open = true }
  HTMLDialogElement.prototype.close = function () { this.open = false }
  vi.clearAllMocks()
  backend.getSettings.mockResolvedValue({ ...SETTINGS, hosted_email: '' })
  backend.saveSettings.mockResolvedValue(undefined)
  backend.invoke.mockResolvedValue({ capacityBytes: 268435456, usedBytes: 0, resultCount: 0 })
  microphones.listMicrophones.mockResolvedValue({ source: 'native', devices: [] })
})

it('lists microphones, names the system default and saves the chosen device', async () => {
  microphones.listMicrophones.mockResolvedValue({ source: 'native', devices: [
    { id: 'Microphone (USB Camera)', label: 'Microphone (USB Camera)', isDefault: true, channels: 2, sampleRate: 48000, unavailable: null },
    { id: 'Microphone (Yeti X)', label: 'Microphone (Yeti X)', isDefault: false, channels: 2, sampleRate: 48000, unavailable: null },
  ] })
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'microphone' } })
  expect(await screen.findByRole('option', { name: 'System default · Microphone (USB Camera)' })).toBeTruthy()
  expect(microphones.listMicrophones).toHaveBeenCalledWith(false)
  const select = screen.getByRole('option', { name: 'Microphone (Yeti X) · 2 ch · 48,000 Hz' }).closest('select')!
  expect(select.matches(':disabled')).toBe(false)
  expect(select.closest('fieldset')).toBeNull()
  fireEvent.change(select, { target: { value: 'Microphone (Yeti X)' } })
  await waitFor(() => expect(backend.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ microphone_device_id: 'Microphone (Yeti X)' }), expect.anything()))
  fireEvent.click(screen.getByRole('button', { name: 'Refresh microphones' }))
  expect(microphones.listMicrophones).toHaveBeenLastCalledWith(true)
})

it('keeps a saved microphone that is no longer connected visible', async () => {
  backend.getSettings.mockResolvedValue({ ...SETTINGS, hosted_email: '', microphone_device_id: 'Microphone (Yeti X)' })
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'microphone' } })
  const missing = await screen.findByRole('option', { name: 'Microphone (Yeti X) (not connected)' }) as HTMLOptionElement
  expect(missing.selected).toBe(true)
})

it('routes explicit update checks to the shared update banner', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'updates' } })
  const check = vi.fn()
  window.addEventListener('skellyspeak-check-update', check)
  fireEvent.click(await screen.findByRole('button', { name: 'Check for updates' }))
  expect(check).toHaveBeenCalledOnce()
  fireEvent.click(await screen.findByRole('button', { name: 'v0.13.4' }))
  expect(check).toHaveBeenCalledTimes(2)
  window.removeEventListener('skellyspeak-check-update', check)
})

it('keeps the modal open until a pending preference save finishes', async () => {
  let finish!: () => void
  backend.saveSettings.mockImplementation(() => new Promise<void>(resolve => { finish = resolve }))
  const close = vi.fn()
  render(<SettingsModal onClose={close} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'text' } })
  fireEvent.change(screen.getByLabelText('Text size · 100%'), { target: { value: '125' } })
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(close).not.toHaveBeenCalled()
  await waitFor(() => expect(backend.saveSettings).toHaveBeenCalledOnce())
  await act(async () => finish())
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
  expect(close).toHaveBeenCalledOnce()
})

it('dismisses on the backdrop but keeps settings open for clicks inside', async () => {
  const close = vi.fn()
  const view = render(<SettingsModal onClose={close} />)
  const search = await screen.findByLabelText('Search settings')
  fireEvent.click(search)
  expect(close).not.toHaveBeenCalled()
  fireEvent.click(view.baseElement.querySelector('.modal-backdrop')!)
  expect(close).toHaveBeenCalledOnce()
})

it('saves text size without exposing spacing controls', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'text' } })
  fireEvent.change(screen.getByLabelText('Text size · 100%'), { target: { value: '125' } })
  expect(screen.queryByLabelText(/Text spacing/)).toBeNull()
  await waitFor(() => expect(backend.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ text_size: 125 }), expect.any(Object)))
})

it('groups mobile settings into collapsible sections and searches inside closed groups', async () => {
  const original = window.matchMedia
  window.matchMedia = vi.fn().mockReturnValue({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() })
  try {
    const view = render(<SettingsModal onClose={vi.fn()} />)
    await screen.findByLabelText('Search settings')
    const groups = view.baseElement.querySelectorAll('details.settings-section')
    expect(groups.length).toBeGreaterThan(1)
    for (const group of groups) expect(group).not.toHaveAttribute('open')
    fireEvent.change(screen.getByLabelText('Search settings'), { target: { value: 'microphone' } })
    expect(view.baseElement.querySelector('details.settings-section')).toBeNull()
    expect(screen.getByText('Microphone')).toBeVisible()
  } finally { window.matchMedia = original }
})

it('offers working data controls: saving a copy and deleting', async () => {
  backend.invoke.mockResolvedValue('C:\\Users\\learner\\Downloads\\skellyspeak-backup-7')
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'data' } })
  expect(screen.queryByText('Not connected.')).toBeNull()
  const save = screen.getByRole('button', { name: 'Save a copy of my data' })
  expect(save).toBeEnabled()
  expect(screen.getAllByText('Copies the database, its supporting files and editable configuration to Downloads.').length).toBeGreaterThan(0)
  expect(screen.getByText(/permanently removes/)).toHaveTextContent('editable configuration files')
  expect(screen.getByRole('button', { name: 'Delete my data and close' })).toBeEnabled()
  fireEvent.click(save)
  expect(await screen.findByText('Saved to C:\\Users\\learner\\Downloads\\skellyspeak-backup-7')).toBeInTheDocument()
  expect(backend.invoke).toHaveBeenCalledExactlyOnceWith('export_workspace')
})

it('enables connected audio volume preferences', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'volume' } })
  expect(screen.getByRole('slider', { name: 'Overall volume' })).toBeEnabled()
  expect(screen.getByRole('slider', { name: 'Voice volume' })).toBeEnabled()
  expect(backend.saveSettings).not.toHaveBeenCalled()
})

it('shows a closable error instead of blanking the app when native volume fields are missing', async () => {
  const incomplete = { ...SETTINGS } as Partial<Settings>
  delete incomplete.master_volume
  delete incomplete.voice_volume
  delete incomplete.effects_volume
  backend.getSettings.mockResolvedValue(incomplete)
  const close = vi.fn()
  render(<SettingsModal onClose={close} />)
  expect(await screen.findByRole('alert')).toHaveTextContent('Settings response is missing the required field: master_volume.')
  expect(screen.queryByRole('slider')).toBeNull()
  expect(backend.saveSettings).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
  expect(close).toHaveBeenCalledOnce()
})

it('shows native settings-load failures inside the settings dialog', async () => {
  backend.getSettings.mockRejectedValue(new Error('Settings response is incomplete.'))
  render(<SettingsModal onClose={vi.fn()} />)
  expect(await screen.findByRole('alert')).toHaveTextContent('Settings response is incomplete.')
  expect(screen.getByRole('dialog', { name: 'Settings' })).toBeVisible()
})

it('passes only the edited draft and its baseline to the shared settings writer', async () => {
  const initial = { ...SETTINGS, scope: { sessionId: 's', conversationId: 'c', settingsRevision: 1, learnerRevision: 1, rewardRevision: 0 } }
  const fresh = { ...initial, auto_translate: true, scope: { ...initial.scope, settingsRevision: 2 } }
  backend.getSettings.mockResolvedValueOnce(initial).mockResolvedValue(fresh)
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'romanization' } })
  fireEvent.click(screen.getByLabelText('Show romanization'))
  await waitFor(() => expect(backend.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ always_romanize: true, scope: initial.scope }), initial))
})

it('preserves a newer text-size edit while the first save completes', async () => {
  let finish!: () => void
  backend.saveSettings.mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve }))
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'text size' } })
  fireEvent.change(screen.getByLabelText('Text size · 100%'), { target: { value: '110' } })
  await waitFor(() => expect(backend.saveSettings).toHaveBeenCalledOnce())
  fireEvent.change(screen.getByLabelText('Text size · 110%'), { target: { value: '125' } })
  backend.getSettings.mockResolvedValue({ ...SETTINGS, hosted_email: '', text_size: 110 })
  await act(async () => finish())
  await waitFor(() => expect(backend.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ text_size: 125 }), expect.any(Object)))
})

it('keeps the entire settings interface in its locale when the explanation language differs', async () => {
  backend.getSettings.mockResolvedValue({ ...SETTINGS, native_language: 'french', native_variety: 'french-france', interface_locale: 'german' })
  render(<I18nProvider locale="german"><SettingsModal onClose={vi.fn()} /></I18nProvider>)
  expect(await screen.findByRole('heading', { name: 'Einstellungen' })).toBeVisible()
  expect(screen.queryByRole('heading', { name: 'Paramètres' })).toBeNull()
})

it('exposes theme and palette and autosaves them', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.click(await screen.findByRole('button', { name: /Appearance/ }))
  expect(screen.queryByLabelText('Surface depth')).toBeNull()
  fireEvent.click(screen.getByRole('radio', { name: 'Warm' }))
  await waitFor(() => expect(backend.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({
    appearance: { palette: 'warm', motion: 'system' as const },
  }), expect.any(Object)))
})

it('places shared model selection in its own section and search result', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  const search = await screen.findByLabelText('Search settings')
  fireEvent.change(search, { target: { value: 'models' } })
  expect(screen.getByText('Shared model editor')).toBeVisible()
  expect(screen.queryByText('AI access', { selector: 'p' })).toBeNull()
  fireEvent.change(search, { target: { value: 'AI access' } })
  expect(screen.getByText('AI access', { selector: 'p:not(.settings-group-k)' })).toBeVisible()
  expect(screen.queryByText('Shared model editor')).toBeNull()
})

it('offers an enabled onboarding restart in language settings', async () => {
  const { useOnboardingStore } = await import('../../state/settings/onboarding')
  const restart = vi.fn().mockResolvedValue(undefined)
  useOnboardingStore.setState({ busy: false, reviewSetup: restart })
  const close = vi.fn()
  render(<SettingsModal onClose={close} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'onboarding' } })
  const button = screen.getByRole('button', { name: 'Restart onboarding' })
  expect(button).toBeEnabled()
  fireEvent.click(button)
  await waitFor(() => expect(restart).toHaveBeenCalledOnce())
  await waitFor(() => expect(close).toHaveBeenCalledOnce())
})

it('saves the XP effects switch without changing the sound preference or voice volume', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'XP effects' } })
  const toggle = screen.getByRole('checkbox', { name: 'XP effects' })
  expect(toggle).toBeEnabled()
  expect(toggle).toBeChecked()
  fireEvent.click(toggle)
  await waitFor(() => expect(backend.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ xp_effects: false, reward_sounds: 'follow_tts', voice_volume: 100 }), expect.any(Object)))
})


it('autosaves motion choices and resets to follow system', async () => {
  render(<SettingsModal onClose={vi.fn()} />)
  fireEvent.change(await screen.findByLabelText('Search settings'), { target: { value: 'motion' } })
  expect(screen.getByRole('radio', { name: 'Follow system' })).toHaveAttribute('aria-checked', 'true')
  for (const [label, motion] of [['Full animation', 'full'], ['Reduced motion', 'reduced'], ['Follow system', 'system']]) {
    fireEvent.click(screen.getByRole('radio', { name: label }))
    await waitFor(() => expect(backend.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({
      appearance: expect.objectContaining({ motion }),
    }), expect.any(Object)))
  }
  fireEvent.click(screen.getByRole('radio', { name: 'Full animation' }))
  await waitFor(() => expect(backend.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ appearance: expect.objectContaining({ motion: 'full' }) }), expect.any(Object)))
  fireEvent.click(screen.getByRole('button', { name: 'Reset appearance' }))
  await waitFor(() => expect(backend.saveSettings).toHaveBeenLastCalledWith(expect.objectContaining({ appearance: { palette: 'cool', motion: 'system' } }), expect.any(Object)))
})
