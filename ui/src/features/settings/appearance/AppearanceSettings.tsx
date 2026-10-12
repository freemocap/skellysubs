import { DEFAULT_APPEARANCE, type MotionPreference, type SurfacePalette } from '../../../generated/contracts'
import type { Settings } from '../../../types'
import { useI18n } from '../../../components/localization/i18n'
import { SegmentedChoice } from '../../../components/controls/SegmentedChoice'

type Theme = NonNullable<Settings['theme']>

/** Learner-owned theme, palette and motion preferences. */
export function AppearanceSettings({ settings, onChange }: { settings: Settings; onChange: (settings: Settings) => void }) {
  const tr = useI18n()
  const value = settings.appearance ?? DEFAULT_APPEARANCE
  return <div className="appearance-settings">
    <div className="appearance-options">
      <div className="appearance-choice"><span>{tr('Theme')}</span>
        <SegmentedChoice<Theme> label={tr('Theme')} value={settings.theme ?? 'light'} onChange={theme => onChange({ ...settings, theme })}
          options={[['light', tr('Light')], ['dark', tr('Dark')], ['system', tr('System')]]} /></div>
      <div className="appearance-choice"><span>{tr('Surface palette')}</span>
        <SegmentedChoice<SurfacePalette> label={tr('Surface palette')} value={value.palette} onChange={palette => onChange({ ...settings, appearance: { ...value, palette } })}
          options={[['cool', tr('Cool neutral')], ['warm', tr('Warm')]]} /></div>
      <div className="appearance-choice appearance-motion"><span>{tr('Animation')}</span>
        <SegmentedChoice<MotionPreference> label={tr('Animation')} value={value.motion} onChange={motion => onChange({ ...settings, appearance: { ...value, motion } })}
          options={[['system', tr('Follow system')], ['full', tr('Full animation')], ['reduced', tr('Reduced motion')]]} /></div>
    </div>
    <div className="appearance-sample"><button type="button" className="btn">{tr('Sample button')}</button><p className="target-text">{tr('Reading preview')}</p><small>{tr('Appearance changes save automatically.')}</small></div>
    <button type="button" className="btn" onClick={() => onChange({ ...settings, theme: 'light', appearance: { ...DEFAULT_APPEARANCE } })}>{tr('Reset appearance')}</button>
  </div>
}
