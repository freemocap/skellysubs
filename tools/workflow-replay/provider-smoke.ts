/** Actual coach UI -> native graph -> local controlled response -> offline replay. */
import {join} from 'node:path'
import {recordingSession} from './record.ts'
import {replayRecording} from './replay.ts'
import {desktopActions} from './desktop-actions.ts'
import {fixtureServer} from './fixture-server.ts'
import {writeFile} from 'node:fs/promises'

const fixture=await fixtureServer()
const session=await recordingSession()
try{
  const {wait,click}=desktopActions(session.connection)
  await session.connection.evaluate(`(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke;const access=await invoke('get_access_settings');await invoke('save_access_settings',{expectedRevision:access.revision,custom:{baseUrl:${JSON.stringify(fixture.url)},bearerAuth:false},sessionToken:null,removeToken:false});const connection=await invoke('get_connection');await invoke('select_route',{expectedRevision:connection.revision,route:'custom'})})()`)
  await wait(`!!document.querySelector('[aria-label="Learn English"]')`)
  await session.connection.evaluate(`document.querySelector('[aria-label="Learn English"]').parentElement.querySelector('.language-choice-start').click()`)
  await click('.onboarding-languages button[type=submit]')
  await wait(`!!document.querySelector('.onboarding-actions .primary:not(:disabled)')`)
  await session.connection.evaluate(`document.querySelector('.onboarding-actions .primary').click()`)
  await wait(`!!document.querySelector('.coach-input')`)
  await session.connection.evaluate(`document.querySelector('#workflow-recorder button').click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: recording'`)
  await click('.coach-input')
  await session.connection.command('Input.insertText',{text:'Explain this Unicode example: café 𠮷.'})
  await wait(`!!document.querySelector('.coach-send:not(:disabled)')`)
  await click('.coach-send')
  await wait(`document.getElementById('root').innerText.includes(${JSON.stringify('Workflow fixture reply: café 𠮷.')})`)
  await new Promise(yes=>setTimeout(yes,300))
  await session.connection.evaluate(`document.querySelectorAll('#workflow-recorder button')[1].click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: saved locally'`)
  await session.flush()
  if(session.failures.length)throw Error('Provider capture failed.')
}catch(error){await writeFile(join(session.directory,'fixture-screen.txt'),await session.connection.evaluate<string>(`document.getElementById('root').innerText`));throw error}
finally{await session.close();await fixture.close()}
const replay=await replayRecording(join(session.directory,'recording'))
console.log(JSON.stringify({capture:session.directory,replay,completed:true,liveProvider:false}))
