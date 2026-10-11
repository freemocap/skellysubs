/** Mute belongs to replay output; capture retains the learner's audio settings. */
export function replayOptions(args:string[], needsRecording:boolean) {
  const mute=args.includes('--mute')
  if(args.filter(arg=>arg==='--mute').length>1 || args.some(arg=>arg.startsWith('--')&&arg!=='--mute'))throw Error('Supported replay option: --mute')
  const paths=args.filter(arg=>arg!=='--mute')
  if(paths.length!==(needsRecording?1:0))throw Error(needsRecording?'Usage: workflow:replay RECORDING [--mute]':'Usage: workflow:record [--mute]')
  return {mute,recording:paths[0]}
}

export function browserArguments(debugPort:number, replay:boolean, mute:boolean) {
  if(mute&&!replay)throw Error('Output muting is only supported for replay.')
  return `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${debugPort}${mute?' --mute-audio':''}`
}
