import type { DesktopConnection } from './cdp.ts'

export async function readyAfterReload(connection: DesktopConnection) {
  const deadline=Date.now()+15000
  while (!await connection.evaluate<boolean>(`!window.__workflowReloadMarker && !!document.getElementById('root')?.children.length && typeof window.__TAURI_INTERNALS__?.invoke==='function'`).catch(()=>false)) {
    if(Date.now()>deadline) throw new Error('Application did not reload into its starting state.')
    await new Promise(yes=>setTimeout(yes,50))
  }
  await connection.command('Page.bringToFront')
  await connection.evaluate(`(async()=>{await document.fonts.ready;await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);if(document.visibilityState!=='visible')throw Error('Replay page must be visible')})()`)
}
