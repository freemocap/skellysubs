/** Same local UI interaction with recording off/on, measured inside the webview. */
import {writeFile} from 'node:fs/promises'
import {join} from 'node:path'
import {recordingSession} from './record.ts'
import {desktopActions} from './desktop-actions.ts'

const session=await recordingSession()
try{
  const {wait}=desktopActions(session.connection)
  await wait(`!!document.querySelector('.language-choice-start')`)
  const measure=()=>session.connection.evaluate<number[]>(`(async()=>{const samples=[];const buttons=document.querySelectorAll('.language-choice-start');for(let i=0;i<40;i++){await new Promise(requestAnimationFrame);const before=performance.now();buttons[i%2].click();samples.push((performance.now()-before)*1000);await new Promise(requestAnimationFrame)}return samples})()`)
  const off=await measure()
  await session.connection.evaluate(`document.querySelector('#workflow-recorder button').click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: recording'`)
  const on=await measure()
  await session.connection.evaluate(`document.querySelectorAll('#workflow-recorder button')[1].click()`)
  await wait(`document.querySelector('#workflow-recorder span')?.textContent==='Workflow recording: saved locally'`)
  await session.flush()
  const summary=(values:number[])=>{const sorted=[...values].sort((a,b)=>a-b);return{count:values.length,p50Micros:sorted[19],p95Micros:sorted[37],maxMicros:sorted.at(-1)}}
  const result={boundary:'synchronous local click dispatch, excluding frame wait and controller transport',off:summary(off),on:summary(on),offSamples:off,onSamples:on}
  await writeFile(join(session.directory,'overhead.json'),JSON.stringify(result,null,2))
  console.log(JSON.stringify({directory:session.directory,...result,offSamples:undefined,onSamples:undefined}))
}finally{await session.close()}
