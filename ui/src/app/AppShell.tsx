import { ActionRewardBursts } from '../components/learning/ActionRewardBursts'
import { LevelUpPresenter } from '../features/skills/levels/LevelUpPresenter'
import { useSkillLevelEventQueue } from '../state/learning/skill-level-events'
import { EffortProgressContext } from '../state/learning/EffortProgressContext'
import { useEffortProgress } from '../state/learning/useEffortProgress'
import { useStreakProgress } from '../state/learning/useStreakProgress'
import { StreakProgressContext } from '../state/learning/StreakProgressContext'
import { onReadingQuestion } from '../platform/ipc/window'
import { OnboardingSetup } from '../features/settings/onboarding/OnboardingSetup'
import { Tour } from './tour/Tour'
import { TOUR_ENABLED, useOnboardingStore } from '../state/settings/onboarding'
import { LanguageBrowser } from '../features/languages/LanguageBrowser'
import { useAppearance } from '../platform/appearance/useAppearance'
import { CredentialCleanup } from '../features/startup/CredentialCleanup'
import { I18nProvider } from '../components/localization/i18n'
import { useEffect } from 'react'
import { useIsMobile } from '../components/layout/useIsMobile'
import { useLoadSkillEvidence, useSkillEvidence } from '../state/learning/useSkillEvidence'
import { useSkillNavigationStore } from '../state/navigation/skill-navigation'
import { useNavigationStore } from '../state/navigation/navigation'
import { useSettingsStore } from '../state/settings/settings'
import { useSessionStore } from '../state/session/session'
import { openOverlay } from '../domain/input/back'
import { reportFault } from '../platform/diagnostics/faults'
import { SHORTCUT_DEFAULTS } from '../domain/input/keyboard'
import { AiViewPanel } from '../features/activity/AiViewPanel'
import { GraphHoverOverlay } from './shell/GraphHoverOverlay'
import { useAiWindowSync } from './useAiWindowSync'
import { ReadingTools } from './ReadingTools'
import { SettingsModal } from '../features/settings/SettingsModal'
import { UpdateBanner } from './shell/UpdateBanner'
import { FaultBar } from './shell/FaultBar'
import { MoreDialog } from './shell/MoreDialog'
import { SurfaceHost } from './shell/SurfaceHost'
import { TopBar } from './shell/TopBar'
import { useConnectionHealthChecks } from './useConnectionHealthChecks'
import { useAppShortcuts } from './shortcuts/useAppShortcuts'

/// The application shell: the effects that belong to the application rather than
/// to a surface, plus the composition itself.
///
/// Every value below is read straight from a store with a selector, so nothing
/// is copied into component state and nothing can go stale behind a store update.
export function AppShell() {
  const page = useNavigationStore((state) => state.page)
  const practiceView = useNavigationStore(state => state.practiceView)
  // The place on screen colours the band under the top bar: Chat, Practice, or
  // neither (the Progress page).
  const place = useNavigationStore((state) => state.page === 'skills' ? 'skills' : state.practiceView === 'drill' ? 'practice' : 'chat')
  const languageInfo = useNavigationStore(state => state.languageInfo)
  const overlay = useNavigationStore((state) => state.overlay)
  const showOverlay = useNavigationStore((state) => state.showOverlay)
  const closeOverlay = useNavigationStore((state) => state.closeOverlay)
  const setSettingsBusy = useNavigationStore((state) => state.setSettingsBusy)

  const settings = useSettingsStore((state) => state.settings)
  // Bumped when a settings **write** lands. The first read is not a write.
  const settingsVersion = useSettingsStore((state) => state.revision)
  // Derived, not copied: editing the shortcut in Settings takes effect at once
  // rather than at the next start.
  const shortcuts = useSettingsStore((state) => state.settings?.shortcuts ?? SHORTCUT_DEFAULTS)

  const onboarding = useOnboardingStore(state => state.preferences)
  useAppearance(settings)

  // A request to show a skill, from anywhere, opens it on the Progress page.
  const mapRequest = useSkillNavigationStore((state) => state.mapRequest)
  useEffect(() => { if (mapRequest) useNavigationStore.getState().openProgress('skills') }, [mapRequest])
  // Android back returns either destination to the retained conversation.
  const isMobile = useIsMobile()
  useEffect(() => isMobile && (page === 'skills' || practiceView === 'drill') ? openOverlay(() => useNavigationStore.getState().openConversation()) : undefined, [isMobile, page, practiceView])
  // A settings write can move the AI access projection with it.
  useEffect(() => {
    if (settingsVersion === 0) return
    void useSessionStore.getState().refresh().catch((error: unknown) => reportFault('Loading AI access', error))
  }, [settingsVersion])

  // Keep evidence read for the active language. The surfaces read the store
  // themselves, so the shell only has to say which language is current.
  useConnectionHealthChecks()
  useAiWindowSync()
  useLoadSkillEvidence()
  const evidence = useSkillEvidence()
  // The one owner of level celebrations for the active language.
  useSkillLevelEventQueue(evidence.snapshot, evidence.reload, settings?.xp_effects !== false)
  const effort = useEffortProgress(settings?.target_language ?? '', evidence.snapshot)
  const streaks = useStreakProgress(settings?.scope?.sessionId ?? '', settings?.xp_effects !== false)
  useAppShortcuts(shortcuts)
  useEffect(() => {
    let closed = false
    let unlisten: (() => void) | undefined
    void onReadingQuestion(question => {
      const navigation = useNavigationStore.getState()
      navigation.draftReadingQuestion(question)
      if (!navigation.settingsBusy) navigation.openConversation('panel')
    }).then(stop => { if (closed) stop(); else unlisten = stop }).catch(error => reportFault('Reading question navigation', error))
    return () => { closed = true; unlisten?.() }
  }, [])


  if (onboarding?.onboardingRequired) return <I18nProvider locale={onboarding.interfaceLocale}><ReadingTools settings={settings} onAsk={null} defaultScope={{ language: onboarding.onboardingLanguage ?? 'english', variety: null, explanation: onboarding.explanationLanguage, explanationVariety: onboarding.explanationVarietyId }}><OnboardingSetup /></ReadingTools></I18nProvider>

  return (
    <I18nProvider locale={settings?.interface_locale ?? 'english'}><ReadingTools settings={settings}><EffortProgressContext value={effort}><div className="app" data-place={place}>
      <ActionRewardBursts enabled={effort.effects} />
      <LevelUpPresenter snapshot={effort.effects ? evidence.snapshot : null} />
      <UpdateBanner />

      <StreakProgressContext value={streaks}><TopBar /></StreakProgressContext>
      <FaultBar />
      <CredentialCleanup />
      <SurfaceHost />
      <MoreDialog />
      {overlay === 'languages' && <LanguageBrowser key={languageInfo} initialLanguage={languageInfo} onClose={closeOverlay} />}
      <AiViewPanel open={overlay === 'activity'} onOpenChange={open => open ? showOverlay('activity') : closeOverlay()} />
      <GraphHoverOverlay />
      {overlay === 'settings' && <SettingsModal onClose={closeOverlay} onBusyChange={setSettingsBusy} />}
      {TOUR_ENABLED && onboarding?.onboardingHelp && <Tour />}
    </div></EffortProgressContext></ReadingTools></I18nProvider>
  )
}
