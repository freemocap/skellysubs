import type { DesktopConnection } from './cdp.ts'

export function desktopActions(connection:DesktopConnection) {
  const wait=async(expression:string)=>{
    const deadline=Date.now()+30000
    while(!await connection.evaluate<boolean>(expression).catch(()=>false)){
      if(Date.now()>deadline)throw new Error(`Desktop expectation timed out: ${expression.slice(0,160)}`)
      await new Promise(yes=>setTimeout(yes,25))
    }
  }
  const click=async(selector:string)=>{
    await wait(`!!document.querySelector(${JSON.stringify(selector)})`)
    const point=await connection.evaluate<{x:number;y:number}>(`(async()=>{const e=document.querySelector(${JSON.stringify(selector)});e.scrollIntoView({block:'center',behavior:'instant'});await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);const r=e.getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`)
    await connection.command('Input.dispatchMouseEvent',{type:'mousePressed',...point,button:'left',buttons:1,clickCount:1})
    await connection.command('Input.dispatchMouseEvent',{type:'mouseReleased',...point,button:'left',buttons:0,clickCount:1})
  }
  return{wait,click}
}
