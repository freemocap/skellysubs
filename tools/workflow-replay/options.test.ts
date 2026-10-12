import assert from 'node:assert/strict'
import test from 'node:test'
import {browserArguments,replayOptions} from './options.ts'

test('mute is opt-in for replay and may be selected for automatic replay after capture',()=>{
  assert.deepEqual(replayOptions(['C:/saved recording','--mute'],true),{recording:'C:/saved recording',mute:true})
  assert.equal(replayOptions(['C:/saved'],true).mute,false)
  assert.deepEqual(replayOptions(['--mute'],false),{recording:undefined,mute:true})
  assert.throws(()=>replayOptions(['--muted'],false))
  assert.throws(()=>replayOptions(['--mute','--mute','path'],true))
  assert.throws(()=>replayOptions([],true))
  assert.match(browserArguments(1234,true,true),/ --mute-audio$/)
  assert.doesNotMatch(browserArguments(1234,false,false),/mute/)
  assert.throws(()=>browserArguments(1234,false,true))
})
