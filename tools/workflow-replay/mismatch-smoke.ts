/** Mutates a copied synthetic capture; the source recording is never changed. */
import {cp,readFile,writeFile} from 'node:fs/promises'
import {randomUUID} from 'node:crypto'
import {join,resolve} from 'node:path'
import {replayRecording} from './replay.ts'
import {decodeManifest} from './bundle-decoder.ts'

if(!process.argv[2])throw Error('Supply the synthetic coach recording directory.')
const source=resolve(process.argv[2])
const copy=resolve('.local','workflow',`mismatch-${randomUUID()}`)
await cp(source,copy,{recursive:true,errorOnExist:true,force:false})
const manifest=decodeManifest(await readFile(join(copy,'session.json')))
const input=manifest.events.find(event=>event.kind==='action'&&event.action==='input')
if(!input||input.kind!=='action')throw Error('Synthetic input action is missing.')
const value=JSON.parse(input.value) as {detail:{value:string}}
value.detail.value='Deliberately changed fixture input.'
input.value=JSON.stringify(value)
await writeFile(join(copy,'session.json'),JSON.stringify(manifest))
let rejected=false
try{await replayRecording(copy)}catch(error){
  if(!(error instanceof Error)||!error.message.includes('Recorded application state was not reached'))throw error
  rejected=true
}
if(!rejected)throw Error('Changed workflow incorrectly passed replay.')
console.log(JSON.stringify({mismatchRejected:true,copy}))
