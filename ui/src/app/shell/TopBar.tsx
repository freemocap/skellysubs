import { ProgressCounters } from '../../components/learning/ProgressCounters'
import { LanguageTable, languageCode } from '../../components/learning/LanguageTable'
import { ProgressCard } from '../../components/learning/ProgressCard'
import { useOverlayLayer } from '../../components/dialogs/useOverlayLayer'
import { useHoverCard } from '../../components/learning/useHoverCard'
import { useLanguageTotals } from '../../state/learning/useLanguageTotals'
import { useVisibleEffort } from '../../state/learning/EffortProgressContext'
import { useVisibleStreaks } from '../../state/learning/StreakProgressContext'
import { StreakCount } from '../../components/learning/StreakCount'
import { playRewardSound } from '../../platform/audio/reward-sounds'
import { useEffect, useId, useRef, type ReactNode, type RefObject } from 'react'
import { LearningPicker } from '../../features/settings/language/LanguagePickers'
import { useI18n } from '../../components/localization/i18n'
import { ToolbarIcon } from '../../components/controls/ToolbarIcon'
import { useNavigationStore } from '../../state/navigation/navigation'
import { openLanguageProgress } from '../../state/navigation/language-progress'
import { useSettingsStore } from '../../state/settings/settings'
import { useSkillEvidence } from '../../state/learning/useSkillEvidence'
import { Destinations } from './Destinations'
import { SkillRadarGlyph } from '../../components/learning/SkillRadar'
import { holdingBack, languageSkillLevels } from '../../domain/learning/statistics/skill-levels'
import { skillColors } from '../../domain/learning/catalog/skill-domains'
import { ThemeControls } from './ThemeControls'
import { cutOff, useFitStage } from '../../components/layout/useFitStage'

/// The bar's layouts, fullest first. Compact mode preserves the skill level,
/// language/globe markers and the app streak before the language name is cut off.
const LAYOUTS = ['full', 'compact-progress'] as const

/// A layout fits when the target language's own name shows whole and nothing runs past the bar.
function languageCutOff(bar: HTMLElement) {
  const name = bar.querySelector('.learning-picker-endonym')
  const box = name?.parentElement
  return cutOff(bar) || (!!name && !!box && name.getBoundingClientRect().width > box.clientWidth + 1)
}

/** The global bar: the wordmark, the language, then the Practice and Progress
 * destinations, the progress counters, Settings and More. The skill level
 * opens the Progress page's Skills tab. Hovering the XP (or a first tap) shows
 * a card with every language's XP and effort; pressing it opens the XP tab,
 * and pressing a language's row opens the XP tab for that language. The conversation is home: the
 * wordmark returns to it. Controls that belong to a place live in that place
 * (Conversations in the chat header; the AI status in the chat composer, with
 * AI activity under More everywhere). The theme and palette sit here when the
 * bar has room, and always in Settings. The bar is one row at every width: when
 * the language's own name would be cut off, the progress pill folds to the
 * skill level and streaks, with XP totals still available in the card.
 * The injected picker supports the local layout fixture; production selection
 * uses the shared settings writer. */
export function TopBar({ languagePicker = <LearningPicker /> }: { languagePicker?: ReactNode }) {
  const tr = useI18n()
  const evidence = useSkillEvidence()
  const effort = useVisibleEffort()
  const streaks = useVisibleStreaks()
  const streakDescription = useId()
  const counter = useRef<HTMLButtonElement>(null)
  const progressAnchor = useRef<HTMLDivElement>(null)
  const bar = useRef<HTMLDivElement>(null)
  useFitStage(bar, LAYOUTS, languageCutOff)
  useEffect(() => {
    // A folded pill has no XP number to sound from; the pill itself still shows.
    const anchor = counter.current?.getClientRects().length ? counter.current : progressAnchor.current
    if (anchor && effort.value?.recent.some(award => effort.arrived.includes(award.id) && award.dimension !== 'partner_understood')) playRewardSound({ kind: 'pop' }, anchor)
  }, [effort.arrived, effort.value])
  // A summary of the language profile: which language, and its XP. There is
  // nothing to show until evidence for the active language has landed.
  const profile = evidence.snapshot ? { target: evidence.snapshot.target, xp: evidence.snapshot.profile.xp } : null
  // The language's skill level: its weakest skill, drawn as the resting radar.
  const levels = evidence.snapshot ? languageSkillLevels(evidence.snapshot) : null
  const savingLanguage = useSettingsStore((state) => state.savingLanguage)
  const included = useSettingsStore((state) => state.settings?.my_languages)
  const totals = useLanguageTotals(evidence.snapshot, effort.value, included)
  const activeRow = totals.rows?.find(row => row.target === profile?.target)
  const overlay = useNavigationStore((state) => state.overlay)
  const goHome = useNavigationStore((state) => state.goHome)
  const showOverlay = useNavigationStore((state) => state.showOverlay)
  const openProgress = useNavigationStore((state) => state.openProgress)
  const progressCard = useHoverCard(() => openProgress('xp'))
  return (
    <div ref={bar} className="topbar">
      <button type="button" className="wordmark app-home" aria-label={tr("SkellySpeak home — Chat")} onClick={goHome}>
        <img src="/skellyspeak-logo.png" alt="" width="28" height="28" />
        <span>SkellySpeak</span>
      </button>
      <div className="topbar-language">{languagePicker}</div>
      <div className="topbar-actions">
      <Destinations />
      <ThemeControls />
      <div ref={progressAnchor} className="progress-anchor">
        {levels && <button type="button" className="progress-trigger skill-level-chip" aria-label={tr('Skill level {value0}', { value0: levels.level })} title={tr('Skill level {value0}', { value0: levels.level })} onClick={() => openProgress('skills')}>
          <SkillRadarGlyph levels={levels} /><strong>{tr('Lv {value0}', { value0: levels.level })}</strong>
        </button>}
        {/* Hover (mouse) or a first tap shows the card; pressing while it shows opens the XP tab. */}
        <button ref={counter} type="button" className="profile-trigger progress-trigger" aria-label={tr("Language XP")} aria-describedby={streakDescription} aria-haspopup="dialog" aria-expanded={progressCard.open} onClick={progressCard.press} {...progressCard.anchor}>
          <ProgressCounters xp={profile?.xp ?? null} xpLabel="Language XP" code={activeRow ? languageCode(activeRow) : undefined} global={totals.globalXp} effort={effort.value} effects={effort.effects} streaks={streaks.value} error={streaks.error ?? effort.error ?? totals.error} />
        </button>
        <span id={streakDescription} hidden>{tr('App streak: {value0}', { value0: streaks.value?.app.count ?? 1 })}</span>
        {progressCard.open && <CardLayer anchor={progressAnchor} onClose={progressCard.close}>
          <div {...progressCard.anchor}><ProgressCard title={tr("All languages")} icon="globe" xp={totals.globalXp} effort={totals.globalEffort} units={[]} headerAccessory={<StreakCount value={streaks.value?.app} effects={false} />} error={totals.error} expandLabel="Open the Progress page" onExpand={() => { progressCard.close(); openProgress('xp') }}>
            {levels && <div className="progress-card-levels">
              <SkillRadarGlyph levels={levels} />
              <div>
                {activeRow && <strong>{activeRow.name}</strong>}
                <strong>{tr('Skill level {value0}', { value0: levels.level })}</strong>
                <span>{levels.level === 0 && levels.ready === 0 ? tr('Get a skill point in each skill to reach level 1.') : tr('{value0} of {value1} skills at {value2} points for level {value3}', { value0: levels.ready, value1: levels.skills.length, value2: levels.target, value3: levels.level + 1 })}</span>
                <ul>{holdingBack(levels).slice(0, 3).map(({ skill, needed }) => <li key={skill.id} style={{ color: skillColors(skill.id).ink }}>{tr(skill.label)}<span>{tr('{value0} more', { value0: needed })}</span></li>)}</ul>
              </div>
            </div>}
            {totals.rows ? <LanguageTable rows={totals.rows} active={profile?.target} compact onSelect={target => { progressCard.close(); void openLanguageProgress(target, 'xp') }} /> : <p role="status" className="progress-card-empty">{tr('Loading…')}</p>}
          </ProgressCard></div>
        </CardLayer>}
      </div>

      {/* App-wide settings. The conversation's own settings open from the chat
          header, so this one carries its name to keep the two apart. On a
          phone it moves into More, where there is room for its label. */}
      <button
        type="button"
        className="gear app-settings"
        onClick={() => showOverlay('settings')}
        disabled={savingLanguage}
        aria-label={tr("Settings")}
        title={tr("Settings")}
      >
        <ToolbarIcon name="cog" /><span>{tr("Settings")}</span>
      </button>
      <button type="button" className="gear" aria-label={tr("More")} aria-expanded={overlay === 'more'} onClick={() => showOverlay('more')}><ToolbarIcon name="more" size={18} /></button>

      </div>
    </div>
  )
}

/** Closes on Escape or an outside press; the anchor holds the button, so pressing it again reaches its handler. */
function CardLayer({ anchor, onClose, children }: { anchor: RefObject<HTMLDivElement | null>; onClose: () => void; children: ReactNode }) {
  useOverlayLayer(anchor, onClose, true)
  return children
}
