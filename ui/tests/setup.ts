import { clearInspectionResources } from '../src/platform/audio/inspection-resource'
// Testing Library's DOM matchers (toBeInTheDocument, toBeDisabled, ...). The
// `/vitest` entry both extends `expect` and declares the matcher types, so
// `tsc` sees them too. Harmless for the pure-function tests, which run in the
// node environment and simply never call them.
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'
import { URL as FileUrl } from 'node:url'
import { afterEach, beforeEach, vi } from 'vitest'

// Zustand stores are module-level singletons. The shared mock resets every
// store after each test and is registered explicitly so its location is independent
// of Vitest's root-level __mocks__ discovery convention.
vi.mock('zustand', async () => ({
  ...await vi.importActual<typeof import('zustand')>('zustand'),
  ...await import('./mocks/zustand'),
}))

if (typeof document !== 'undefined') {
  const { cleanup } = await import('@testing-library/react')
  afterEach(cleanup)

  // jsdom implements no media queries, and the app asks about viewport width
  // to choose between its desktop and mobile layouts. Answer "not mobile";
  // a test that cares about the mobile surface can override this.
  if (!window.matchMedia) {
    window.matchMedia = ((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })) as unknown as typeof window.matchMedia
  }

  // jsdom has no layout observer; component tests can supply measured boxes.
  globalThis.ResizeObserver ??= class { observe() {} unobserve() {} disconnect() {} }

  // jsdom loads no stylesheet. The AI workspace measures its geometry from root
  // tokens, so apply the stylesheet's own pixel-valued geometry tokens.
  const tokens = readFileSync(new FileUrl('../src/styles/foundations/tokens.css', import.meta.url), 'utf8')
  const root = tokens.slice(tokens.indexOf(':root {'), tokens.indexOf('}', tokens.indexOf(':root {')))
  for (const [, name, value] of root.matchAll(/(--graph-[\w-]+|--border-width):\s*([\d.]+px);/g)) document.documentElement.style.setProperty(name, value)

  // Nothing under test scrolls, but the chat stream asks the DOM to.
  Element.prototype.scrollIntoView ??= vi.fn()
  // Model dialog visibility for all modal consumers; geometry stays browser-tested.
  HTMLDialogElement.prototype.showModal ??= function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close ??= function () { this.removeAttribute('open') }
  // jsdom has no top-layer popover implementation; layout is verified in-browser.
  HTMLElement.prototype.showPopover ??= function () { this.style.display = 'block' }
  HTMLElement.prototype.hidePopover ??= function () { this.style.display = 'none' }
}

beforeEach(() => clearInspectionResources())
