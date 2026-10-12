import assert from 'node:assert/strict'
import { test } from 'node:test'
import { hasStartupSurface } from './android-startup.ts'

const pkg = 'com.freemocap.skellyspeak'
const label = 'Welcome to SkellySpeak'
const node = (attributes: string) => `<hierarchy><node ${attributes} /></hierarchy>`

test('cold startup requires actual app content, not the empty WebView shell', () => {
  const blank = `<hierarchy><node package="${pkg}" class="android.webkit.WebView" text="" enabled="true"><node package="${pkg}" class="android.webkit.WebView" text="" enabled="true" /></node></hierarchy>`
  assert.equal(hasStartupSurface(blank, pkg, label), false)
  assert.equal(hasStartupSurface(node(`package="${pkg}" enabled="true" text="${label}"`), pkg, label), true)
  assert.equal(hasStartupSurface(node(`package="android.launcher" enabled="true" text="${label}"`), pkg, label), false)
  assert.equal(hasStartupSurface(node(`package="${pkg}" enabled="false" text="${label}"`), pkg, label), false)
  assert.throws(() => hasStartupSurface('ERROR: could not get idle state.', pkg, label), /complete UI hierarchy/)
})

test('supports accessible labels and XML escaping without double decoding', () => {
  assert.equal(hasStartupSurface(node(`package="${pkg}" enabled="true" content-desc="Settings"`), pkg, 'Settings'), true)
  assert.equal(hasStartupSurface(node(`package="${pkg}" enabled="true" text="A &amp; B &quot;C&quot;"`), pkg, 'A & B "C"'), true)
  assert.equal(hasStartupSurface(node(`package="${pkg}" enabled="true" text="&amp;quot;"`), pkg, '&quot;'), true)
})

test('fails immediately on startup or render errors even if expected content is also present', () => {
  for (const failure of ['Retry startup', 'Reload app', 'Reload view', 'Configuration could not be loaded']) {
    const xml = `<hierarchy><node package="${pkg}" enabled="true" text="${label}" /><node package="${pkg}" enabled="true" text="${failure}" /></hierarchy>`
    assert.throws(() => hasStartupSurface(xml, pkg, label), /App displayed an error/)
  }
  assert.throws(() => hasStartupSurface(node(`package="${pkg}" enabled="true" text="Could not open SkellySpeak: state not managed for field state on command get_startup_state"`), pkg, label), /native state was not registered/)
})
