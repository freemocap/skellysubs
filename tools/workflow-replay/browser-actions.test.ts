import assert from 'node:assert/strict'
import test from 'node:test'
import { hardwareAction, recordedWheel } from './browser-actions.ts'

test('keyboard preserves individual down/up, repeats and held modifiers', () => {
  const detail = { type: 'keydown', key: ' ', code: 'Space', keyCode: 32, location: 0, repeat: true, ctrl: true, shift: true }
  assert.deepEqual(hardwareAction('key', detail), { method: 'Input.dispatchKeyEvent', params: { type: 'rawKeyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32, location: 0, autoRepeat: true, modifiers: 10, isKeypad: false } })
  assert.equal((hardwareAction('key', { ...detail, type: 'keyup' })!.params as { type: string }).type, 'keyUp')
  assert.throws(() => hardwareAction('key', { ...detail, type: 'keypress' }))
})
test('mouse and pixel wheel retain modifiers while touch and unsupported wheel units fail', () => {
  const detail = { type: 'pointerup', pointerType: 'mouse', x: 20, y: 40, button: 0, buttons: 0, alt: true, clicks: 2 }
  const params = hardwareAction('pointer', detail)!.params as { modifiers: number; clickCount: number }
  assert.equal(params.modifiers, 1)
  assert.equal(params.clickCount, 2)
  assert.throws(() => hardwareAction('pointer', { ...detail, pointerType: 'touch' }))
  assert.throws(() => hardwareAction('wheel', { ...detail, dx: 0, dy: 10, mode: 1 }))
  assert.equal((hardwareAction('wheel', { ...detail, dx: 0, dy: 10, mode: 0 })!.params as { modifiers: number }).modifiers, 1)
})

test('recorded wheel delivery preserves handler inputs without requesting native inertia', () => {
  assert.deepEqual(recordedWheel({ x: 275, y: 225, dx: 0, dy: -100, mode: 0, ctrl: true }), {
    bubbles: true, cancelable: true, clientX: 275, clientY: 225,
    deltaX: 0, deltaY: -100, deltaMode: 0,
    ctrlKey: true, metaKey: false, altKey: false, shiftKey: false,
  })
  assert.throws(() => recordedWheel({ x: 1, y: 2, dx: 0, dy: 10, mode: 1 }))
  assert.throws(() => recordedWheel({ x: NaN, y: 2, dx: 0, dy: 10, mode: 0 }))
})
