/** Release-APK launch/resume regression. No debug bridge, reload, reset or AI calls. */
import { execFileSync } from 'node:child_process'
import { parseArgs } from 'node:util'
import { pathToFileURL } from 'node:url'
import { deviceFromList } from './android.ts'

function xmlAttribute(value: string): string {
  return value.replace(/&(?:quot|apos|lt|gt|amp);/g, entity => ({
    '&quot;': '"', '&apos;': "'", '&lt;': '<', '&gt;': '>', '&amp;': '&',
  })[entity]!)
}

/** Empty nested WebViews and labels belonging to another app are not success. */
export function hasStartupSurface(xml: string, packageId: string, label: string): boolean {
  if (!xml.includes('<hierarchy') || !xml.includes('</hierarchy>')) {
    throw new Error('Android did not return a complete UI hierarchy.')
  }
  const nodes = [...xml.matchAll(/<node\b[^>]*>/g)].map(([node]) =>
    Object.fromEntries([...node.matchAll(/([\w-]+)="([^"]*)"/g)]
      .map(([, name, value]) => [name, xmlAttribute(value)]))
  ).filter(attributes => attributes.package === packageId)
  const labels = nodes.flatMap(attributes => [attributes.text ?? '', attributes['content-desc'] ?? ''])
  if (labels.some(text => text.includes('state not managed for field state'))) {
    throw new Error('App displayed an error: native state was not registered before its first command.')
  }
  if (labels.some(text => ['Retry startup', 'Reload app', 'Reload view', 'Configuration could not be loaded'].includes(text)
    || text.startsWith('Could not open SkellySpeak:') || text.startsWith('This view crashed:'))) {
    throw new Error('App displayed an error. No retry or reload was attempted.')
  }
  return nodes.some(attributes => attributes.enabled === 'true'
    && (attributes.text === label || attributes['content-desc'] === label))
}

async function main() {
  const { values } = parseArgs({ options: {
    package: { type: 'string', default: 'com.freemocap.skellyspeak' },
    activity: { type: 'string', default: 'com.freemocap.skellyspeak.MainActivity' },
    expect: { type: 'string', default: 'Welcome to SkellySpeak' },
    runs: { type: 'string', default: '5' },
    mode: { type: 'string', default: 'cold' },
    'timeout-ms': { type: 'string', default: '15000' },
  } })
  const packageId = values.package!
  const activity = values.activity!
  const runs = Number(values.runs)
  const timeoutMs = Number(values['timeout-ms'])
  if (!['cold', 'resume'].includes(values.mode!)) throw new Error('Mode must be cold or resume.')
  if (!/^[\w]+(?:\.[\w]+)+$/.test(packageId) || !/^[\w.]+$/.test(activity)) {
    throw new Error('Invalid Android package or activity.')
  }
  if (!Number.isInteger(runs) || runs < 1 || runs > 100 || !Number.isInteger(timeoutMs)
    || timeoutMs < 1000 || timeoutMs > 60000 || !values.expect?.trim()) {
    throw new Error('Use 1–100 runs, a 1000–60000 ms timeout, and a nonempty expected label.')
  }
  const serial = deviceFromList(execFileSync('adb', ['devices'], { encoding: 'utf8' }), process.env.ANDROID_SERIAL)
  const adb = (...args: string[]) => execFileSync('adb', ['-s', serial, ...args], {
    encoding: 'utf8', timeout: timeoutMs, maxBuffer: 4 * 1024 * 1024,
  })
  // Keep a charging test phone awake without changing its saved screen timeout.
  // Restore the system setting even when an assertion or observer fails.
  const stayAwakeKey = 'stay_on_while_plugged_in'
  const previousStayAwake = adb('shell', 'settings', 'get', 'global', stayAwakeKey).trim()
  if (!/^\d+$/.test(previousStayAwake)) throw new Error('Cannot preserve the device stay-awake setting.')
  try {
    adb('shell', 'settings', 'put', 'global', stayAwakeKey, '7')
    for (let run = 1; run <= runs; run++) {
      // Wake before the process starts, so the phone's idle timeout cannot lock
      // an unattended run. There is no input or lifecycle recovery after launch.
      adb('shell', 'input', 'keyevent', 'KEYCODE_WAKEUP')
      if (values.mode === 'cold') adb('shell', 'am', 'force-stop', packageId)
      else {
        adb('shell', 'input', 'keyevent', 'KEYCODE_HOME')
        await new Promise(resolve => setTimeout(resolve, 1000))
      }
      const started = performance.now()
      const launch = adb('shell', 'am', 'start', '-W', '-a', 'android.intent.action.MAIN',
        '-c', 'android.intent.category.LAUNCHER', '-n', `${packageId}/${activity}`)
      if (!/^Status: ok$/m.test(launch)) throw new Error(`${values.mode} launch ${run} was not accepted by Android.`)
      let passed = false
      let consecutiveSurfaces = 0
      while (performance.now() - started < timeoutMs) {
        const activities = adb('shell', 'dumpsys', 'activity', 'activities')
        const foreground = activities.match(/(?:topResumedActivity|mResumedActivity)=ActivityRecord\{[^}\n]*? ([\w.]+)\//)?.[1]
        if (!foreground) {
          if (adb('shell', 'dumpsys', 'window').includes('mDreamingLockscreen=true')) {
            throw new Error('Device test interrupted: the phone is locked. This is not an app startup result.')
          }
          await new Promise(resolve => setTimeout(resolve, 250))
          continue
        }
        if (foreground !== packageId) {
          throw new Error('Device test interrupted: the selected app is no longer in the foreground. Leave the phone unlocked and untouched.')
        }
        const dumpPath = `/data/local/tmp/skellyspeak-startup-${process.pid}.xml`
        let xml: string
        try {
          adb('shell', 'uiautomator', 'dump', dumpPath)
          xml = adb('shell', 'cat', dumpPath)
        } finally {
          adb('shell', 'rm', '-f', dumpPath)
        }
        if (performance.now() - started <= timeoutMs && hasStartupSurface(xml, packageId, values.expect!)) {
          if (++consecutiveSurfaces === 2) { passed = true; break }
        } else consecutiveSurfaces = 0
        await new Promise(resolve => setTimeout(resolve, 250))
      }
      const elapsed = Math.round(performance.now() - started)
      console.log(`${passed ? 'PASS' : 'FAIL'} ${values.mode} launch ${run}/${runs}: ${elapsed} ms`)
      if (!passed) throw new Error('Expected app surface did not appear without user interaction. No recovery was attempted.')
    }
  } finally {
    adb('shell', 'settings', 'put', 'global', stayAwakeKey, previousStayAwake)
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  void main().catch(error => { console.error(error.message); process.exitCode = 1 })
}
