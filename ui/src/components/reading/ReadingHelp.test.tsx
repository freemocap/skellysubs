import { Markdown } from './Markdown'
import { SavedReadingProvider } from './SavedReadingProvider'
// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { useMemo, useState } from 'react'
import { ReadingPreferencesContext } from './ReadingPreferences'
import { ReadingHelp } from './ReadingHelp'
import { useReadingActions, ReadingScopeContext, type ReadingServices } from './ReadingContext'
import { TargetText } from './TargetText'
import { SavedGlossText } from './SavedGlossText'
import { ReadingLanguageScope } from './ReadingLanguageScope'
import type { ReadingResult } from '../../generated/contracts'

/** Opens every message's ⋯ menu, where Words, Analysis and Pronunciation live. */
const openMenus = () => screen.queryAllByRole('button', { name: 'More actions' }).forEach(button => { if (button.getAttribute('aria-expanded') !== 'true') fireEvent.click(button) })


vi.mock('../../platform/ipc/tauri', () => ({ languageFor: (language: string) => ({ languageTag: language === 'arabic' ? 'ar' : 'es', romanization: language === 'arabic' ? 'ala' : null }) }))
const scope = { language: 'spanish', variety: 'spanish-spain', explanation: 'english', explanationVariety: 'english-us' }
const languages = [{ code: 'spanish', name: 'Spanish', languageTag: 'es', defaultVariety: 'spanish-spain', varieties: [{ id: 'spanish-spain', label: 'Spain' }] }, { code: 'arabic', name: 'Arabic', languageTag: 'ar', defaultVariety: 'arabic-egypt', varieties: [{ id: 'arabic-egypt', label: 'Egypt' }] }]
const result = { gloss: { coverage: 'complete', segments: [{start:0,end:4,kind:'gloss',gloss:'hello'}] }, audioBase64: null, receipt: { providerId: 'receipt-1' } } as ReadingResult
const services: ReadingServices = { read: vi.fn(), saved: vi.fn(), speak: vi.fn(), activity: vi.fn() }
beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(services.saved!).mockResolvedValue(null)
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
  vi.mocked(services.read).mockResolvedValue(result)
  vi.mocked(services.speak).mockResolvedValue({ providerId: 'speech-1' })
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', '') }
  HTMLDialogElement.prototype.close = function () { this.removeAttribute('open') }
})
function app(children: React.ReactNode) { return render(<ReadingPreferencesContext value={{autoTranslate:true, alwaysRomanize:true, alwaysPronunciation:true}}><ReadingScopeContext value={scope}><ReadingHelp services={services} languages={languages}>{children}</ReadingHelp></ReadingScopeContext></ReadingPreferencesContext>) }

it('shows only the saved translation on tap without added labels, actions or inference', async () => {
  vi.mocked(services.saved!).mockResolvedValue({...result, gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'dictionary hello'}],coverage:'partial'},dictionaryAnchors:[{start:0,end:4}]})
  app(<TargetText text="Hola" />)
  fireEvent.click(screen.getByRole('button',{name:'Hola'}))
  const helper = await screen.findByRole('group',{name:'Word help'})
  await waitFor(() => expect(helper).toHaveTextContent('dictionary hello'))
  expect(helper).toHaveTextContent('dictionary hello')
  expect(services.read).not.toHaveBeenCalled()
  expect(helper).not.toHaveTextContent('Dictionary meaning')
  expect(within(helper).queryByRole('button',{name:'Explain in context'})).not.toBeInTheDocument()
  expect(within(helper).queryByRole('button',{name:'Retry word meanings'})).not.toBeInTheDocument()
})

it('does not treat dictionary coverage as a complete whole-passage analysis', async () => {
  const { useReadingLookup } = await import('./ReadingContext')
  let lookup: ReturnType<typeof useReadingLookup> = null
  function Probe() { lookup = useReadingLookup(); return null }
  vi.mocked(services.saved!).mockResolvedValue({...result,gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'dictionary hello'}],coverage:'partial'},dictionaryAnchors:[{start:0,end:4}]})
  app(<Probe />)
  const resolved = await lookup!({...scope,text:'Hola',aid:'word_gloss'},new AbortController().signal)
  expect(services.read).toHaveBeenCalledOnce()
  expect(resolved.gloss?.segments[0].gloss).toBe('hello')
})

it('keeps saved translations compact on mouse hover for dictionary and contextual sources', async () => {
  vi.useFakeTimers()
  try {
    vi.mocked(services.saved!).mockResolvedValueOnce({...result,gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'dictionary hello'}],coverage:'partial'},dictionaryAnchors:[{start:0,end:4}]}).mockResolvedValue(result)
    app(<TargetText text="Hola" />)
    const word = screen.getByRole('button',{name:'Hola'})
    const hover = () => {
      const event = new MouseEvent('pointerover',{bubbles:true})
      Object.defineProperty(event,'pointerType',{value:'mouse'})
      fireEvent(word,event)
    }
    hover()
    await act(async () => { vi.advanceTimersByTime(350) })
    expect(screen.getByRole('group',{name:'Word help'})).toHaveTextContent('dictionary hello')
    expect(screen.getByRole('group',{name:'Word help'})).not.toHaveTextContent('Dictionary meaning')
    expect(services.read).not.toHaveBeenCalled()
    fireEvent.keyDown(document,{key:'Escape'})
    hover()
    await act(async () => { vi.advanceTimersByTime(350) })
    const helper = screen.getByRole('group',{name:'Word help'})
    expect(helper).toHaveTextContent('hello')
    expect(helper).not.toHaveTextContent('Dictionary meaning')
    expect(services.read).not.toHaveBeenCalled()
  } finally { vi.useRealTimers() }
})

it('keeps an inspector request alive when unrelated saved meanings update', async () => {
  let finish!: (value: ReadingResult) => void
  vi.mocked(services.read).mockImplementation(() => new Promise(resolve => { finish = resolve }))
  function Controls() {
    const actions = useReadingActions()
    const [updated, setUpdated] = useState(false)
    const sources = useMemo(() => updated ? [{ scope, text:'casa', segments:[{start:0,end:4,kind:'gloss' as const,gloss:'house'}] }] : [], [updated])
    return <><button onClick={() => actions?.inspect({scope, text:'Hola', start:0, end:4})}>Inspect</button>
      <button onClick={() => setUpdated(true)}>Update saved meanings</button>
      <SavedReadingProvider sources={sources}>{null}</SavedReadingProvider></>
  }
  app(<Controls />)
  fireEvent.click(screen.getByRole('button', {name:'Inspect'}))
  await waitFor(() => expect(services.read).toHaveBeenCalledOnce())
  const signal = vi.mocked(services.read).mock.calls[0][1]
  fireEvent.click(screen.getByRole('button', {name:'Update saved meanings'}))
  expect(signal.aborted).toBe(false)
  expect(services.read).toHaveBeenCalledOnce()
  await act(async () => finish(result))
  expect(screen.getByRole('dialog', {name:'Word help'})).toHaveTextContent('hello')
})

it('requests help only on explicit actions and delegates reuse to the local service', async () => {
  app(<TargetText text="Hola" />)
  expect(services.read).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
  await waitFor(() => expect(services.read).toHaveBeenCalledOnce())
  expect(services.read).toHaveBeenCalledWith({ ...scope, text: 'Hola', aid:'word_gloss' }, expect.any(AbortSignal))
  await waitFor(() => expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('hello'))
  fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
  fireEvent.keyDown(screen.getByRole('button', { name: 'Hola' }), { key: 'Enter' })
  await waitFor(() => expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('hello'))
  expect(services.read).toHaveBeenCalledTimes(2)
})

it('speaks an exact saved source occurrence without fetching glosses or opening its parent action', async () => {
  const parent = vi.fn()
  app(<div onClick={parent}><SavedGlossText text="sí, sí" segments={[{start:0,end:2,kind:'gloss',gloss:'yes'}, {start:4,end:6,kind:'gloss',gloss:'indeed'}]} /></div>)
  fireEvent.click(screen.getAllByRole('button', { name: 'sí' })[1])
  fireEvent.click(screen.getByRole('button', { name: 'Read aloud: sí' }))
  await waitFor(() => expect(services.speak).toHaveBeenCalledWith({ ...scope, text: 'sí', aid:'speech' }, expect.any(AbortSignal), expect.any(Function), expect.objectContaining({ text: expect.any(String), start: expect.any(Number), end: expect.any(Number) })))
  expect(services.read).not.toHaveBeenCalled(); expect(parent).not.toHaveBeenCalled()
})

it('reads a language-browser source in its own language, not the active conversation language', async () => {
  app(<ReadingLanguageScope language="arabic" variety="arabic-egypt"><TargetText text="كتاب" /></ReadingLanguageScope>)
  fireEvent.click(screen.getByRole('button', { name: 'كتاب' }))
  await waitFor(() => expect(services.read).toHaveBeenCalledWith(expect.objectContaining({ text:'كتاب', language:'arabic', variety:'arabic-egypt' }), expect.any(AbortSignal)))
})

it('does not show a selection popup or request help when selecting text or clicking controls', () => {
  app(<><textarea aria-label="Config" defaultValue="key: Hola" /><button>Topic</button></>)
  const field = screen.getByRole('textbox') as HTMLTextAreaElement
  field.focus(); field.setSelectionRange(5,9); fireEvent.keyUp(field, {key:'Shift'})
  fireEvent.pointerUp(screen.getByRole('button', {name:'Topic'}))
  fireEvent.click(screen.getByRole('button', {name:'Topic'}))
  expect(screen.queryByRole('button', { name:'Inspect selected text' })).toBeNull()
  expect(services.read).not.toHaveBeenCalled()
  expect(field.value).toBe('key: Hola')
})

it('closes pending work and offers speech even when word meanings fail', async () => {
  vi.mocked(services.read).mockRejectedValue({message:'Provider refused',diagnostics:{request_id:'failed-1'}})
  app(<TargetText text="Hola" />)
  fireEvent.click(screen.getByRole('button', { name:'Hola' }))
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Provider refused'))
  const helper=screen.getByRole('group', { name: 'Word help' })
  fireEvent.click(within(helper).getByText('Response details'))
  await waitFor(() => expect(helper).toHaveTextContent('failed-1'))
  expect(screen.queryByRole('dialog')).toBeNull()
  fireEvent.click(within(helper).getByRole('button',{name:'Read aloud: Hola'}))
  await waitFor(() => expect(services.speak).toHaveBeenCalledOnce())
  fireEvent.click(screen.getByRole('button',{name:'Hola'}))
  expect(vi.mocked(services.read).mock.calls[0][1].aborted).toBe(true)
})


it('keeps the final Arabic word and sentence mounted through hover, request completion, pinning and dismissal', async () => {
  vi.useFakeTimers()
  try {
    const source = 'بتحب البيوت القديمة'
    const gloss = { ...result, gloss: { coverage: 'complete', segments: [{start:11,end:18,kind:'gloss',gloss:'old',romanization:'qadīme'}] } } as ReadingResult
    vi.mocked(services.read).mockResolvedValue(gloss)
    const view = app(<ReadingLanguageScope language="arabic" variety="arabic-egypt"><p data-testid="correction"><TargetText text={source} /></p></ReadingLanguageScope>)
    const word = screen.getByRole('button', {name:'القديمة'})
    const hover = new MouseEvent('pointerover', {bubbles:true})
    Object.defineProperty(hover, 'pointerType', {value:'mouse'})
    fireEvent(word, hover)
    await act(async () => { vi.advanceTimersByTime(350) })
    expect(screen.getByRole('group', {name:'Word help'})).toHaveTextContent('old')
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(view.getByTestId('correction').textContent).toBe(source)
    expect(screen.getByRole('button', {name:'القديمة'})).toBe(word)
    expect(word).toBeVisible()
    fireEvent.click(word)
    await act(async () => { vi.advanceTimersByTime(1000) })
    expect(word).toBeVisible()
    fireEvent.keyDown(document, {key:'Escape'})
    expect(screen.queryByRole('group', {name:'Word help'})).toBeNull()
    expect(view.getByTestId('correction').textContent).toBe(source)
    expect(word).toBeVisible()
    expect(services.read).toHaveBeenCalledOnce()
  } finally { vi.useRealTimers() }
})

it('does not request missing meanings for a brief pointer pass', async () => {
  vi.useFakeTimers()
  try {
    app(<TargetText text="Hola" />)
    const word = screen.getByRole('button', {name:'Hola'})
    const hover = new MouseEvent('pointerover', {bubbles:true})
    Object.defineProperty(hover, 'pointerType', {value:'mouse'})
    fireEvent(word, hover)
    await act(async () => { vi.advanceTimersByTime(100) })
    fireEvent.pointerOut(word)
    await act(async () => { vi.advanceTimersByTime(1000) })
    expect(services.read).not.toHaveBeenCalled()
    expect(screen.queryByRole('group', {name:'Word help'})).toBeNull()
    expect(word).toBeVisible()
  } finally { vi.useRealTimers() }
})

it('preserves a selected final Arabic word without turning its selection click into inspection', () => {
  const source = 'بتحب البيوت القديمة'
  app(<TargetText text={source} />)
  const word = screen.getByRole('button', {name:'القديمة'})
  const selected = window.getSelection()!
  const range = document.createRange()
  range.selectNodeContents(word)
  selected.removeAllRanges(); selected.addRange(range)
  try {
    fireEvent.click(word)
    expect(selected.toString()).toBe('القديمة')
    expect(word).toBeVisible()
    expect(services.read).not.toHaveBeenCalled()
    expect(screen.queryByRole('group', {name:'Word help'})).toBeNull()
  } finally { selected.removeAllRanges() }
})


it('shows a conversation word in another sentence synchronously without loading or AI', () => {
  app(<SavedReadingProvider sources={[{scope,text:'La playa.',segments:[{start:3,end:8,kind:'gloss',gloss:'beach',pronunciation:'pla-ya'}]}]}><TargetText text="Otra playa." /></SavedReadingProvider>)
  fireEvent.click(screen.getByRole('button',{name:'playa'}))
  expect(within(screen.getByRole('group', {name:'Word help'})).getByText('beach')).toBeVisible()
  expect(screen.queryByText('Finding word meanings…')).toBeNull()
  expect(services.read).not.toHaveBeenCalled()
})

it('uses the native saved lookup for a generated word in a different passage', async () => {
  vi.mocked(services.saved!).mockResolvedValueOnce(null).mockResolvedValue(result)
  app(<><TargetText text="Hola" /><TargetText text="Hola amigo" /></>)
  fireEvent.click(screen.getAllByRole('button',{name:'Hola'})[0])
  await waitFor(()=>expect(screen.getByText('hello')).toBeVisible())
  fireEvent.click(screen.getAllByRole('button',{name:'Hola'})[0])
  fireEvent.click(screen.getAllByRole('button',{name:'Hola'})[1])
  await waitFor(() => expect(screen.getByText('hello')).toBeVisible())
  expect(screen.queryByText('Finding word meanings…')).toBeNull()
  expect(services.read).toHaveBeenCalledOnce()
})


it('opens the deep inspector from the same durable cache without asking held AI access', async () => {
  vi.mocked(services.read).mockRejectedValue({code:'admission_held',message:'Daily limit'})
  function InspectSaved() { const actions = useReadingActions(); return <button onClick={() => actions?.inspect({scope,text:'playa',start:0,end:5})}>Inspect saved</button> }
  app(<SavedReadingProvider sources={[{scope,text:'La playa.',segments:[{start:3,end:8,kind:'gloss',gloss:'beach'}]}]}><InspectSaved /></SavedReadingProvider>)
  fireEvent.click(screen.getByRole('button',{name:'Inspect saved'}))
  await waitFor(() => expect(screen.getByRole('dialog',{name:'Word help'})).toBeVisible())
  fireEvent.click(screen.getByRole('button',{name:'playa'}))
  expect(within(screen.getByRole('group', {name:'Word help'})).getByText('beach')).toBeVisible()
  expect(services.read).not.toHaveBeenCalled()
})

it('delegates translations and word meanings as distinct local requests', async () => {
  const { useReadingLookup, useReadingPeek } = await import('./ReadingContext')
  let lookup: ReturnType<typeof useReadingLookup> = null
  let peek: ReturnType<typeof useReadingPeek> | null = null
  function Probe() { lookup = useReadingLookup(); peek = useReadingPeek(); return null }
  vi.mocked(services.read).mockImplementation(async input => input.aid === 'translation'
    ? { gloss: null, audioBase64: null, translation: 'Hello', receipt: { providerId: 'translation-1' } } as ReadingResult
    : result)
  app(<Probe />)
  const words = await lookup!({ ...scope, text: 'Hola', aid: 'word_gloss' }, new AbortController().signal)
  expect(words.gloss?.segments[0].gloss).toBe('hello')
  expect(peek!({ ...scope, text: 'Hola', aid: 'translation' })).toBeNull()
  const translated = await lookup!({ ...scope, text: 'Hola', aid: 'translation' }, new AbortController().signal)
  expect(translated.translation).toBe('Hello')
  await lookup!({ ...scope, text: 'Hola', aid: 'translation' }, new AbortController().signal)
  expect(vi.mocked(services.read).mock.calls.map(([input]) => input.aid)).toEqual(['word_gloss', 'translation', 'translation'])
  await expect(lookup!({ ...scope, text: 'Hola', aid: 'speech' }, new AbortController().signal)).rejects.toThrow('Read-aloud is requested through reading actions')
})

it('asks once when a word stays unresolved, and once more per explicit retry', async () => {
  // A recovered partial result that resolves nothing for this word.
  const unresolved = { gloss: { coverage: 'partial', segments: [{start:0,end:4,kind:'unresolved'}] }, audioBase64: null, translation: null, explanations: null, receipt: null } as unknown as ReadingResult
  vi.mocked(services.read).mockResolvedValue(unresolved)
  app(<TargetText text="Hola" />)
  fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
  await waitFor(() => expect(screen.getByRole('button', { name: 'Retry word meanings' })).toBeVisible())
  // Displaying a partial result must not ask again.
  await act(async () => { await Promise.resolve() })
  expect(services.read).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button', { name: 'Retry word meanings' }))
  await waitFor(() => expect(services.read).toHaveBeenCalledTimes(2))
  await act(async () => { await Promise.resolve() })
  expect(services.read).toHaveBeenCalledTimes(2)
})

it('keeps a partial meaning for the word it covers and does not re-ask after unrelated aid results', async () => {
  const { useReadingLookup } = await import('./ReadingContext')
  let lookup: ReturnType<typeof useReadingLookup> = null
  function Probe() { lookup = useReadingLookup(); return null }
  vi.mocked(services.read).mockImplementation(async input => (input.aid === 'word_gloss'
    ? { gloss: { coverage: 'partial', segments: [{start:0,end:4,kind:'gloss',gloss:'hello'},{start:5,end:9,kind:'unresolved'}] }, audioBase64: null, translation: null, explanations: null, receipt: null }
    : { gloss: null, audioBase64: null, translation: 'Hello house', explanations: null, receipt: null }) as unknown as ReadingResult)
  app(<><Probe /><TargetText text="Hola casa" /></>)
  // The word this partial result left unresolved.
  fireEvent.click(screen.getByRole('button', { name: 'casa' }))
  await waitFor(() => expect(screen.getByRole('button', { name: 'Retry word meanings' })).toBeVisible())
  expect(services.read).toHaveBeenCalledOnce()
  // An unrelated aid completes while the card is open.
  await act(async () => { await lookup!({ ...scope, text: 'Hola casa', aid: 'translation' }, new AbortController().signal) })
  expect(vi.mocked(services.read).mock.calls.map(([input]) => input.aid)).toEqual(['word_gloss', 'translation'])
  // The meanings that did come back are still shown for the word they cover.
  fireEvent.keyDown(screen.getByRole('button', { name: 'casa' }), { key: 'Escape' })
  fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
  await waitFor(() => expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('hello'))
  expect(vi.mocked(services.read).mock.calls.map(([input]) => input.aid)).toEqual(['word_gloss', 'translation', 'word_gloss'])
})

it('drops a pending result when the source text changes and asks for the new source', async () => {
  const pending: { input: { text: string }; resolve: (value: ReadingResult) => void }[] = []
  vi.mocked(services.read).mockImplementation(input => new Promise(resolve => { pending.push({ input, resolve }) }))
  function Switcher() {
    const [text, setText] = useState('Hola')
    return <><button onClick={() => setText('Casa')}>Change text</button><TargetText text={text} /></>
  }
  app(<Switcher />)
  fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
  await waitFor(() => expect(pending).toHaveLength(1))
  fireEvent.click(screen.getByRole('button', { name: 'Change text' }))
  await waitFor(() => expect(pending).toHaveLength(2))
  expect(pending.map(call => call.input.text)).toEqual(['Hola', 'Casa'])
  // The first source's answer arrives after its question was abandoned.
  await act(async () => { pending[0].resolve(result) })
  expect(screen.getByRole('group', { name: 'Word help' })).not.toHaveTextContent('hello')
  expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('Finding word meanings…')
  await act(async () => { pending[1].resolve({ ...result, gloss: { coverage: 'complete', segments: [{start:0,end:4,kind:'gloss',gloss:'house'}] } } as ReadingResult) })
  expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('house')
})

it('drops a pending result when the language scope changes and asks in the new scope', async () => {
  const pending: { input: { variety?: string | null }; resolve: (value: ReadingResult) => void; reject: (error: unknown) => void }[] = []
  vi.mocked(services.read).mockImplementation(input => new Promise((resolve, reject) => { pending.push({ input, resolve, reject }) }))
  function Switcher() {
    const [variety, setVariety] = useState('spanish-spain')
    return <><button onClick={() => setVariety('spanish-mexico')}>Change variety</button>
      <ReadingScopeContext value={{ ...scope, variety }}><TargetText text="Hola" /></ReadingScopeContext></>
  }
  app(<Switcher />)
  fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
  await waitFor(() => expect(pending).toHaveLength(1))
  fireEvent.click(screen.getByRole('button', { name: 'Change variety' }))
  await waitFor(() => expect(pending).toHaveLength(2))
  expect(pending.map(call => call.input.variety)).toEqual(['spanish-spain', 'spanish-mexico'])
  // Neither a late result nor a late failure may reach the new scope's card.
  await act(async () => { pending[0].reject(new Error('Word meanings failed')) })
  expect(screen.queryByRole('alert')).toBeNull()
  await act(async () => { pending[1].resolve(result) })
  expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('hello')
})

it('delegates grammar explanation reuse to native and keeps source inputs distinct', async () => {
  const { useReadingLookup, useReadingPeek } = await import('./ReadingContext')
  let lookup: ReturnType<typeof useReadingLookup> = null
  let peek: ReturnType<typeof useReadingPeek> | null = null
  function Probe() { lookup = useReadingLookup(); peek = useReadingPeek(); return null }
  const cards = { cards: [{ quote: 'Hola', title: 'Greeting', body: 'A greeting.', example: 'Hola, Ana.', contrast: '' }] }
  vi.mocked(services.read).mockImplementation(async input => ({ gloss: null, audioBase64: null, translation: null, explanations: input.aid === 'explanations' ? cards : null, receipt: null }) as ReadingResult)
  app(<Probe />)
  expect(peek!({ ...scope, text: 'Hola', aid: 'explanations' })).toBeNull()
  const first = await lookup!({ ...scope, text: 'Hola', aid: 'explanations' }, new AbortController().signal)
  expect(first.explanations).toEqual(cards)
  expect(peek!({ ...scope, text: 'Hola', aid: 'explanations' })).toBeNull()
  await lookup!({ ...scope, text: 'Hola', aid: 'explanations' }, new AbortController().signal)
  await lookup!({ ...scope, text: 'Hola.', aid: 'explanations' }, new AbortController().signal)
  expect(vi.mocked(services.read).mock.calls.map(([input]) => [input.aid, input.text])).toEqual([['explanations', 'Hola'], ['explanations', 'Hola'], ['explanations', 'Hola.']])
})

it('a partial cached or saved result still lets Words request the whole passage, once', async () => {
  const { TargetMessage } = await import('./TargetMessage')
  vi.mocked(services.read).mockResolvedValue({ gloss: { coverage: 'complete', segments: [{start:0,end:4,kind:'gloss',gloss:'hello'},{start:5,end:9,kind:'gloss',gloss:'house'}] }, audioBase64: null, translation: null, receipt: null } as ReadingResult)
  const props = { text: 'Hola casa', segments: [], segmentsKey: 'hola-casa', translation: null, romanization: null, pronunciation: null, layout: 'passage' as const, translateLabel: null,
    segmentsPending: false, lookupWords: true, status: null, annotation: null, speech: null, analysis: null, focused: false, rtl: false }
  // Only "Hola" is known from a durable source: the provider's peek reports partial coverage.
  app(<SavedReadingProvider sources={[{ scope, text: 'Hola casa', segments: [{start:0,end:4,kind:'gloss',gloss:'hello'}] }]}><TargetMessage provenance={null} {...props} /></SavedReadingProvider>)
  openMenus()
  await waitFor(() => expect(screen.getByRole('button', { name: 'Words' })).toBeEnabled())
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Words' }))
  await waitFor(() => expect(services.read).toHaveBeenCalledOnce())
  expect(services.read).toHaveBeenCalledWith({ ...scope, text: 'Hola casa', aid: 'word_gloss' }, expect.any(AbortSignal))
  await waitFor(() => expect(screen.getByText('house')).toBeVisible())
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Words' }))
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Words' }))
  expect(services.read).toHaveBeenCalledOnce()
})

it('each passage requests generated help explicitly, and Chat-style owners never look up', async () => {
  const { TargetMessage } = await import('./TargetMessage')
  const props = { text: 'Hola', segments: [], segmentsKey: 'hola', translation: null, romanization: null, pronunciation: null, layout: 'passage' as const, translateLabel: null,
    segmentsPending: false, lookupWords: true, status: null, annotation: null, speech: null, analysis: null, focused: false, rtl: false }
  const view = app(<><TargetMessage provenance={null} {...props} /><TargetMessage provenance={null} {...props} segmentsKey="second" /></>)
  openMenus()
  fireEvent.click(screen.getAllByRole('button', { name: 'Words' })[0])
  await waitFor(() => expect(services.read).toHaveBeenCalledOnce())
  // A different surface has no generated inference state of its own until requested.
  await waitFor(() => expect(view.container.querySelectorAll('.wg')).toHaveLength(1))
  openMenus()
  fireEvent.click(screen.getAllByRole('button', { name: 'Words' })[1])
  await waitFor(() => expect(view.container.querySelectorAll('.wg')).toHaveLength(2))
  expect(services.read).toHaveBeenCalledTimes(2)
  view.unmount()
  app(<TargetMessage provenance={null} {...props} layout="bubble" lookupWords={false} />)
  openMenus()
  expect(screen.getByRole('button', { name: 'Words' })).toBeDisabled()
  expect(screen.queryByRole('button', { name: 'Translate' })).toBeNull()
  expect(services.read).toHaveBeenCalledTimes(2)
})


it('keeps word help and speech status usable without WebView popover methods', async () => {
  const show = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'showPopover')
  const hide = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'hidePopover')
  Reflect.deleteProperty(HTMLElement.prototype, 'showPopover')
  Reflect.deleteProperty(HTMLElement.prototype, 'hidePopover')
  const media = vi.spyOn(window, 'matchMedia').mockReturnValue({ matches: false } as MediaQueryList)
  function Speak() {
    const actions = useReadingActions()
    return <button onClick={() => actions?.speak({ scope, text: 'Hola', start: 0, end: 4 })}>Speak</button>
  }
  let view: ReturnType<typeof render> | undefined
  try {
    view = app(<><TargetText text="Hola" /><SavedGlossText text="casa" segments={[{ start: 0, end: 4, kind: 'gloss', gloss: 'house' }]} /><Speak /></>)
    fireEvent.click(screen.getByRole('button', { name: 'Hola' }))
    const help = await screen.findByRole('group', { name: 'Word help' })
    await waitFor(() => expect(help).toHaveTextContent('hello'))
    expect(help.parentElement).toBe(document.body)
    expect(help).not.toHaveAttribute('popover')
    fireEvent.pointerDown(document.body)
    fireEvent.click(screen.getByRole('button', { name: 'casa' }))
    const saved = screen.getByRole('group', { name: 'Word help' })
    expect(saved.parentElement).toBe(document.body)
    expect(saved).toHaveTextContent('house')
    expect(saved).not.toHaveAttribute('popover')
    fireEvent.pointerDown(document.body)
    vi.mocked(services.speak).mockRejectedValue(new Error('Speech unavailable'))
    fireEvent.click(screen.getByRole('button', { name: 'Speak' }))
    await screen.findByText('Speech unavailable')
    const status = document.querySelector('.reading-audio-status')!
    expect(status.parentElement).toBe(document.body)
    expect(status).not.toHaveAttribute('popover')
    expect(screen.queryByRole('dialog')).toBeNull()
  } finally {
    view?.unmount()
    media.mockRestore()
    if (show) Object.defineProperty(HTMLElement.prototype, 'showPopover', show)
    if (hide) Object.defineProperty(HTMLElement.prototype, 'hidePopover', hide)
  }
})

it('shows word meanings in the inspector without auto-translate and carries no selects, receipts or history', async () => {
  function Controls() {
    const actions = useReadingActions()
    return <button onClick={() => actions?.inspect({scope, text:'Hola', start:0, end:4})}>Inspect</button>
  }
  render(<ReadingPreferencesContext value={{autoTranslate:false, alwaysRomanize:false, alwaysPronunciation:false}}><ReadingScopeContext value={scope}><ReadingHelp services={services} languages={languages}><Controls /></ReadingHelp></ReadingScopeContext></ReadingPreferencesContext>)
  fireEvent.click(screen.getByRole('button', {name:'Inspect'}))
  const dialog = await screen.findByRole('dialog', {name:'Word help'})
  await waitFor(() => expect(within(dialog).getByText('hello')).toBeVisible())
  expect(within(dialog).queryAllByRole('combobox')).toHaveLength(0)
  expect(within(dialog).queryByRole('button', {name:'Reading request history'})).toBeNull()
  expect(within(dialog).queryByText('Response details')).toBeNull()
  expect(within(dialog).queryByRole('button', {name:'Start a conversation from this phrase'})).toBeNull()
  expect(services.activity).not.toHaveBeenCalled()
})


it('opens inline target words by touch or keyboard, pins help, and dismisses outside without changing source', async () => {
  const view = app(<Markdown text={'Use '+String.fromCharCode(96)+'Hola'+String.fromCharCode(96)+' here.'} targetCode />)
  const word = screen.getByRole('button', { name: 'Hola' })
  const source = view.container.querySelector('.target-inline')!
  const touch = new MouseEvent('pointerover', { bubbles: true })
  Object.defineProperty(touch, 'pointerType', { value: 'touch' })
  fireEvent(word, touch)
  expect(services.read).not.toHaveBeenCalled()
  fireEvent.click(word)
  expect(await screen.findByRole('group', { name: 'Word help' })).toBeVisible()
  await waitFor(() => expect(screen.getByRole('group', { name: 'Word help' })).toHaveTextContent('hello'))
  fireEvent.pointerOut(word)
  expect(screen.getByRole('group', { name: 'Word help' })).toBeVisible()
  expect(source.textContent).toBe('Hola')
  fireEvent.pointerDown(document.body)
  expect(screen.queryByRole('group', { name: 'Word help' })).toBeNull()
  fireEvent.keyDown(word, { key: 'Enter' })
  expect(screen.getByRole('group', { name: 'Word help' })).toBeVisible()
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('group', { name: 'Word help' })).toBeNull()
  expect(source.textContent).toBe('Hola')
})
