/** Development-only observation. Keeps playback evidence separate from speaker output. */
export function installBrowserObservation() {
  const host = window as unknown as { __workflowSend: (message: string) => void; __workflowObserve: (active: boolean) => void; __workflowCheckpoint: () => Promise<unknown> }
  let active = false
  let pending = 0
  let sequence = 0
  const media: unknown[] = []
  const failures: string[] = []
  const playing = new Set<number>()
  const hashes=new Map<number,unknown>()
  const record = (value: unknown) => {
    if (media.length >= 100000) throw new Error('Playback observation limit exceeded.')
    media.push(value)
  }
  const hash = async (bytes: ArrayBuffer) => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), n => n.toString(16).padStart(2, '0')).join('')
  const task = (work: () => Promise<void>) => {
    pending++
    void work().catch(() => failures.push('Playback bytes could not be observed.')).finally(() => pending--)
  }
  window.addEventListener('error', event => {
    // Application diagnostics retain these layout delivery notices. They do not
    // indicate a failure to record user actions or playback evidence.
    if (event.message === 'ResizeObserver loop completed with undelivered notifications.'
      || event.message === 'ResizeObserver loop limit exceeded') return
    if (active) failures.push('Unhandled browser error.')
  })
  window.addEventListener('unhandledrejection', () => { if (active) failures.push('Unhandled browser rejection.') })
  const play = HTMLMediaElement.prototype.play
  const known = new WeakMap<HTMLMediaElement, number>()
  HTMLMediaElement.prototype.play = function () {
    if (active) {
      let id = known.get(this)
      if (id === undefined) {
        id = ++sequence; known.set(this, id)
        const source = this.currentSrc || this.src
        const identity = id
        task(async () => {
          if (!source.startsWith('blob:') && !source.startsWith('data:audio/')) throw new Error('Unsupported playback source.')
          const response = await fetch(source)
          const bytes = await response.arrayBuffer()
          if (bytes.byteLength > 64 * 1024 * 1024) throw new Error('Playback observation bound exceeded.')
          hashes.set(identity,{ kind: 'media-bytes', id: identity, digest: await hash(bytes), bytes: bytes.byteLength })
        })
        for (const event of ['playing', 'ended', 'pause', 'error']) this.addEventListener(event, () => {
          if(event==='playing')playing.add(identity);else playing.delete(identity)
          if (active) record({ kind: 'media-state', id: identity, event })
        })
      }
      record({ kind: 'media-play', id, rate: this.playbackRate, volume: this.volume })
    }
    return play.call(this)
  }
  const start = AudioBufferSourceNode.prototype.start
  AudioBufferSourceNode.prototype.start = function (when?: number, offset?: number, duration?: number) {
    if (active && this.buffer) {
      const id = ++sequence
      playing.add(id)
      const buffer = this.buffer
      const arrays = Array.from({ length: buffer.numberOfChannels }, (_, channel) => buffer.getChannelData(channel).slice().buffer)
      record({ kind: 'pcm-start', id, rate: buffer.sampleRate, channels: buffer.numberOfChannels, frames: buffer.length, offset: offset ?? 0, duration: duration ?? null })
      task(async () => {hashes.set(id,{ kind: 'pcm-bytes', id, digests: await Promise.all(arrays.map(hash)) })})
      this.addEventListener('ended', () => { playing.delete(id);if (active) record({ kind: 'pcm-ended', id }) }, { once: true })
    }
    if (duration !== undefined) start.call(this, when ?? 0, offset ?? 0, duration)
    else if (offset !== undefined) start.call(this, when ?? 0, offset)
    else start.call(this, when ?? 0)
  }
  const Worklet=window.AudioWorkletNode
  window.AudioWorkletNode=class extends Worklet{
    constructor(context:BaseAudioContext,name:string,options?:AudioWorkletNodeOptions){
      super(context,name,options)
      if(!active)return
      const id=++sequence
      record({kind:'worklet',id,name})
      const post=this.port.postMessage.bind(this.port)
      this.port.postMessage=(message:unknown,transfer?:Transferable[]|StructuredSerializeOptions)=>{
        if(active){
          const item=message as {type?:string;channels?:Float32Array[];sourceRate?:number;seconds?:number}
          if(name!=='skellyspeak-scrub'||!['load','start','move','stop'].includes(item.type??''))failures.push('Unsupported audio worklet observation.')
          else{
            record({kind:'worklet-command',id,type:item.type,sourceRate:item.sourceRate,seconds:item.seconds})
            if(item.type==='start')playing.add(id)
            if(item.type==='stop')playing.delete(id)
            if(item.channels){const arrays=item.channels.map(channel=>channel.slice().buffer);task(async()=>{hashes.set(id,{kind:'worklet-bytes',id,digests:await Promise.all(arrays.map(hash))})})}
          }
        }
        if(Array.isArray(transfer))post(message,transfer);else post(message,transfer)
      }
    }
  }
  host.__workflowObserve = value => { active = value }
  host.__workflowCheckpoint = async () => {
    // A caller's ended callback may run before our observer in the same browser
    // event dispatch. Let that dispatch finish before testing playback state.
    await new Promise(yes=>setTimeout(yes,0))
    const deadline = performance.now() + 5000
    while (pending) {
      if (performance.now() > deadline) throw new Error('Playback observation did not finish.')
      await new Promise(yes => setTimeout(yes, 10))
    }
    if (failures.length) throw new Error(failures[0])
    if (playing.size) throw new Error('Wait for playback to finish before saving the recording.')
    return {
      text: document.getElementById('root')?.innerText,
      fields: Array.from(document.querySelectorAll('#root input,#root textarea,#root select')).map(element => {
        const field = element as HTMLInputElement
        if (field.type === 'password') return { type: 'password', omitted: true }
        return { tag: field.tagName, type: field.type, value: field.value, checked: field.checked, disabled: field.disabled }
      }),
      states: Array.from(document.querySelectorAll('#root [aria-expanded],#root [aria-checked],#root [aria-pressed],#root [aria-busy]')).map(element => Array.from(element.attributes).filter(attr => ['aria-expanded','aria-checked','aria-pressed','aria-busy'].includes(attr.name)).map(attr => [attr.name,attr.value])),
      // Preserve each source's transition order. Hash completion order is not a
      // playback fact; compare those separately by source identity.
      media,
      hashes:[...hashes.entries()].sort((a,b)=>a[0]-b[0]),
    }
  }
}
