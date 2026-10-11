import type {DesktopConnection} from './cdp.ts'
/** These development controls return only recorder paths/status and fixed errors. */
export async function workflowControl<T>(connection:DesktopConnection,action:'start'|'stop') {
  const result=await connection.evaluate<{value?:T;error?:{code?:string;message?:string}}>(`window.__TAURI_INTERNALS__.invoke('workflow_control',{action:${JSON.stringify(action)}}).then(value=>({value}),error=>({error:{code:error.code,message:error.message}}))`)
  if(result.error)throw Error(`Workflow ${action}: ${result.error.code??'error'}: ${result.error.message??'failed'}`)
  return result.value as T
}
