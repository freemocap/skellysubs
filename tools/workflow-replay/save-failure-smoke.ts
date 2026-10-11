/** Exercise failure preservation through the real recorder and isolated desktop. */
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { recordingSession } from './record.ts'

const session = await recordingSession()
try {
  const wait = async (expression: string) => {
    const deadline = Date.now() + 20000
    while (!await session.connection.evaluate<boolean>(expression)) {
      if (Date.now() > deadline) throw Error('Failure preservation smoke timed out.')
      await new Promise(resolve => setTimeout(resolve, 50))
    }
  }
  await wait(`!!document.querySelector('.language-choice-start')`)
  await session.connection.evaluate(`document.querySelector('#workflow-recorder button').click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: recording'`)
  await session.connection.evaluate(`document.querySelector('.language-choice-start').click()`)
  await wait(`Array.from(document.querySelectorAll('#root button')).some(e=>e.textContent==='Continue'&&!e.disabled)`)
  await session.connection.evaluate(`window.dispatchEvent(new ErrorEvent('error',{message:'Injected checkpoint failure'}));document.querySelectorAll('#workflow-recorder button')[1].click()`)
  await assert.rejects(session.waitForCompletion(), /Unhandled browser error/)
  await session.flush()
  const manifest = JSON.parse(await readFile(join(session.directory, 'partial-recording/session.json'), 'utf8'))
  const timing = JSON.parse(await readFile(join(session.directory, 'partial-latency.json'), 'utf8'))
  const failure = JSON.parse(await readFile(join(session.directory, 'incomplete.json'), 'utf8'))
  assert.equal(manifest.status, 'incomplete')
  assert.equal(failure.stage, 'final_checkpoint')
  assert.ok(manifest.providerTape)
  assert.ok(manifest.events.some((event: {kind:string}) => event.kind === 'action'))
  assert.ok(timing.samples.length > 0)
  console.log(JSON.stringify({ directory: session.directory, preserved: true, stage: failure.stage, samples: timing.samples.length }))
} finally { await session.close() }
