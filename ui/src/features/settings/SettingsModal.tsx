import { AppLanguageFields } from './language/AppLanguageFields'
import { useExecutionPreferences } from '../../state/settings/useExecutionPreferences'
import { ErrorNotice } from '../../components/feedback/ErrorNotice'
import { errorMessage } from '../../platform/diagnostics/error-details'
import { ShowHelp } from './onboarding/ShowHelp'
import { TEXT_SIZE } from '../../generated/contracts'
import { AppearanceSettings } from './appearance/AppearanceSettings'
import { messageKey } from '../../domain/localization'
import { useI18n } from '../../components/localization/i18n'
import { InfoTip } from '../../components/controls/InfoTip'
import { configureAudioVolumes } from '../../platform/audio/audio-volume'
import { configureRewardSounds } from '../../platform/audio/reward-sounds'
import { MicrophoneSelector } from '../../components/media/MicrophoneSelector'
import { MicrophoneCheck } from '../../components/media/MicrophoneCheck'
import { useCallback, useEffect, useState, type ReactNode } from 'react'
import type { Settings, Shortcuts } from '../../types'
import { logInfo, languages } from '../../platform/ipc/tauri'
import { comboFromEvent, SHORTCUT_DEFAULTS, type ShortcutAction } from '../../domain/input/keyboard'
import { VarietyField } from './language/VarietyField'
import { t, type UiLang } from '../../domain/localization'
import { useIsMobile } from '../../components/layout/useIsMobile'
import { reportFault } from '../../platform/diagnostics/faults'
import { SettingsDialog } from './SettingsDialog'
import { ToolbarIcon, type ToolbarIconName } from '../../components/controls/ToolbarIcon'
import { openOverlay } from '../../domain/input/back'
import { useSettingsStore } from '../../state/settings/settings'
import { languageLabel } from '../../domain/language/language-label'
import { appVersion as loadAppVersion, openDownloads } from '../../platform/updates/updater'

import { SettingsModels } from './models/SettingsModels'
import { AiExecutionSettings } from '../../components/controls/AiExecutionSettings'
import { SettingsAccess } from './access/SettingsAccess'
import { FactoryReset } from './workspace/FactoryReset'
import { InferenceCacheSettings } from './workspace/InferenceCacheSettings'
import { SaveDataCopy } from '../../components/persistence/SaveDataCopy'

type SaveState = 'idle' | 'pending' | 'saving' | 'saved' | 'error'

/// How long an edit rests before it is written. Long enough that typing into a
/// text field is one write rather than one per keystroke, short enough that
/// closing the modal straight after a change still catches it.
const AUTOSAVE_DEBOUNCE_MS = 500

type SectionId = 'models' | 'keys' | 'languages' | 'voice' | 'shortcuts' | 'updates' | 'reading' | 'appearance' | 'data'

function SaveStatus({ state }: { state: SaveState }) {
  const tr = useI18n()
  if (state === 'error')
    return (
      <ErrorNotice as="span" error={state} className="save-status error">
        {tr("Not saved — check the logs")}</ErrorNotice>
    )
  if (state === 'saving' || state === 'pending')
    return <span className="save-status">{tr("Saving…")}</span>
  if (state === 'saved') return <span className="save-status saved">{tr("Saved ✓")}</span>
  return <span className="save-status hint">{tr("Changes save automatically")}</span>
}

/// Shortcut recorder: click to arm, press a combo. Esc resets to default.
function ShortcutField({
  label,
  action,
  value,
  ui,
  onChange,
}: {
  label: string
  action: ShortcutAction
  value: string
  ui: UiLang
  onChange: (v: string) => void
}) {
  const [recording, setRecording] = useState(false)
  // Shortcut labels localize via settings.sc.<action>; falls back to the
  // English label when the key is missing.
  const displayLabel = t(ui, label)
  return (
    <div className="shortcut-field">
      <span className="shortcut-label">{displayLabel}</span>
      <input
        data-shortcut-capture={recording || undefined}
        className="shortcut-input"
        value={recording ? t(ui, 'press keys…') : value || SHORTCUT_DEFAULTS[action]}
        readOnly
        onFocus={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => {
          e.preventDefault()
          e.stopPropagation()
          if (e.key === 'Escape') {
            onChange(SHORTCUT_DEFAULTS[action])
            ;(e.target as HTMLInputElement).blur()
            return
          }
          if (e.key === 'Control' || e.key === 'Alt' || e.key === 'Shift' || e.key === 'Meta')
            return
          onChange(comboFromEvent(e))
          ;(e.target as HTMLInputElement).blur()
        }}
      />
    </div>
  )
}

const SECTIONS: { id: SectionId; labelKey: string; icon: ToolbarIconName; descKey: string }[] = [
  { id: 'appearance', labelKey: messageKey('Appearance'), icon: 'appearance', descKey: messageKey('Colors, spacing, depth and reading size') },
  { id: 'reading', labelKey: messageKey('Reading & display'), icon: 'reading', descKey: messageKey('Text size, spacing, and reading aids') },
  {
    id: 'keys',
    labelKey: messageKey('AI access'),
    icon: 'key',
    descKey: messageKey('Hosted sign-in or a custom server'),
  },
  { id: 'models', labelKey: messageKey('Models'), icon: 'models', descKey: messageKey('Models') },
  {
    id: 'languages',
    labelKey: "Languages",
    icon: 'globe',
    descKey: "What you're learning and what you already speak.",
  },
  {
    id: 'voice',
    labelKey: "Audio & Voice",
    icon: 'voice',
    descKey: "Microphone, speech playback, and transcription behavior.",
  },
  {
    id: 'shortcuts',
    labelKey: "Shortcuts",
    icon: 'keyboard',
    descKey: "Click a field and press the combo. Esc resets to default.",
  },
  {
    id: 'updates',
    labelKey: "Updates",
    icon: 'update',
    descKey: "Application version and updates.",
  },
  { id: 'data', labelKey: messageKey('Your data'), icon: 'data', descKey: messageKey('Save a copy, or delete everything and start over') },
]

const SECTION_LABEL_KEY: Record<SectionId, string> = Object.fromEntries(
  SECTIONS.map((s) => [s.id, s.labelKey])
) as Record<SectionId, string>

interface RowDef {
  section: SectionId
  label: string
  kw: string
  node: ReactNode
  /// Rows that do not apply to the current configuration — e.g. the custom
  /// server fields while the provider is set to cloud. Hidden everywhere,
  /// search included: offering a field that is ignored is worse than absent.
  hidden?: boolean
}

const SHORTCUT_ROWS: { action: ShortcutAction; label: string }[] = [
  { action: 'mic', label: messageKey('Toggle microphone') },
  { action: 'speak', label: messageKey('Speak last reply') },
  { action: 'panel', label: messageKey('Toggle analysis panel') },
  { action: 'settings', label: messageKey('Open settings') },
]

export function SettingsModal({
  onClose: closeModal,
  onBusyChange,
}: {
  onClose: () => void
  onBusyChange?: (busy: boolean) => void
}) {
  const executionPreferences = useExecutionPreferences()
  const tr = useI18n()
  const [settings, setSettings] = useState<Settings | null>(null)
  useEffect(() => { if (settings) configureRewardSounds(settings.xp_effects === false ? 'no' : settings.reward_sounds, settings.auto_speak) }, [settings?.xp_effects, settings?.reward_sounds, settings?.auto_speak])
  const [loadError, setLoadError] = useState<string | null>(null)
  useEffect(() => {
    if (!settings) return
    try { configureAudioVolumes(settings) }
    catch (error) {
      reportFault('Audio settings', error)
      setLoadError(errorMessage(error))
      setSettings(null)
    }
  }, [settings === null, settings?.master_volume, settings?.voice_volume, settings?.effects_volume])
  // The last state known to be on disk. Autosave fires whenever `settings`
  // drifts from this, and this catches up once the write lands.
  const [persisted, setPersisted] = useState<Settings | null>(null)
  const [saveState, setSaveState] = useState<SaveState>('idle')
  const [routeBusy, setRouteBusy] = useState(false)
  const [modelsBusy, setModelsBusy] = useState(false)
  const accessBusy = routeBusy || modelsBusy
  const [configurationRevision, setConfigurationRevision] = useState(0)
  const [appVersion, setAppVersion] = useState<string | null>(null)
  const [section, setSection] = useState<SectionId>('keys')
  const [search, setSearch] = useState('')
  const isMobile = useIsMobile()
  // The app's UI language follows the learner's NATIVE language.
  const ui = tr.locale

  useEffect(() => {
    logInfo('[settings] modal opened')
    // The store owns the record; this is a re-read so the draft starts from what
    // Rust holds right now. It does not count as a settings change.
    void useSettingsStore.getState().load()
      .then(() => {
        const s = useSettingsStore.getState().settings
        if (!s) throw new Error('Settings are still loading.')
        setSettings(s)
        setPersisted(s)
        logInfo('[settings] loaded')
      })
      .catch((e: unknown) => {
        reportFault('Loading settings', e)
        setLoadError(String(e instanceof Error ? e.message : e))
        setSettings(null)
      })
  }, [])
  useEffect(() => { void loadAppVersion().then(setAppVersion).catch(error => reportFault('Loading application version', error)) }, [])

  // AI access writes its own commands, so it re-reads through the store. That is
  // a real change to the record, hence `refresh` rather than `reload`.
  const refreshFromBackend = useCallback(async () => {
    setConfigurationRevision(value => value + 1)
    await useSettingsStore.getState().refresh()
    const fresh = useSettingsStore.getState().settings
    if (fresh) { setSettings(fresh); setPersisted(fresh) }
  }, [])

  // ── Autosave ────────────────────────────────────────────────────────────
  // Supported preference edits save after a short pause; access owns its own writes.
  const dirty = !!settings && !!persisted && JSON.stringify(settings) !== JSON.stringify(persisted)

  useEffect(() => {
    if (!settings || !dirty) return
    // Serialize writes and require explicit retry after failure.
    if (saveState === 'saving' || saveState === 'error') return
    const timer = setTimeout(() => {
      setSaveState('saving')
      logInfo('[settings] autosaving')
      Promise.resolve().then(async () => {
        if (!persisted) throw new Error('Settings are still loading.')
        // The store writes and adopts the result, so every reader of the record
        // sees it without the modal having to announce the change.
        return useSettingsStore.getState().save(settings, persisted)
      })
        .then((fresh) => {
          setSettings(current => {
            if (!current) return fresh
            const next = { ...fresh }
            for (const key of Object.keys(current) as (keyof Settings)[]) {
              if (key !== 'scope' && JSON.stringify(current[key]) !== JSON.stringify(settings[key])) Object.assign(next, { [key]: current[key] })
            }
            return next
          })
          setPersisted(fresh)
          setSaveState('saved')
        })
        .catch((e) => {
          // Preserve the draft on failure and report it in the shared fault bar.
          reportFault('Saving settings', e)
          setSaveState('error')
        })
    }, AUTOSAVE_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [settings, dirty, saveState])

  const onClose = useCallback(() => {
    if (accessBusy || dirty || saveState === 'saving') return
    closeModal()
  }, [accessBusy, dirty, saveState, closeModal])

  useEffect(() => { onBusyChange?.(accessBusy || dirty || saveState === 'saving') }, [accessBusy, dirty, saveState, onBusyChange])
  useEffect(() => openOverlay(onClose), [onClose])

  // Let "Saved" fade back to nothing so the footer is not permanently shouting.
  useEffect(() => {
    if (saveState !== 'saved') return
    const t = setTimeout(() => setSaveState('idle'), 1800)
    return () => clearTimeout(t)
  }, [saveState])

  if (!settings) {
    return (
      <SettingsDialog onClose={onClose} title={tr("Settings")}>
        <div className="settings-modal">
          {loadError ? <><ErrorNotice as="p" error={loadError}>{loadError}</ErrorNotice><button type="button" className="btn" onClick={onClose}>{tr("Close")}</button></> : <p className="center-note">{tr("Loading…")}</p>}
        </div>
      </SettingsDialog>
    )
  }

  const setShortcuts = (patch: Partial<Shortcuts>) =>
    setSettings({ ...settings, shortcuts: { ...settings.shortcuts, ...patch } })

  // Localized display label for a registry row (English label = search index).

  // ── Row registry: adding a setting = one entry here ──────────────────────
  // Display labels localize via the settings.row.<id> convention (English
  // fallbacks double as the search index).
  const rows: Record<string, RowDef> = {
    conversation_help: {
      section: 'languages', label: tr('Restart onboarding'), kw: 'onboarding tutorial help guide',
      node: <ShowHelp onShown={onClose} />,
    },
    models: {
      section: 'models', label: tr('Models'), kw: 'model standard fast transcription automatic on demand assessment coaching reading brief',
      node: <div inert={routeBusy}><AiExecutionSettings {...executionPreferences} /><SettingsModels refreshKey={configurationRevision} onBusyChange={setModelsBusy} onChanged={refreshFromBackend} /></div>,
    },
    provider_mode: {
      section: 'keys', label: tr('AI access'), kw: 'provider server token key account custom hosted',
      node: <div inert={modelsBusy}><SettingsAccess refreshKey={configurationRevision} onBusyChange={setRouteBusy} onChanged={refreshFromBackend} /></div>,
    },
    target_language: {
      section: 'languages',
      label: tr('I want to learn'),
      kw: 'target language learn spanish studying',
      node: (
        <div className="form-row">
          <label>{tr("I want to learn")}</label>
          <select
            value={settings.target_language}
            onChange={(e) => {
              // New language choices start at their configured variety.
              setSettings({ ...settings, target_language: e.target.value, target_variety: languages().find(l => l.code === e.target.value)!.defaultVariety })
            }}
          >
            {languages().map((l) => (
              <option key={l.code} value={l.code}>
                {languageLabel(l, tr.locale)}
              </option>
            ))}
          </select>
        </div>
      ),
    },
    target_variety: {
      section: 'languages',
      label: tr('Variety'),
      kw: 'dialect regional variety accent region levantine mexican',
      node: (
        <div className="form-row">
          <label>{tr("Variety")} <InfoTip>{tr('Speech variety matching is not guaranteed.')}</InfoTip></label>
          <VarietyField
            presets={
              languages().find((l) => l.code === settings.target_language)?.varieties ?? []
            }
            value={settings.target_variety}
            onChange={(v) => setSettings({ ...settings, target_variety: v })}
          />
        </div>
      ),
    },
    interface_locale: {
      section: 'languages', label: tr('App language'), kw: 'interface ui locale native language explanations variety dialect',
      node: <AppLanguageFields value={settings} onChange={choice => setSettings({ ...settings, ...choice })} />,
    },
    audio_volume: {
      section: 'voice', label: tr('Volume'), kw: 'audio master overall volume voice speech tts effects sound mute rewards',
      node: <div className="audio-volume-controls">
        {(['master_volume', 'voice_volume', 'effects_volume'] as const).map((key, index) => <div className={`audio-volume-row${index ? ' audio-volume-channel' : ''}`} key={key}>
          <label htmlFor={key}>{tr(['Overall volume', 'Voice volume', 'Sound effects volume'][index])}</label>
          <input id={key} type="range" min="0" max="100" step="1" value={settings[key]}
            onChange={event => setSettings({ ...settings, [key]: Number(event.target.value) })} />
          <output htmlFor={key}>{settings[key]}%</output>
        </div>)}
        <div className="audio-volume-channel audio-effects-toggles">
          <label className="check-label"><input type="checkbox" checked={settings.reward_sounds !== 'no'}
            onChange={event => setSettings({ ...settings, reward_sounds: event.target.checked ? 'yes' : 'no' })} />{tr("Sound effects")}</label>
          {settings.reward_sounds !== 'no' && <label className="check-label"><input type="checkbox" checked={settings.reward_sounds === 'follow_tts'}
            onChange={event => setSettings({ ...settings, reward_sounds: event.target.checked ? 'follow_tts' : 'yes' })} />{tr("Only with Read aloud")}</label>}
        </div>
      </div>,
    },
    microphone: {
      section: 'voice',
      label: tr('Microphone'),
      kw: 'microphone input device recording yeti',
      node: (
        <>
          <MicrophoneSelector value={settings.microphone_device_id} onChange={microphone_device_id => setSettings({ ...settings, microphone_device_id })} />
          <MicrophoneCheck device={settings.microphone_device_id} />
        </>
      ),
    },
    tts_rate: {
      section: 'voice', label: tr('Voice speed'), kw: 'voice speech speed rate slower faster',
      node: <div className="form-row"><label htmlFor="voice-speed">{tr("Voice speed")}</label><select id="voice-speed" value={settings.tts_rate} onChange={event => setSettings({ ...settings, tts_rate: Number(event.target.value) })}>{[0.5, 0.65, 0.8, 1, 1.2, 1.5].map(rate => <option key={rate} value={rate}>{rate}×</option>)}</select></div>,
    },
    auto_speak: {
      section: 'reading',
      label: tr('Read partner replies aloud'),
      kw: 'auto speak tts voice speech playback audio read aloud',
      node: (
        <div className="form-row check-row">
          <label className="check-label">
            <input
              type="checkbox"
              checked={settings.auto_speak}
              onChange={(e) => setSettings({ ...settings, auto_speak: e.target.checked })}
            />
            <span>{tr("Read partner replies aloud")}</span>
          </label>
        </div>
      ),
    },
    auto_send: {
      section: 'voice',
      label: tr('Auto-send'),
      kw: 'auto send transcription mic speech stt voice input',
      node: (
        <div className="form-row check-row">
          <label className="check-label">
            <input
              type="checkbox"
              checked={settings.auto_send}
              onChange={(e) => setSettings({ ...settings, auto_send: e.target.checked })}
            />
            <span>{tr("Auto-send")}</span>
          </label>
        </div>
      ),
    },
    appearance: { section: 'appearance', label: tr('Appearance'), kw: 'theme light dark system palette color warm cool appearance animation motion follow reduced full', node: <AppearanceSettings settings={settings} onChange={setSettings} /> },
    text_size: {
      section: 'appearance', label: tr('Text size'), kw: 'font text size reading display accessibility',
      node: <div className="form-row"><label htmlFor="reading-size">{tr("Text size · ")}{settings.text_size}%</label>
        <input id="reading-size" type="range" min={TEXT_SIZE.min} max={TEXT_SIZE.max} step={TEXT_SIZE.step} value={settings.text_size} onChange={event => setSettings({ ...settings, text_size: Number(event.target.value) })} />
      </div>,
    },

    text_spacing: {
      section: 'appearance', label: tr('Word spacing'), kw: 'word spacing reading accessibility',
      node: <div className="form-row"><label htmlFor="reading-spacing">{tr('Word spacing')} · {`${settings.text_spacing}px`}</label><input id="reading-spacing" type="range" min="0" max="12" step="1" value={settings.text_spacing} onChange={event => setSettings({ ...settings, text_spacing: Number(event.target.value) })} /></div>,
    },
    always_romanize: {
      section: 'reading',
      label: tr('Always show romanization'),
      kw: 'romanization always show latin arabic pinyin pronunciation',
      node: (
        <div className="form-row check-row">
          <label className="check-label">
            <input
              type="checkbox"
              checked={settings.always_romanize}
              onChange={(e) => setSettings({ ...settings, always_romanize: e.target.checked })}
            />
            <span>{tr("Show romanization")}</span>
          </label>
        </div>
      ),
    },
    xp_effects: {
      section: 'reading', label: tr('XP effects'), kw: 'xp rewards cards bars animation sound off',
      node: <div className="form-row check-row"><label className="check-label">
        <input type="checkbox" checked={settings.xp_effects !== false}
          onChange={event => setSettings({ ...settings, xp_effects: event.target.checked })} />
        <span>{tr('XP effects')}</span>
      </label></div>,
    },
    fast_mode: {
      section: 'reading',
      label: tr('Fast mode'),
      kw: 'xp reward cards fast mode animation dismiss progress',
      node: <div className="form-row check-row"><label className="check-label">
        <input type="checkbox" checked={settings.fast_mode}
          onChange={event => setSettings({ ...settings, fast_mode: event.target.checked })} />
        <span>{tr("Fast mode · dismiss XP badges automatically")}</span>
      </label></div>,
    },
    always_pronunciation: {
      section: 'reading',
      label: tr('Always show pronunciation'),
      kw: 'pronunciation phonetic reading coach reply',
      node: <div className="form-row check-row"><label className="check-label">
        <input type="checkbox" checked={settings.always_pronunciation}
          onChange={event => setSettings({ ...settings, always_pronunciation: event.target.checked })} />
        <span>{tr("Show pronunciation")}</span>
      </label></div>,
    },
    auto_translate: {
      section: 'reading',
      label: tr('Translation'),
      kw: 'translation always show native meaning under reply',
      node: (
        <div className="form-row check-row">
          <label className="check-label">
            <input
              type="checkbox"
              checked={settings.auto_translate}
              onChange={(e) => setSettings({ ...settings, auto_translate: e.target.checked })}
            />
            <span>{tr("Show word and message translations")}</span>
          </label>
        </div>
      ),
    },
    app_updates: {
      section: 'updates',
      label: tr('Application updates'),
      kw: 'update updates upgrade version release install newer check',
      node: <div className="update-controls"><button type="button" className="btn" onClick={() => window.dispatchEvent(new Event('skellyspeak-check-update'))}>{tr("Check for updates")}</button><button type="button" className="btn" onClick={() => { void openDownloads().catch(error => reportFault('Opening downloads', error)) }}>{tr("Downloads")}</button><InfoTip>{tr("Desktop updates install in the app. Android updates open the APK download page. Development builds do not install updates.")}</InfoTip></div>,
    },
    data_copy: {
      section: 'data',
      label: tr('Save a copy of my data'),
      kw: 'data export backup save copy download workspace conversations',
      node: <SaveDataCopy />,
    },
    inference_cache: {
      section: 'data',
      label: tr('AI result cache'),
      kw: 'cache speech audio storage capacity results',
      node: <InferenceCacheSettings />,
    },
    data_reset: {
      section: 'data',
      label: tr('Delete my data'),
      kw: 'data delete erase factory reset wipe start over',
      node: <FactoryReset />,
    },
  }
  for (const sr of SHORTCUT_ROWS) {
    rows[`shortcut_${sr.action}`] = {
      section: 'shortcuts',
      label: t(ui, sr.label),
      kw: `keyboard shortcut hotkey key combo ${sr.label}`,
      node: (
        <div className="form-row">
        <ShortcutField
          label={sr.label}
          ui={ui}
          action={sr.action}
            value={settings.shortcuts[sr.action]}
            onChange={(v) => setShortcuts({ [sr.action]: v })}
          />
        </div>
      ),
    }
  }

  const supported = new Set(['conversation_help', 'models', 'appearance', 'app_updates', 'tts_rate', 'microphone', 'xp_effects', 'fast_mode', 'audio_volume', 'auto_send', 'auto_speak', 'provider_mode', 'target_language', 'target_variety', 'native_variety', 'interface_locale', 'native_language', 'text_size', 'text_spacing', 'always_romanize', 'always_pronunciation', 'auto_translate', 'data_copy', 'data_reset'])
  for (const [id, row] of Object.entries(rows)) {
    if (!supported.has(id)) row.node = <fieldset disabled><p className="field-note">{tr("Not connected.")}</p>{row.node}</fieldset>
    else if (id !== 'provider_mode' && id !== 'models' && accessBusy) row.node = <fieldset disabled>{row.node}</fieldset>
  }

  const q = search.trim().toLowerCase()
  const searching = q.length > 0
  const allRows = Object.entries(rows).filter(([, r]) => !r.hidden).sort(([, a], [, b]) => SECTIONS.findIndex(group => group.id === a.section) - SECTIONS.findIndex(group => group.id === b.section))
  // Mobile groups settings in collapsible sections; search exposes matching rows.
  const stacked = isMobile && !searching
  const visibleRows = searching
    ? allRows.filter(
        ([, r]) => r.label.toLowerCase().includes(q) || r.kw.includes(q)
      )
    : stacked
      ? allRows
      : allRows.filter(([, r]) => r.section === section)

  const activeSection = SECTIONS.find((s) => s.id === section) ?? SECTIONS[0]
  // A heading is printed wherever the section changes going down the list —
  // while searching to show where a hit came from, while stacked to divide the
  // one long scroll into the same groups the desktop nav has. Resolved here,
  // not during the map, so nothing mutates while rendering.
  const showGroupHeadings = searching || stacked
  let runningSection: SectionId | null = null
  const renderRows = visibleRows.map(([id, row]) => {
    const heading = showGroupHeadings && row.section !== runningSection ? row.section : null
    runningSection = row.section
    return { id, row, heading }
  })

  return (
    <SettingsDialog onClose={onClose} title={tr("Settings")}>
      <div
        className="settings-modal"
        onFocusCapture={(e) => {
          const t = e.target as HTMLElement
          if (t.tagName === 'INPUT' || t.tagName === 'SELECT') {
            t.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' })
          }
        }}
      >
        <aside className="settings-nav">
          <input
            className="settings-search"
            placeholder={tr("Search settings…")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            aria-label={tr("Search settings")}
            disabled={accessBusy || dirty || saveState === 'saving'}
          />
          {!isMobile && (
            <nav className="settings-tree">
              {SECTIONS.map((s) => (
                <button
                  key={s.id}
                  disabled={accessBusy || dirty || saveState === 'saving'}
                  type="button"
                  className={`nav-item ${!searching && section === s.id ? 'active' : ''}`}
                  onClick={() => {
                    setSearch('')
                    setSection(s.id)
                  }}
                >
                  <span className="nav-icon"><ToolbarIcon name={s.icon} size={18} /></span>
                  {t(ui, s.labelKey)}
                </button>
              ))}
            </nav>
          )}
        </aside>
        <main className="settings-content">
          <div className="settings-head">
            <h2>{searching ? `“${search.trim()}”` : t(ui, "Settings")}</h2>
            {!searching && !stacked && <p className="settings-section-title">{t(ui, activeSection.labelKey)}</p>}
            <p className="sub">
              {searching
                ? t(ui, 'Search matches', { count: visibleRows.length })
                : stacked
                  ? tr('Expand a section to adjust its settings.')
                  : t(ui, activeSection.descKey)}
            </p>
          </div>
          <div className="settings-scroll" inert={saveState === 'saving'}>
            {searching && visibleRows.length === 0 && (
              <p className="center-note">{t(ui, "Nothing matches “{q}”.", { q: search.trim() })}</p>
            )}
            {stacked ? SECTIONS.map(group => <details className="settings-section" key={group.id}>
              <summary>{t(ui, group.labelKey)}</summary>
              {allRows.filter(([, row]) => row.section === group.id).map(([id, row]) => <div key={id} className="settings-entry">{row.node}</div>)}
            </details>) : renderRows.map(({ id, row, heading }) => (
              <div key={id} className="settings-entry">
                {heading && (
                  <p className="settings-group-k">{t(ui, SECTION_LABEL_KEY[heading])}</p>
                )}
                {row.node}
              </div>
            ))}
            {!searching && visibleRows.length === 0 && (
              <p className="center-note">{tr("Nothing here yet.")}</p>
            )}
          </div>
          <div className="modal-actions">
            <button type="button" className="settings-version" onClick={() => window.dispatchEvent(new Event('skellyspeak-check-update'))} title={tr("Check for updates")}>v{appVersion ?? '…'}</button>
            <SaveStatus state={dirty && saveState === 'idle' ? 'pending' : saveState} />
            {saveState === 'error' && <button className="btn" onClick={() => setSaveState('idle')}>{tr("Retry save")}</button>}
            <button type="button" className="btn settings-close" disabled={accessBusy || dirty || saveState === 'saving'} onClick={onClose}>
              {tr("Close")}</button>
          </div>
        </main>
      </div>
    </SettingsDialog>
  )
}
