import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { mockIPC } from '@tauri-apps/api/mocks'
import { TopBar } from '../src/app/shell/TopBar'
import { StreakProgressContext } from '../src/state/learning/StreakProgressContext'
import { EffortProgressContext } from '../src/state/learning/EffortProgressContext'
import { useSettingsStore } from '../src/state/settings/settings'
import { useSkillEvidenceStore } from '../src/state/learning/skill-evidence'
import { skillDemo } from '../src/domain/learning/catalog/skillDemo'
import type { Settings } from '../src/types'
import type { StreakSnapshot } from '../src/generated/contracts'
import '../src/styles/index.css'

// Development-only fixture: production components, explicit example snapshots.
mockIPC(command => {
  if (command === 'get_language_totals') return [{ target: 'spanish', name: 'Spanish', nativeName: 'Español', languageTag: 'es', xp: 1234, conversations: 2, partnerUnderstood: 0, revisionsSent: 0, practiceAttempts: 0, explorations: 0, bot: 0 }]
  throw new Error(`Unexpected preview command: ${command}`)
})
useSettingsStore.setState({ settings: { target_language: 'spanish', my_languages: ['spanish'] } as Settings })
useSkillEvidenceStore.setState({ snapshot: { ...skillDemo, target: 'spanish', profile: { ...skillDemo.profile, xp: 1234 } }, scope: 0 })

function Preview() {
  const [value, setValue] = useState<StreakSnapshot>({ sessionId: 'preview', today: 100, app: { count: 1, day: null, celebration: null } })
  const [effects, setEffects] = useState(true)
  const [width, setWidth] = useState('100%')
  const increment = () => setValue(previous => {
    const today = previous.today + 1
    return { ...previous, today, app: { count: previous.app.count + 1, day: today, celebration: `app:${today}` } }
  })
  return <>
    <div className="app" style={{ width, maxWidth: '100%', height: 180 }}>
      <EffortProgressContext value={{ target: 'spanish', value: null, error: null, arrived: [], effects }}>
        <StreakProgressContext value={{ value, error: null, effects }}>
          <TopBar languagePicker={<button className="learning-picker"><span className="learning-picker-endonym">Español</span></button>} />
        </StreakProgressContext>
      </EffortProgressContext>
      <div className="composer"><textarea className="field composer-input" aria-label="Message" /></div>
    </div>
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, padding: 16 }}>
      <button onClick={increment}>Increment streak</button>
      <button onClick={() => setEffects(value => !value)}>Effects {effects ? 'on' : 'off'}</button>
      <button onClick={() => setWidth('320px')}>320px</button>
      <button onClick={() => setWidth('100%')}>Full width</button>
      <button onClick={() => setValue(previous => ({ ...previous, app: { count: 1, day: null, celebration: null } }))}>Reset</button>
    </div>
  </>
}
createRoot(document.getElementById('root')!).render(<Preview />)
