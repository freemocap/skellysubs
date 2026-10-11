/** Runs an actual isolated desktop capture/replay; never calls an AI provider. */
import { join } from 'node:path'
import { recordingSession } from './record.ts'
import { replayRecording } from './replay.ts'

const session = await recordingSession()
try {
  const wait = async (expression: string) => {
    const deadline = Date.now() + 20000
    while (!await session.connection.evaluate<boolean>(expression).catch(() => false)) {
      if (Date.now() > deadline) throw new Error('Desktop smoke expectation timed out.')
      await new Promise(yes => setTimeout(yes, 50))
    }
  }
  await wait(`!!document.querySelector('.language-choice-start')`)
  await session.connection.evaluate(`document.querySelector('#workflow-recorder button').click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: recording'`)
  await session.connection.evaluate(`document.querySelector('.language-choice-start').click()`)
  await wait(`Array.from(document.querySelectorAll('#root button')).some(e=>e.textContent==='Continue'&&!e.disabled)`)
  const before = await session.connection.evaluate<string>(`document.getElementById('root').innerText`)
  await session.connection.evaluate(`Array.from(document.querySelectorAll('#root button')).find(e=>e.textContent==='Continue'&&!e.disabled).click()`)
  await wait(`document.getElementById('root').innerText!==${JSON.stringify(before)}`)
  await session.connection.evaluate(`window.dispatchEvent(new ErrorEvent('error',{message:'ResizeObserver loop completed with undelivered notifications.'}));document.querySelectorAll('#workflow-recorder button')[1].click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: saved locally'`)
  await session.flush()
  if (session.failures.length) throw new Error('Capture reported a failure.')
} finally { await session.close() }
const replay = await replayRecording(join(session.directory, 'recording'))
console.log(JSON.stringify({ capture: session.directory, replay, completed: true }))
