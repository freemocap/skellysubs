import assert from 'node:assert/strict'
import test from 'node:test'
import { presentationStorage } from './browser-state.ts'

test('browser snapshot retains bounded presentation preferences and excludes credentials and arbitrary keys', () => {
  assert.deepEqual(presentationStorage({ 'skellyspeak.practice-view': 'cards', skellyspeak_recorder_chat_pad: '1', skellyspeak_pane_reading: 'open', token: 'secret', skellyspeak_access_token: 'secret', skellyspeak_recorder_other_pad: '1' }), { 'skellyspeak.practice-view': 'cards', skellyspeak_recorder_chat_pad: '1', skellyspeak_pane_reading: 'open' })
  assert.throws(() => presentationStorage({ 'skellyspeak.practice-view': 'x'.repeat(1025) }))
  assert.deepEqual(presentationStorage({ token: 'x'.repeat(1025) }), {})
})
