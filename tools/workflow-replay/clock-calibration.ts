import type { DesktopConnection } from './cdp.ts'

/** Native-to-UI offset with explicit IPC round-trip uncertainty. */
export async function calibrate(connection: DesktopConnection) {
  const samples: { offsetMicros: number; uncertaintyMicros: number }[] = []
  for (let index = 0; index < 7; index++) samples.push(await connection.evaluate(`(async()=>{const before=performance.now()*1000;const {micros}=await window.__TAURI_INTERNALS__.invoke('workflow_control',{action:'clock'});const after=performance.now()*1000;return {offsetMicros:(before+after)/2-micros,uncertaintyMicros:(after-before)/2}})()`))
  const best = samples.reduce((a, b) => a.uncertaintyMicros < b.uncertaintyMicros ? a : b)
  return { ...best, samples }
}
