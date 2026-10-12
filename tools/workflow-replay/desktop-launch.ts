/** Explicit local development launch; never selects an existing desktop target. */
import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises'
import { createServer } from 'node:http'
import { createServer as createTcpServer } from 'node:net'
import { resolve, join, extname, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { DesktopConnection } from './cdp.ts'
import { restoreWorkspace } from './workspace-bundle.ts'
import type { Bundle } from './session.ts'
import { buildIdentity } from './build-identity.ts'
import { presentationStorage } from './browser-state.ts'
import { browserArguments } from './options.ts'

const root = fileURLToPath(new URL('../../', import.meta.url))
async function port(): Promise<number> {
  const server = createTcpServer()
  await new Promise<void>((yes, no) => { server.once('error', no); server.listen(0, '127.0.0.1', yes) })
  const address = server.address()
  if (!address || typeof address === 'string') throw new Error('No local port available.')
  await new Promise<void>((yes, no) => server.close(error => error ? no(error) : yes()))
  return address.port
}

export async function launchDesktop(observe: (method: string, params: unknown) => void = () => {}, replay?: { bundle: Bundle; assets: ReadonlyMap<string, Uint8Array> }, options:{mute?:boolean}={}) {
  if (process.platform !== 'win32') throw new Error('This desktop harness currently requires Windows WebView2.')
  const isolation = randomUUID()
  const directory = join(root, '.local', 'workflow', isolation)
  await mkdir(directory, { recursive: true, mode: 0o700 })
  if (replay) {
    const tape = replay.bundle.providerTape && replay.assets.get(replay.bundle.providerTape)
    if (!tape) throw new Error('Native provider tape is required for application replay.')
    await restoreWorkspace(replay.bundle, replay.assets, join(directory, 'restored-workspace'))
    await writeFile(join(directory, 'native-provider.json'), tape, { flag: 'wx', mode: 0o600 })
  }
  const assets = join(root, 'ui', 'dist')
  const build=await buildIdentity()
  const served=new Map<string,Buffer>()
  for(const file of await readdir(assets,{recursive:true,withFileTypes:true}))if(file.isFile()){
    const path=join(file.parentPath,file.name);served.set(path,await readFile(path))
  }
  if(await buildIdentity()!==build)throw Error('Build changed while preparing workflow session.')
  if(replay){
    const initial=replay.bundle.events.find(event=>event.kind==='display'&&event.target==='initial-browser-state')
    if(!initial||initial.kind!=='display')throw Error('Starting browser state is missing.')
    const browser=JSON.parse(initial.text) as {storage:Record<string,string>}
    const storage=presentationStorage(browser.storage)
    if(JSON.stringify(storage)!==JSON.stringify(browser.storage))throw Error('Unsupported starting browser preferences.')
    const bootstrap=`<script>for(const [key,value] of Object.entries(${JSON.stringify(storage).replaceAll('<','\\u003c')}))localStorage.setItem(key,value)</script>`
    const path=join(assets,'index.html')
    served.set(path,Buffer.from(served.get(path)!.toString('utf8').replace('<head>',`<head>${bootstrap}`)))
  }
  const types: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2' }
  const server = createServer((request, response) => {
    void (async () => {
      const pathname = decodeURIComponent(new URL(request.url ?? '/', 'http://127.0.0.1').pathname)
      const file = resolve(assets, `.${pathname === '/' ? '/index.html' : pathname}`)
      if (!file.startsWith(assets + sep)) { response.writeHead(403).end(); return }
      const data = served.get(file)
      if(!data){response.writeHead(404).end();return}
      if(replay) response.setHeader('Content-Security-Policy', "default-src 'self' data: blob:; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: http://ipc.localhost; img-src 'self' data: blob: asset: http://asset.localhost; media-src 'self' data: blob:; frame-src 'none'; form-action 'none'")
      response.writeHead(200, { 'Content-Type': types[extname(file)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' }); response.end(data)
    })().catch(() => response.writeHead(404).end())
  })
  await new Promise<void>((yes, no) => { server.once('error', no); server.listen(0, '127.0.0.1', yes) })
  const address = server.address()
  if (!address || typeof address === 'string') throw new Error('UI server did not bind.')
  const debugPort = await port()
  const executable = join(root, '.local', 'workflow-build', 'skellyspeak.exe')
  const child = spawn(executable, [], { cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: {
    ...process.env,
    SKELLYSPEAK_WORKFLOW_DIR: directory,
    SKELLYSPEAK_WORKFLOW_MODE: replay ? 'replay' : 'capture',
    SKELLYSPEAK_WORKFLOW_ISOLATION: isolation,
    SKELLYSPEAK_WORKFLOW_UI_URL: `http://127.0.0.1:${address.port}`,
    SKELLYSPEAK_LOG_RUN_DIR: join(directory, 'logs'),
    WEBVIEW2_USER_DATA_FOLDER: join(directory, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArguments(debugPort,!!replay,options.mute??false),
  } })
  // Startup diagnostics stay local, bounded, and out of the conversation.
  let diagnostics = ''
  child.stdout.on('data', bytes => { diagnostics = (diagnostics + String(bytes)).slice(-65536) })
  child.stderr.on('data', bytes => { diagnostics = (diagnostics + String(bytes)).slice(-65536) })
  let failure: Error | undefined
  child.once('error', error => { failure = error })
  let connection: DesktopConnection | undefined
  let networkFailure=false
  let closed=false
  const close = async () => {
    if(closed)return
    closed=true
    connection?.close()
    if (child.exitCode === null) child.kill()
    server.closeAllConnections(); server.close()
    await writeFile(join(directory, 'startup.log'), diagnostics, { mode: 0o600 })
  }
  try {
    const deadline = Date.now() + 30000
    while (Date.now() < deadline) {
      if (failure || child.exitCode !== null) throw failure ?? new Error('Isolated application exited during startup.')
      try { connection = await DesktopConnection.connect(`http://127.0.0.1:${debugPort}`, (method,params)=>{
        if(method==='Fetch.requestPaused'){
          const item=params as {requestId:string;request:{url:string}}
          const allowed=new URL(item.request.url).origin===`http://127.0.0.1:${address.port}` || new URL(item.request.url).origin==='http://ipc.localhost'
          if(!allowed)networkFailure=true
          void connection?.command(allowed?'Fetch.continueRequest':'Fetch.failRequest',allowed?{requestId:item.requestId}:{requestId:item.requestId,errorReason:'BlockedByClient'}).catch(()=>{networkFailure=true})
        }else observe(method,params)
      }); break }
      catch { await new Promise(yes => setTimeout(yes, 250)) }
    }
    if (!connection) throw new Error('Isolated desktop attachment timed out.')
    await connection.command('Runtime.enable')
    if(replay)await connection.command('Fetch.enable',{patterns:[{urlPattern:'http*',requestStage:'Request'}]})
    while (!await connection.evaluate<boolean>(`typeof window.__TAURI_INTERNALS__?.invoke === 'function' && !!document.getElementById('root')`)) {
      if (Date.now() >= deadline) throw new Error('Desktop application page did not become ready.')
      await new Promise(yes => setTimeout(yes, 100))
    }
    const status = await connection.evaluate<{ isolated: boolean; enabled: boolean }>(`window.__TAURI_INTERNALS__.invoke('workflow_control',{action:'status'})`)
    if (!status.enabled || !status.isolated) throw new Error('Application did not confirm capture-build isolation.')
    return { connection, directory, build, close,isRunning:()=>child.exitCode===null&&child.signalCode===null, verifyNetwork:()=>{if(networkFailure)throw Error('Replay attempted an unsupported browser network request.')} }
  } catch (error) { await close(); throw error }
}

if (import.meta.main) {
  const session = await launchDesktop()
  try {
    const proof = await session.connection.evaluate(`({native:!!window.__TAURI_INTERNALS__,root:!!document.getElementById('root'),title:document.title})`)
    await writeFile(join(session.directory, 'attachment.json'), JSON.stringify(proof, null, 2))
    if (process.argv.includes('--inspect')) {
      await new Promise(yes => setTimeout(yes, 1000))
      console.log(await session.connection.evaluate(`Array.from(document.querySelectorAll('button,input')).map(e=>({tag:e.tagName,type:e.type,label:e.textContent||e.getAttribute('aria-label')||e.placeholder})).slice(0,35)`))
    }
    console.log(`Desktop attachment verified. Evidence: ${session.directory}`)
  } finally { await session.close() }
}
