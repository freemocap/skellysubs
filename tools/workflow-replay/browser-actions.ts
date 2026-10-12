export type ActionDetail = { value?: string; checked?: boolean; key?: string; code?: string; ctrl?: boolean; meta?: boolean; alt?: boolean; shift?: boolean; x?: number; y?: number; dx?: number; dy?: number; mode?: number; button?: number; buttons?: number; type?: string; clicks?: number; eventType?: string; data?: string | null; composing?: boolean; inputType?: string; repeat?: boolean; location?: number; keyCode?: number; pointerType?: string }
const modifiers = (detail: ActionDetail) => (detail.alt ? 1 : 0) | (detail.ctrl ? 2 : 0) | (detail.meta ? 4 : 0) | (detail.shift ? 8 : 0)

/** Deliver wheel handlers; recorded scroll positions own browser scrolling.
 * Re-running native wheel inertia as well would apply scrolling twice. */
export function recordedWheel(detail: ActionDetail): WheelEventInit {
  if (![detail.x, detail.y, detail.dx, detail.dy].every(Number.isFinite) || detail.mode !== 0) throw new Error('Unsupported wheel coordinates or delta units.')
  return { bubbles: true, cancelable: true, clientX: detail.x, clientY: detail.y,
    deltaX: detail.dx, deltaY: detail.dy, deltaMode: 0,
    ctrlKey: !!detail.ctrl, metaKey: !!detail.meta, altKey: !!detail.alt, shiftKey: !!detail.shift }
}

/** One recorded hardware phase maps to one CDP phase. Text values are applied
 * separately by recorded input events; key dispatch never invents key releases. */
export function hardwareAction(action: string, detail: ActionDetail): { method: string; params: object } | null {
  if (action === 'key') {
    if (!['keydown', 'keyup'].includes(detail.type ?? '') || typeof detail.key !== 'string' || typeof detail.code !== 'string' || !Number.isInteger(detail.keyCode) || detail.keyCode! < 0 || detail.keyCode! > 255 || ![0, 1, 2, 3].includes(detail.location ?? -1)) throw new Error('Invalid keyboard phase.')
    return { method: 'Input.dispatchKeyEvent', params: { type: detail.type === 'keyup' ? 'keyUp' : 'rawKeyDown', key: detail.key, code: detail.code, modifiers: modifiers(detail), windowsVirtualKeyCode: detail.keyCode, autoRepeat: !!detail.repeat, location: detail.location, isKeypad: detail.location === 3 } }
  }
  if (action === 'pointer') {
    if (detail.pointerType !== 'mouse' || !['pointerdown', 'pointerup', 'pointermove'].includes(detail.type ?? '') || !Number.isFinite(detail.x) || !Number.isFinite(detail.y) || ![-1, 0, 1, 2].includes(detail.button ?? -2) || !Number.isInteger(detail.buttons) || detail.buttons! < 0 || detail.buttons! > 7) throw new Error('Unsupported pointer input.')
    const type = detail.type === 'pointerdown' ? 'mousePressed' : detail.type === 'pointerup' ? 'mouseReleased' : 'mouseMoved'
    return { method: 'Input.dispatchMouseEvent', params: { type, x: detail.x, y: detail.y, buttons: detail.buttons, button: detail.button === 2 ? 'right' : detail.button === 1 ? 'middle' : detail.button === 0 ? 'left' : 'none', clickCount: type === 'mouseMoved' ? 0 : Math.max(1, detail.clicks ?? 1), modifiers: modifiers(detail), pointerType: 'mouse' } }
  }
  if (action === 'wheel') {
    if (![detail.x, detail.y, detail.dx, detail.dy].every(Number.isFinite) || detail.mode !== 0) throw new Error('Unsupported wheel coordinates or delta units.')
    return { method: 'Input.dispatchMouseEvent', params: { type: 'mouseWheel', x: detail.x, y: detail.y, deltaX: detail.dx, deltaY: detail.dy, modifiers: modifiers(detail) } }
  }
  return null
}
