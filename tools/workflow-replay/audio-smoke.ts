/** Synthetic silent audio verifies browser byte/playback observation without mic access. */
import {writeFile} from 'node:fs/promises'
import {join} from 'node:path'
import {launchDesktop} from './desktop-launch.ts'
import {installBrowserObservation} from './browser-observation.ts'
import {readyAfterReload} from './browser-ready.ts'

const session=await launchDesktop()
try{
  await session.connection.command('Page.enable')
  await readyAfterReload(session.connection)
  await session.connection.evaluate(`(${installBrowserObservation.toString()})();window.__workflowObserve(true)`)
  const result=await session.connection.command<{result:{value:unknown};exceptionDetails?:unknown}>('Runtime.evaluate',{awaitPromise:true,returnByValue:true,userGesture:true,expression:`(async()=>{const context=new AudioContext();await context.resume();const source=context.createBufferSource();source.buffer=context.createBuffer(1,800,8000);source.connect(context.destination);const ended=new Promise(resolve=>source.addEventListener('ended',resolve,{once:true}));source.start();await ended;await context.close();return window.__workflowCheckpoint()})()`})
  if(result.exceptionDetails){await writeFile(join(session.directory,'synthetic-audio-error.json'),JSON.stringify(result.exceptionDetails,null,2));throw Error(`Synthetic audio observation failed: ${session.directory}`)}
  const value=result.result.value as {media:{kind:string}[];hashes:unknown[]}
  if(!value.media.some(event=>event.kind==='pcm-start')||!value.media.some(event=>event.kind==='pcm-ended')||value.hashes.length!==1)throw Error('PCM bytes or playback transition missing.')
  await writeFile(join(session.directory,'audio-observation.json'),JSON.stringify({synthetic:true,physicalSpeakerVerified:false,...value},null,2))
  console.log(JSON.stringify({directory:session.directory,pcmBytesAndPlaybackEventsVerified:true}))
}finally{await session.close()}
