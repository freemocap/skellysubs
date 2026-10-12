/** Exercise real conversation commands and prompt construction in two isolated apps. */
import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { launchDesktop } from './desktop-launch.ts'
import { workflowControl } from './control.ts'
import { SessionCapture } from './session.ts'
import { captureWorkspace } from './workspace-bundle.ts'
import { loadBundle } from './bundle-import.ts'
import type { DesktopConnection } from './cdp.ts'

async function createAndPreview(connection: DesktopConnection) {
  return connection.evaluate<Array<{id:string;promptDigest:string}>>(`(async()=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    const results=[];
    for(let i=0;i<3;i++){
      const before=await invoke('get_snapshot');
      const receipt=await invoke('execute_command',{command:{sessionId:before.sessionId,
        actionId:crypto.randomUUID(),action:{kind:'startChat',languageId:'english'}}});
      const snapshot=await invoke('get_snapshot');
      const conversation=snapshot.conversations.find(c=>c.id===receipt.entityId);
      if(!conversation)throw Error('Created conversation missing.');
      const settings=conversation.settings;
      const preview=await invoke('preview_conversation_prompt',{conversationId:conversation.id,
        configuration:{difficulty:settings.difficulty,varietyId:settings.varietyId,
          direction:{topic:null,timeReference:'any',usePersonaDetails:true}}});
      if(!preview.systemPrompt)throw Error('Prompt preview is empty.');
      const bytes=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(preview.systemPrompt));
      results.push({id:conversation.id,promptDigest:Array.from(new Uint8Array(bytes),n=>n.toString(16).padStart(2,'0')).join('')});
    }
    return results;
  })()`)
}

const capture = await launchDesktop()
const bundle = new SessionCapture({contentConsent:true,build:capture.build,maxBytes:256*1024*1024,maxEvents:10})
const directory=join(capture.directory,'identity-recording')
let expected: Awaited<ReturnType<typeof createAndPreview>>
try {
  const start=await workflowControl<{workspace:string}>(capture.connection,'start')
  await captureWorkspace(start.workspace,bundle)
  bundle.append({kind:'display',target:'initial-browser-state',text:JSON.stringify({storage:{}})},0)
  expected=await createAndPreview(capture.connection)
  assert.equal(new Set(expected.map(item=>item.id)).size,3)
  assert.ok(expected.every(item=>item.promptDigest.length===64))
  const stop=await workflowControl<{providerTape:string}>(capture.connection,'stop')
  bundle.setProviderTape(await readFile(stop.providerTape))
  bundle.stop()
  await bundle.export(directory)
} finally { await capture.close() }
const replay=await launchDesktop(undefined,await loadBundle(directory),{mute:true})
try {
  assert.deepEqual(await createAndPreview(replay.connection),expected)
  await workflowControl(replay.connection,'stop')
  replay.verifyNetwork()
  await writeFile(join(replay.directory,'identity-result.json'),JSON.stringify({
    completed:true,conversations:3,identitiesMatch:true,promptsMatch:true,
    tapeConsumed:true,build:replay.build,capture:capture.directory,
  },null,2))
  console.log(JSON.stringify({completed:true,directory:replay.directory}))
} finally { await replay.close() }
