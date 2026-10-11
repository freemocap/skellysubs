// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { TargetMessage, type TargetMessageProps } from './TargetMessage'
import { ReadingPreferencesContext } from './ReadingPreferences'
import { ReadingLookupContext, ReadingScopeContext } from './ReadingContext'
import type { ReadingResult } from '../../generated/contracts'

/** Opens every message's ⋯ menu, where Words, Analysis and Pronunciation live. */
const openMenus = () => screen.queryAllByRole('button', { name: 'More actions' }).forEach(button => { if (button.getAttribute('aria-expanded') !== 'true') fireEvent.click(button) })


vi.mock('../../platform/ipc/tauri', () => ({languageFor: () => ({languageTag:'es'})}))

/// A standalone passage: no saved meanings, lookup allowed, no owner actions.
function passage(overrides: Partial<TargetMessageProps> & Pick<TargetMessageProps, 'text'>): TargetMessageProps {
  return { segments: [], segmentsKey: overrides.text, translation: null, romanization: null, pronunciation: null, layout: 'passage', translateLabel: null,
    segmentsPending: false, lookupWords: true, status: null, annotation: null, speech: null, analysis: null, focused: false, rtl: false, provenance: null, ...overrides }
}

it('uses durable owner requests for missing aids without a generic lookup or a mount request', async () => {
  const words = vi.fn().mockResolvedValue(undefined), translation = vi.fn().mockResolvedValue(undefined)
  const lookup = vi.fn()
  render(<ReadingLookupContext value={lookup}><TargetMessage {...passage({ text: 'Hola', lookupWords: false, onRequestWords: words, onRequestTranslation: translation })} /></ReadingLookupContext>)
  expect(words).not.toHaveBeenCalled(); expect(translation).not.toHaveBeenCalled()
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Words' }))
  await waitFor(() => expect(words).toHaveBeenCalledOnce())
  fireEvent.click(screen.getByRole('button', { name: 'Translate' }))
  await waitFor(() => expect(translation).toHaveBeenCalledOnce())
  expect(lookup).not.toHaveBeenCalled()
})

it('shows requested missing word help on the first click with always-visible aids off', async () => {
  const read = vi.fn().mockResolvedValue({gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'hello',romanization:'roman',pronunciation:'redundant'}]}} as ReadingResult)
  const source = (enabled:boolean) => <ReadingPreferencesContext value={{autoTranslate:enabled,alwaysRomanize:enabled,alwaysPronunciation:enabled}}>
    <ReadingScopeContext value={{language:'spanish',variety:'spain',explanation:'english',explanationVariety:'american'}}>
      <ReadingLookupContext value={read}><TargetMessage {...passage({text:"Hola", romanization:"sentence roman"})} /></ReadingLookupContext>
    </ReadingScopeContext>
  </ReadingPreferencesContext>
  const view = render(source(false))
  expect(read).not.toHaveBeenCalled()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(read).toHaveBeenCalledTimes(1))
  openMenus()
  await waitFor(() => expect(screen.getByRole('button',{name:'Words'})).toBeEnabled())
  expect(view.container.querySelector('.wg')).toHaveTextContent('hello')
  expect(view.container.querySelector('.wroman')).toHaveTextContent('roman')
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  expect(view.container.querySelector('.wg,.wroman,.wpronunciation')).toBeNull()
  view.rerender(source(true))
  expect(screen.getByText('hello')).toBeVisible()
  expect(screen.getByText('roman')).toBeVisible()
  expect(screen.queryByText('sentence roman')).toBeNull()
  expect(screen.queryByText('redundant')).toBeNull()
  expect(read).toHaveBeenCalledTimes(1)
})

it('reveals saved word aids on the first click with all defaults off', () => {
  const view = render(<ReadingPreferencesContext value={{autoTranslate:false,alwaysRomanize:false,alwaysPronunciation:false}}>
    <TargetMessage {...passage({text:"Hola casa", segments:[
      {start:0,end:4,kind:'gloss',gloss:'hello',romanization:'roman',pronunciation:'redundant'},
      {start:5,end:9,kind:'gloss',gloss:'house',pronunciation:'fallback'},
    ]})} />
  </ReadingPreferencesContext>)
  expect(view.container.querySelector('.wg,.wroman,.wpronunciation')).toBeNull()
  openMenus()
  const button = screen.getByRole('button',{name:'Words'})
  fireEvent.click(button)
  expect(button).toHaveAttribute('aria-expanded','true')
  expect(screen.getByText('hello')).toBeVisible()
  expect(screen.getByText('roman')).toBeVisible()
  expect(screen.getByText('fallback')).toBeVisible()
  expect(screen.queryByText('redundant')).toBeNull()
  fireEvent.click(button)
  expect(button).toHaveAttribute('aria-expanded','false')
  expect(view.container.querySelector('.wg,.wroman,.wpronunciation')).toBeNull()
})

/// A source that is not a conversation turn: the same tools, supplied by a
/// different owner. This proves presentation wiring and the shared reading
/// lookup; it does not exercise native aid execution.
const item = 'ممكن تحكي شوي شوي؟'
const itemSegments = [
  {start:0,end:4,kind:'gloss' as const,gloss:'can',romanization:'mumkin'},
  {start:5,end:9,kind:'gloss' as const,gloss:'you speak',romanization:'tiḥki'},
]

it('gives a non-conversation source every message tool with its own actions', () => {
  const onToggle = vi.fn(), onOpen = vi.fn()
  const view = render(<ReadingPreferencesContext value={{autoTranslate:false,alwaysRomanize:false,alwaysPronunciation:false,supportsRomanization:true}}>
    <TargetMessage {...passage({text:item, layout:'bubble', segments:itemSegments, translation:'Can you speak slowly?', romanization:'mumkin tiḥki shwayy shwayy?',
      status:<p role="status">Owner status</p>, speech:{speaking:false, onToggle, error:null}, analysis:{pending:false, onOpen}, rtl:true, lookupWords:false})} />
  </ReadingPreferencesContext>)
  const bubble = view.container.querySelector('.msg.chat-message.bot.with-actions.rtl')
  expect(bubble).not.toBeNull()
  expect(screen.getByRole('status')).toHaveTextContent('Owner status')
  expect(screen.queryByText('Can you speak slowly?')).toBeNull()
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  expect(screen.getByText('Can you speak slowly?')).toBeVisible()
  expect(view.container.querySelector('.wg')).toBeNull()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  expect(screen.getByText('you speak')).toBeVisible()
  expect(screen.getByText('tiḥki')).toBeVisible()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Pronunciation'}))
  expect(screen.getByText('mumkin tiḥki shwayy shwayy?')).toBeVisible()
  fireEvent.click(screen.getByRole('button',{name:'Play'}))
  expect(onToggle).toHaveBeenCalledOnce()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Analysis'}))
  expect(onOpen).toHaveBeenCalledOnce()
})

it('shows owner progress, playback state and failures without owner-specific state', async () => {
  const read = vi.fn().mockRejectedValue(new Error('Lookup refused'))
  const props = passage({text:item, layout:'bubble', segmentsPending:true, translation:'Can you speak slowly?',
    speech:{speaking:true, onToggle:vi.fn(), error:{text:'Speech failed', details:{code:'x'}}}, analysis:{pending:true, onOpen:vi.fn()}})
  const view = render(<ReadingPreferencesContext value={{autoTranslate:true,alwaysRomanize:false,alwaysPronunciation:false}}>
    <ReadingScopeContext value={{language:'arabic',variety:'arabic-levantine',explanation:'english',explanationVariety:'american'}}>
      <ReadingLookupContext value={read}><TargetMessage {...props} /></ReadingLookupContext>
    </ReadingScopeContext>
  </ReadingPreferencesContext>)
  expect(screen.getByText('Can you speak slowly?')).toBeVisible()
  expect(screen.getByRole('button',{name:'Stop playback'})).toBeVisible()
  expect(screen.getByText('Speech failed')).toBeVisible()
  openMenus()
  expect(screen.getByRole('button',{name:'Analysis'})).toHaveClass('is-hydrating')
  openMenus()
  const words = screen.getByRole('button',{name:'Words'})
  expect(words).toHaveClass('is-hydrating')
  fireEvent.click(words)
  await waitFor(() => expect(read).toHaveBeenCalledOnce())
  await waitFor(() => expect(view.container).toHaveTextContent('Lookup refused'))
})

it('keeps an explicit translation choice when the reading preference changes', () => {
  const source = (autoTranslate: boolean) => <ReadingPreferencesContext value={{autoTranslate,alwaysRomanize:false,alwaysPronunciation:false}}>
    <TargetMessage {...passage({text:item, translation:'Can you speak slowly?', lookupWords:false})} />
  </ReadingPreferencesContext>
  const view = render(source(false))
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  view.rerender(source(true))
  expect(screen.getByText('Can you speak slowly?')).toBeVisible()
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  expect(screen.queryByText('Can you speak slowly?')).toBeNull()
  openMenus()
  expect(screen.getByRole('button',{name:'Words'})).toBeDisabled()
})

const spain = {language:'spanish',variety:'spain',explanation:'english',explanationVariety:'american'}
const mexico = {language:'spanish',variety:'mexico',explanation:'english',explanationVariety:'american'}
const quiet = {autoTranslate:false,alwaysRomanize:false,alwaysPronunciation:false}
function deferred() {
  let resolve!: (value: ReadingResult) => void
  const promise = new Promise<ReadingResult>(done => { resolve = done })
  return { promise, resolve }
}
function lookupSource(read: ReadingServicesRead, scope: typeof spain, props: TargetMessageProps) {
  return <ReadingPreferencesContext value={quiet}><ReadingScopeContext value={scope}>
    <ReadingLookupContext value={read}><TargetMessage {...props} /></ReadingLookupContext>
  </ReadingScopeContext></ReadingPreferencesContext>
}
type ReadingServicesRead = (input: unknown, signal: AbortSignal) => Promise<ReadingResult>

it('bulk aids request once, hide independently, and retain individual controls', async () => {
  const read = vi.fn<ReadingServicesRead>().mockImplementation(async input => (input as {aid:string}).aid === 'translation'
    ? {translation:'Hello',gloss:null} as ReadingResult
    : {gloss:{coverage:'complete',segments:[{start:0,end:4,kind:'gloss',gloss:'meaning',romanization:'roman',pronunciation:'fallback'}]}} as ReadingResult)
  const source = (translations:boolean|null, phonetics:boolean|null) => lookupSource(read, spain, passage({text:'Hola', bulkReading:{translations,phonetics}}))
  const view = render(source(null, null))
  expect(read).not.toHaveBeenCalled()
  view.rerender(source(true, true))
  await waitFor(() => expect(screen.getByText('Hello')).toBeVisible())
  expect(await screen.findByText('roman')).toBeVisible()
  expect(screen.queryByText('meaning')).toBeNull()
  expect(screen.queryByText('fallback')).toBeNull()
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  expect(screen.queryByText('Hello')).toBeNull()
  view.rerender(source(true, true))
  expect(screen.queryByText('Hello')).toBeNull()
  view.rerender(source(false, false))
  expect(screen.queryByText('roman')).toBeNull()
  view.rerender(source(true, true))
  expect(screen.getByText('Hello')).toBeVisible()
  expect(screen.getByText('roman')).toBeVisible()
  expect(read).toHaveBeenCalledTimes(2)
})

it('bulk phonetics reuse complete saved aids without requesting them again', () => {
  const read = vi.fn<ReadingServicesRead>()
  render(lookupSource(read, spain, passage({text:'Hola',segments:[{start:0,end:4,kind:'gloss',gloss:'hello',romanization:'oh-la'}],
    bulkReading:{translations:null,phonetics:true}})))
  expect(screen.getByText('oh-la')).toBeVisible()
  expect(read).not.toHaveBeenCalled()
})

it('bulk cancellation and source changes abort old work and ignore late results', async () => {
  const pending: {signal:AbortSignal; resolve:(value:ReadingResult)=>void}[] = []
  const read: ReadingServicesRead = (_, signal) => new Promise(resolve => pending.push({signal,resolve}))
  const source = (text:string, visible:boolean) => lookupSource(read, spain, passage({text,bulkReading:{translations:visible,phonetics:null}}))
  const view = render(source('Hola', true))
  await waitFor(() => expect(pending).toHaveLength(1))
  view.rerender(source('Adiós', true))
  await waitFor(() => expect(pending).toHaveLength(2))
  expect(pending[0].signal.aborted).toBe(true)
  view.rerender(source('Adiós', false))
  expect(pending[1].signal.aborted).toBe(true)
  await act(async () => pending.forEach(job => job.resolve({translation:'Stale'} as ReadingResult)))
  expect(screen.queryByText('Stale')).toBeNull()
  view.rerender(source('Adiós', true))
  await waitFor(() => expect(pending).toHaveLength(3))
  view.unmount()
  expect(pending[2].signal.aborted).toBe(true)
})

it('uses pronunciation for a language without romanization and retries missing aids explicitly', async () => {
  const read = vi.fn<ReadingServicesRead>().mockResolvedValueOnce({gloss:{segments:[]}} as unknown as ReadingResult)
    .mockResolvedValueOnce({gloss:{coverage:'complete',segments:[{start:0,end:4,kind:'gloss',gloss:'hello',pronunciation:'oh-la'}]}} as ReadingResult)
  render(<ReadingPreferencesContext value={{...quiet,supportsRomanization:false}}><ReadingScopeContext value={spain}>
    <ReadingLookupContext value={read}><TargetMessage {...passage({text:'Hola',bulkReading:{translations:null,phonetics:true}})} /></ReadingLookupContext>
  </ReadingScopeContext></ReadingPreferencesContext>)
  await screen.findByText('The reading service returned no pronunciation or romanization.')
  expect(read).toHaveBeenCalledOnce()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Pronunciation'}))
  expect(await screen.findByText('oh-la')).toBeVisible()
  expect(read).toHaveBeenCalledTimes(2)
})

it('requests whole-passage meanings when saved meanings cover only part of the text', async () => {
  const read = vi.fn<ReadingServicesRead>().mockResolvedValue({gloss:{segments:[
    {start:0,end:4,kind:'gloss',gloss:'hello'},{start:5,end:9,kind:'gloss',gloss:'house'},
  ]}} as ReadingResult)
  const view = render(lookupSource(read, spain, passage({text:'Hola casa', segments:[{start:0,end:4,kind:'gloss',gloss:'hello'}]})))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(read).toHaveBeenCalledOnce())
  await waitFor(() => expect(screen.getByText('house')).toBeVisible())
  expect(screen.getByText('hello')).toBeVisible()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  expect(read).toHaveBeenCalledOnce()
  view.unmount()
})

it('never looks meanings up for an owner that does not allow lookup, even with partial meanings', () => {
  const read = vi.fn<ReadingServicesRead>()
  render(lookupSource(read, spain, passage({text:'Hola casa', layout:'bubble', lookupWords:false, segments:[{start:0,end:4,kind:'gloss',gloss:'hello'}]})))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  expect(read).not.toHaveBeenCalled()
  expect(screen.getByText('hello')).toBeVisible()
})

it('cancels a pending lookup when the text changes and ignores its late result', async () => {
  const first = deferred()
  const signals: AbortSignal[] = []
  const read = vi.fn<ReadingServicesRead>().mockImplementation((_, signal) => { signals.push(signal); return first.promise })
  const view = render(lookupSource(read, spain, passage({text:'Hola', segmentsKey:'a'})))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(read).toHaveBeenCalledOnce())
  openMenus()
  expect(screen.getByRole('button',{name:'Words'})).toBeDisabled()
  view.rerender(lookupSource(read, spain, passage({text:'Adiós', segmentsKey:'b'})))
  expect(signals[0].aborted).toBe(true)
  openMenus()
  expect(screen.getByRole('button',{name:'Words'})).toBeEnabled()
  first.resolve({gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'hello'}]}} as ReadingResult)
  await Promise.resolve()
  expect(screen.queryByText('hello')).toBeNull()
  expect(view.container).toHaveTextContent('Adiós')
})

it('does not show a completed lookup on different text', async () => {
  const read = vi.fn<ReadingServicesRead>().mockImplementation(async input => (input as {text:string}).text === 'Hola'
    ? {gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'hello'}]}} as ReadingResult
    : {gloss:{segments:[{start:0,end:5,kind:'gloss',gloss:'goodbye'}]}} as ReadingResult)
  const view = render(lookupSource(read, spain, passage({text:'Hola', segmentsKey:'a'})))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(screen.getByText('hello')).toBeVisible())
  view.rerender(lookupSource(read, spain, passage({text:'Adiós', segmentsKey:'b'})))
  expect(screen.queryByText('hello')).toBeNull()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(screen.getByText('goodbye')).toBeVisible())
  expect(read).toHaveBeenCalledTimes(2)
})

it('scopes lookups to the reading language, including identical text', async () => {
  const pending = deferred()
  const signals: AbortSignal[] = []
  const read = vi.fn<ReadingServicesRead>()
    .mockResolvedValueOnce({gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'hello (Spain)'}]}} as ReadingResult)
    .mockImplementationOnce((_, signal) => { signals.push(signal); return pending.promise })
  const props = passage({text:'Hola', segmentsKey:'same'})
  const view = render(lookupSource(read, spain, props))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(screen.getByText('hello (Spain)')).toBeVisible())
  view.rerender(lookupSource(read, mexico, props))
  expect(screen.queryByText('hello (Spain)')).toBeNull()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Words'}))
  await waitFor(() => expect(read).toHaveBeenCalledTimes(2))
  expect(read.mock.calls[1][0]).toMatchObject({variety:'mexico', text:'Hola'})
  view.rerender(lookupSource(read, spain, props))
  expect(signals[0].aborted).toBe(true)
  pending.resolve({gloss:{segments:[{start:0,end:4,kind:'gloss',gloss:'hello (Mexico)'}]}} as ReadingResult)
  await Promise.resolve()
  expect(screen.queryByText('hello (Mexico)')).toBeNull()
})

it('translates a non-conversation source on request through the reading lookup, once', async () => {
  const read = vi.fn<ReadingServicesRead>().mockResolvedValue({gloss:null,audioBase64:null,translation:'Can you speak slowly?',receipt:null} as ReadingResult)
  render(lookupSource(read, spain, passage({text:'¿Puedes hablar despacio?'})))
  const translate = screen.getByRole('button',{name:'Translate'})
  expect(translate).toHaveAttribute('aria-pressed','false')
  fireEvent.click(translate)
  await waitFor(() => expect(screen.getByText('Can you speak slowly?')).toBeVisible())
  expect(read).toHaveBeenCalledOnce()
  expect(read.mock.calls[0][0]).toMatchObject({aid:'translation', text:'¿Puedes hablar despacio?', variety:'spain'})
  fireEvent.click(translate)
  expect(screen.queryByText('Can you speak slowly?')).toBeNull()
  fireEvent.click(translate)
  expect(screen.getByText('Can you speak slowly?')).toBeVisible()
  expect(read).toHaveBeenCalledOnce()
})

it('never offers a lookup translation to an owner that does not allow lookup', () => {
  const read = vi.fn<ReadingServicesRead>()
  render(lookupSource(read, spain, passage({text:'Hola', layout:'bubble', lookupWords:false})))
  expect(screen.queryByRole('button',{name:'Translate'})).toBeNull()
  expect(read).not.toHaveBeenCalled()
})

it('shows a failed translation request and drops a late translation for an earlier source', async () => {
  const first = deferred()
  const read = vi.fn<ReadingServicesRead>()
    .mockImplementationOnce(() => first.promise)
    .mockRejectedValueOnce(new Error('Translation refused'))
  const view = render(lookupSource(read, spain, passage({text:'Hola', segmentsKey:'a'})))
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  await waitFor(() => expect(read).toHaveBeenCalledOnce())
  expect(screen.getByRole('button',{name:'Translate'})).toHaveClass('is-hydrating')
  view.rerender(lookupSource(read, spain, passage({text:'Adiós', segmentsKey:'b'})))
  first.resolve({gloss:null,audioBase64:null,translation:'Hello',receipt:null} as ReadingResult)
  await Promise.resolve()
  expect(screen.queryByText('Hello')).toBeNull()
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  await waitFor(() => expect(view.container).toHaveTextContent('Translation refused'))
})

function translatingSource(read: ReadingServicesRead, props: TargetMessageProps) {
  return <ReadingPreferencesContext value={{autoTranslate:true,alwaysRomanize:false,alwaysPronunciation:false}}><ReadingScopeContext value={spain}>
    <ReadingLookupContext value={read}><TargetMessage {...props} /></ReadingLookupContext>
  </ReadingScopeContext></ReadingPreferencesContext>
}

it('fetches and shows a missing translation on one click with automatic translation on, without requesting on mount', async () => {
  const read = vi.fn<ReadingServicesRead>().mockResolvedValue({gloss:null,audioBase64:null,translation:'Hello',receipt:null} as ReadingResult)
  render(translatingSource(read, passage({text:'Hola'})))
  expect(read).not.toHaveBeenCalled()
  const translate = screen.getByRole('button',{name:'Translate'})
  expect(translate).toHaveAttribute('aria-pressed','false')
  fireEvent.click(translate)
  await waitFor(() => expect(screen.getByText('Hello')).toBeVisible())
  expect(translate).toHaveAttribute('aria-pressed','true')
  expect(read).toHaveBeenCalledOnce()
  fireEvent.click(translate)
  expect(screen.queryByText('Hello')).toBeNull()
})

it('with automatic translation on, a new source needs its own click and does not reuse the old translation', async () => {
  const read = vi.fn<ReadingServicesRead>().mockImplementation(async input => ({gloss:null,audioBase64:null,translation:(input as {text:string}).text === 'Hola' ? 'Hello' : 'Goodbye',receipt:null} as ReadingResult))
  const view = render(translatingSource(read, passage({text:'Hola', segmentsKey:'a'})))
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  await waitFor(() => expect(screen.getByText('Hello')).toBeVisible())
  view.rerender(translatingSource(read, passage({text:'Adiós', segmentsKey:'b'})))
  expect(screen.queryByText('Hello')).toBeNull()
  expect(read).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  await waitFor(() => expect(screen.getByText('Goodbye')).toBeVisible())
  expect(read).toHaveBeenCalledTimes(2)
})

it('with automatic translation on, a failed request is retried by one more click', async () => {
  const read = vi.fn<ReadingServicesRead>()
    .mockRejectedValueOnce(new Error('Translation refused'))
    .mockResolvedValueOnce({gloss:null,audioBase64:null,translation:'Hello',receipt:null} as ReadingResult)
  const view = render(translatingSource(read, passage({text:'Hola'})))
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  await waitFor(() => expect(view.container).toHaveTextContent('Translation refused'))
  expect(screen.getByRole('button',{name:'Translate'})).toHaveAttribute('aria-pressed','false')
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  await waitFor(() => expect(screen.getByText('Hello')).toBeVisible())
  expect(view.container).not.toHaveTextContent('Translation refused')
  expect(read).toHaveBeenCalledTimes(2)
})

it('puts an owner-supplied practice action in place of the shared Add to Practice button', () => {
  render(<ReadingScopeContext value={{language:'spanish',variety:'spain',explanation:'english',explanationVariety:'american'}}>
    <TargetMessage {...passage({text:'Hola', practiceAction: <button type="button">Keep</button>})} />
  </ReadingScopeContext>)
  expect(screen.getByRole('button',{name:'Keep'})).toBeVisible()
  expect(screen.queryByRole('button',{name:'Add to Practice'})).toBeNull()
})


it('tags the rendered text and translation independently for graph surface hints', () => {
  const view = render(<ReadingPreferencesContext value={{ autoTranslate: true, alwaysRomanize: false, alwaysPronunciation: false }}><TargetMessage {...passage({ text: 'Hola', translation: 'Hello', graphTargets: { text: { 'data-graph-results': 'answer' }, translation: { 'data-graph-results': 'translated' } } })} /></ReadingPreferencesContext>)
  expect(view.container.querySelector('[data-graph-results="answer"]')).toHaveTextContent('Hola')
  expect(view.container.querySelector('[data-graph-results="translated"]')).toHaveTextContent('Hello')
})
