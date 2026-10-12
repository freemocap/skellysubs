import { AskCoachContext } from '../learning/AskCoachButton'
// @vitest-environment jsdom
import { fireEvent, render as renderView, screen, within } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { SavedGlossText as SharedSavedGlossText } from './SavedGlossText'
import { ReadingPreferencesContext } from './ReadingPreferences'

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open') }
})

// The reported popup displayed these two clitics in three separate field passes.
const enabled = { autoTranslate: true, alwaysRomanize: true, alwaysPronunciation: true }
function render(node: React.ReactNode) {
  return renderView(node, { wrapper: ({ children }) => <ReadingPreferencesContext value={enabled}>{children}</ReadingPreferencesContext> })
}
function SavedGlossText(props: React.ComponentProps<typeof SharedSavedGlossText>) {
  return <SharedSavedGlossText showAids={false} {...props} />
}

const houses = [
  { start: 0, end: 2, kind: 'gloss' as const, gloss: 'the', romanization: 'al-', pronunciation: 'il' },
  { start: 2, end: 6, kind: 'gloss' as const, gloss: 'houses', romanization: 'buyūt', pronunciation: 'buyuut' },
]

it('tags only displayed gloss annotations or an open word helper', () => {
  const graphTarget = { 'data-graph-results': '["gloss"]' }
  const view = render(<SavedGlossText graphTarget={graphTarget} text="البيوت" segments={houses} />)
  expect(document.querySelector('[data-graph-results]')).toBeNull()
  view.rerender(<ReadingPreferencesContext value={enabled}><SavedGlossText graphTarget={graphTarget} showAids text="البيوت" segments={houses} /></ReadingPreferencesContext>)
  expect(document.querySelector('.wg')).toHaveAttribute('data-graph-results', '["gloss"]')
  expect(screen.getByRole('button', { name: 'البيوت' })).not.toHaveAttribute('data-graph-results')
  fireEvent.click(screen.getByRole('button', { name: 'البيوت' }))
  expect(screen.getByRole('group', { name: 'Word help' })).toHaveAttribute('data-graph-results', '["gloss"]')
})

it('groups each Arabic part once with unlabeled romanization and no redundant pronunciation', () => {
  render(<SavedGlossText text="البيوت" segments={houses} />)
  fireEvent.click(screen.getByRole('button', { name: 'البيوت' }))
  const help = screen.getByRole('group', { name: 'Word help' })
  expect(within(help).getAllByText('ال', { exact: true })).toHaveLength(1)
  expect(within(help).getAllByText('بيوت', { exact: true })).toHaveLength(1)
  const parts = help.querySelectorAll('.gloss-help-part')
  expect(parts).toHaveLength(2)
  expect(parts[0]).toHaveTextContent('الtheal-')
  expect(parts[1]).toHaveTextContent('بيوتhousesbuyūt')
  expect(help.querySelector('.wpronunciation')).toBeNull()
  expect(help).not.toHaveTextContent(/Romanization|Pronunciation/)
})

it('shows identical sound values once without field labels', () => {
  render(<SavedGlossText text="البيوت" segments={houses.map(part => ({ ...part, pronunciation: part.romanization }))} />)
  fireEvent.click(screen.getByRole('button', { name: 'البيوت' }))
  const help = screen.getByRole('group', { name: 'Word help' })
  expect(within(help).getAllByText('buyūt', { exact: true })).toHaveLength(1)
  expect(help.querySelector('.wpronunciation')).toBeNull()
  expect(help).not.toHaveTextContent(/Romanization|Pronunciation/)
})

it('keeps actual word help available alongside inline aids without substituting pronunciation', () => {
  render(<ReadingPreferencesContext value={{autoTranslate: true, alwaysRomanize: true, alwaysPronunciation: false}}>
    <SavedGlossText showAids text="البيوت" segments={houses} />
  </ReadingPreferencesContext>)
  fireEvent.click(screen.getByRole('button', { name: 'البيوت' }))
  const help = screen.getByRole('group', { name: 'Word help' })
  expect(within(help).getByText('the')).toBeVisible()
  expect(within(help).getByText('houses')).toBeVisible()
  expect(within(help).getByText('al-')).toBeVisible()
  expect(within(help).getByText('buyūt')).toBeVisible()
  expect(help.querySelector('.wpronunciation')).toBeNull()
  expect(help).not.toHaveTextContent(/Romanization|Pronunciation/)
  expect(screen.getByText('the houses')).toBeVisible()
  expect(screen.getByText('al-buyūt')).toBeVisible()
})

it.each([undefined, ''])('uses unlabeled pronunciation when romanization is unavailable (%s)', romanization => {
  render(<SavedGlossText text="بيوت" segments={[
    { start: 0, end: 4, kind: 'gloss', gloss: 'houses', romanization, pronunciation: 'buyuut' },
  ]} />)
  fireEvent.click(screen.getByRole('button', { name: 'بيوت' }))
  const help = screen.getByRole('group', { name: 'Word help' })
  expect(within(help).getByText('buyuut')).toBeVisible()
  expect(help.querySelector('.wroman')).toBeNull()
  expect(help).not.toHaveTextContent(/Romanization|Pronunciation/)
})

it('does not substitute pronunciation for existing unsupported romanization', () => {
  render(<ReadingPreferencesContext value={{ autoTranslate: false, alwaysRomanize: false, alwaysPronunciation: false, supportsRomanization: false }}>
    <SavedGlossText showAids text="البيوت" segments={houses} />
  </ReadingPreferencesContext>)
  fireEvent.click(screen.getByRole('button', { name: 'البيوت' }))
  const help = screen.getByRole('group', { name: 'Word help' })
  expect(within(help).queryByText('buyuut')).toBeNull()
  expect(help.querySelector('.wroman')).toBeNull()
  expect(help).not.toHaveTextContent(/Romanization|Pronunciation/)
})

it.each([false, true])('keeps help anchored when its content is clicked (mobile=%s)', mobile => {
  const media = vi.spyOn(window, 'matchMedia').mockReturnValue({
    matches: mobile, addEventListener: vi.fn(), removeEventListener: vi.fn(),
  } as unknown as MediaQueryList)
  const boxes = vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    return this.classList.contains('saved-word-help')
      ? new DOMRect(0, 0, 160, 70) : new DOMRect(120, 300, 40, 30)
  })
  try {
    render(<SavedGlossText text="الأكلة" segments={[
      { start: 0, end: 2, kind: 'gloss', gloss: 'the', romanization: 'al-', pronunciation: 'al' },
      { start: 2, end: 6, kind: 'gloss', gloss: 'dish', romanization: 'aklah', pronunciation: 'ak-la' },
    ]} />)
    const word = screen.getByRole('button', { name: 'الأكلة' })
    fireEvent.click(word)
    const help = screen.getByRole('group', { name: 'Word help' })
    if (mobile) expect(help).not.toHaveAttribute('popover')
    else expect(help).toHaveAttribute('popover', 'manual')
    expect(help.style.top).toBe('226px')
    fireEvent.click(within(help).getByText('dish'))
    expect(screen.queryByRole('dialog')).toBeNull()
    for (const value of ['the', 'dish', 'al-', 'aklah']) {
      expect(within(help).getByText(value)).toBeVisible()
    }
    expect(screen.queryByRole('button', { name: 'Ask the coach' })).toBeNull()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('group', { name: 'Word help' })).toBeNull()
    expect(word).toHaveFocus()
    fireEvent.keyDown(word, { key: 'Enter' })
    expect(screen.getByRole('group', { name: 'Word help' })).toBeVisible()
  } finally { media.mockRestore(); boxes.mockRestore() }
})


it('closes compact help and passes the selected occurrence and saved information to the shared coach action', () => {
  const ask = vi.fn()
  render(<AskCoachContext value={ask}><SavedGlossText text="sí, sí" segments={[
    { start: 0, end: 2, kind: 'gloss', gloss: 'yes' },
    { start: 4, end: 6, kind: 'gloss', gloss: 'indeed', pronunciation: 'see' },
  ]} /></AskCoachContext>)
  fireEvent.click(screen.getAllByRole('button', { name: 'sí' })[1])
  fireEvent.click(screen.getByRole('button', { name: 'Ask the coach' }))
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.queryByRole('group', { name: 'Word help' })).toBeNull()
  expect(ask).toHaveBeenCalledTimes(1)
  const question = ask.mock.calls[0][0]
  expect(question).toContain('“sí”')
  expect(question).toContain('“sí, sí”')
  expect(question).toContain('indeed')
  expect(question).toContain('see')
  expect(question).not.toContain('"yes"')
})
