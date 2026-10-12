// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'
import { ReplyHelp } from './ReplyHelp'
import { SavedReadingProvider } from '../../../components/reading/SavedReadingProvider'
import { ReadingScopeContext } from '../../../components/reading/ReadingContext'
import { ReadingProvider } from '../../../components/reading/TargetText'
import { replyHelpFixture } from './ReplyHelp.fixtures'

/** Opens every message's ⋯ menu, where Words, Analysis and Pronunciation live. */
const openMenus = () => screen.queryAllByRole('button', { name: 'More actions' }).forEach(button => { if (button.getAttribute('aria-expanded') !== 'true') fireEvent.click(button) })


const backend = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('../../../platform/ipc/tauri', () => ({...backend,languageFor:() => ({languageTag:'zh',romanization:'pinyin'})}))
beforeEach(() => { backend.invoke.mockReset() })

const annotatedReplies = replyHelpFixture.saved
const { replies, grammar } = replyHelpFixture
const base = { brief: replyHelpFixture.brief, busy: false, errors: [] as string[], onUse: () => {} }

it.each([false, true])('requests a missing brief once on deliberate open (inline=%s)', inline => {
  const request = vi.fn().mockResolvedValue(undefined)
  render(<ReplyHelp {...base} brief={undefined} inline={inline} onRequestBrief={request} />)
  expect(request).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: inline ? 'Help understanding this message' : 'Help with this reply' }))
  expect(request).toHaveBeenCalledOnce()
  if (inline) {
    fireEvent.click(screen.getByRole('button', { name: 'Help understanding this message' }))
    fireEvent.click(screen.getByRole('button', { name: 'Help understanding this message' }))
    expect(request).toHaveBeenCalledOnce()
  }
})

it('exposes both coach actions immediately without generating on mount', () => {
  const explain = vi.fn().mockResolvedValue(undefined)
  const suggest = vi.fn().mockResolvedValue(undefined)
  render(<ReplyHelp {...base} inline onExplainGrammar={explain} onSuggestReply={suggest} />)
  expect(screen.queryByRole('button', { name: 'Help with this reply' })).toBeNull()
  expect(screen.getByRole('button', { name: 'Explain grammar' })).toBeVisible()
  expect(screen.getByRole('button', { name: 'Suggest a reply' })).toBeVisible()
  expect(explain).not.toHaveBeenCalled()
  expect(suggest).not.toHaveBeenCalled()
  expect(screen.queryByText(/She asked how you are/)).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Suggest a reply' }))
  expect(suggest).toHaveBeenCalledOnce()
  expect(explain).not.toHaveBeenCalled()
  expect(screen.queryByText(/She asked how you are/)).toBeNull()
})

it('reveals the hint only on request, independently of grammar and reply suggestions', () => {
  const view = render(wrap({ inline: true, brief: undefined, briefPending: true, grammar, replies }))
  const hint = screen.getByRole('button', { name: 'Help understanding this message' })
  expect(hint).toHaveAttribute('aria-expanded', 'false')
  expect(screen.queryByText('Reading the conversation…')).toBeNull()
  view.rerender(wrap({ inline: true, grammar, replies }))
  expect(screen.queryByText(/She asked how you are/)).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'Explain grammar' }))
  expect(screen.getByRole('heading', { name: 'Yes-no questions with 吗' })).toBeVisible()
  expect(screen.queryByText(/She asked how you are/)).toBeNull()
  fireEvent.click(hint)
  expect(hint).toHaveAttribute('aria-expanded', 'true')
  expect(screen.getByText(/She asked how you are/)).toBeVisible()
  fireEvent.click(hint)
  expect(screen.queryByText(/She asked how you are/)).toBeNull()
  expect(screen.getByRole('heading', { name: 'Yes-no questions with 吗' })).toBeVisible()
  expect(screen.getByRole('button', { name: 'Suggest a reply' })).toBeVisible()
  expect(backend.invoke).not.toHaveBeenCalled()
})

function wrap(props: Partial<Parameters<typeof ReplyHelp>[0]>) {
  const scope = {language:'mandarin',variety:null,explanation:'english',explanationVariety:null}
  return <ReadingProvider settings={null}><ReadingScopeContext value={scope}><SavedReadingProvider sources={annotatedReplies.map(reply => ({...reply,scope}))}><ReplyHelp {...base} {...props} /></SavedReadingProvider></ReadingScopeContext></ReadingProvider>
}

function show(props: Partial<Parameters<typeof ReplyHelp>[0]> = {}) {
  const view = render(wrap(props))
  const folded = screen.queryByRole('button', { name: 'Help with this reply' })
  if (folded) fireEvent.click(folded)
  return view
}

it('opens to the brief alone on demand and computes neither request', () => {
  const onExplainGrammar = vi.fn(), onSuggestReply = vi.fn()
  show({ onExplainGrammar, onSuggestReply })
  expect(screen.getByText(/She asked how you are/)).toBeVisible()
  expect(screen.getByRole('button', { name: /Explain grammar/ })).toHaveAttribute('aria-expanded', 'false')
  expect(screen.getByRole('button', { name: /Suggest a reply/ })).toHaveAttribute('aria-expanded', 'false')
  expect(onExplainGrammar).not.toHaveBeenCalled()
  expect(onSuggestReply).not.toHaveBeenCalled()
  expect(backend.invoke).not.toHaveBeenCalled()
})

it('asks for reply ideas once and keeps them on reopen', async () => {
  const onSuggestReply = vi.fn().mockResolvedValue(undefined)
  const view = show({ onSuggestReply })
  fireEvent.click(screen.getByRole('button', { name: /Suggest a reply/ }))
  expect(onSuggestReply).toHaveBeenCalledOnce()
  // The answer lands in the snapshot, so reopening must not ask again.
  view.rerender(wrap({onSuggestReply, replies}))
  expect(screen.getAllByRole('listitem')).toHaveLength(2)
  fireEvent.click(screen.getByRole('button', { name: /Suggest a reply/ }))
  fireEvent.click(screen.getByRole('button', { name: /Suggest a reply/ }))
  expect(onSuggestReply).toHaveBeenCalledOnce()
})

it('discloses grammar without touching the reply ideas', () => {
  show({ grammar, replies })
  expect(screen.queryByRole('heading', {name:'Yes-no questions with 吗'})).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: /Explain grammar/ }))
  expect(screen.getByRole('heading', {name:'Yes-no questions with 吗'})).toBeVisible()
  expect(screen.getByText((_, element) => element?.tagName === 'P' && element.textContent?.startsWith('Add 吗') && element.textContent.endsWith('to a statement to make it a question.'))).toBeVisible()
  expect(screen.queryByRole('button', { name: 'Insert reply: 我很好。' })).toBeNull()
})

it('inserts only from the arrow and leaves words tappable', () => {
  const onUse = vi.fn()
  const view = show({ replies, onUse })
  fireEvent.click(screen.getByRole('button', { name: /Suggest a reply/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Insert reply: 我很好。' }))
  expect(onUse).toHaveBeenCalledExactlyOnceWith('我很好。', 'suggestion')
  fireEvent.click(view.container.querySelector('.help-reply')!)
  expect(onUse).toHaveBeenCalledOnce()
  fireEvent.click(screen.getByRole('button', { name: '很' }))
  expect(screen.getByText('very')).toBeVisible()
  expect(onUse).toHaveBeenCalledOnce()
})

it('inserts a sentence starter as a scaffold', () => {
  const onUse = vi.fn()
  show({ starters: ['我和___。'], onUse })
  fireEvent.click(screen.getByRole('button', { name: /Suggest a reply/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Insert starter: 我和___。' }))
  expect(onUse).toHaveBeenCalledExactlyOnceWith('我和___。', 'scaffold')
})

it('disables insertion during a send but not word help', () => {
  show({ replies, busy: true })
  fireEvent.click(screen.getByRole('button', { name: /Suggest a reply/ }))
  expect(screen.getByRole('button', { name: 'Insert reply: 我很好。' })).toBeDisabled()
  expect(screen.getByRole('button', { name: '很' })).toBeEnabled()
})

it('folds to one button that restores the brief', () => {
  show({ grammar })
  fireEvent.click(screen.getByRole('button', { name: 'Hide reply help' }))
  expect(screen.queryByText(/She asked how you are/)).toBeNull()
  const reopen = screen.getByRole('button', { name: 'Help with this reply' })
  expect(reopen).toHaveAttribute('aria-expanded', 'false')
  fireEvent.click(reopen)
  expect(screen.getByText(/She asked how you are/)).toBeVisible()
})

it('stays folded while no brief has arrived', () => {
  render(<ReplyHelp busy={false} errors={[]} onUse={() => {}} onSuggestReply={vi.fn()} />)
  expect(screen.getByRole('button', { name: 'Help with this reply' })).toHaveAttribute('aria-expanded', 'false')
})

it('reports a failed suggestions job without hiding the brief', () => {
  show({ errors: ['Coach feedback rejected: suggestion_token_not_in_reply.'] })
  expect(screen.getByText(/She asked how you are/)).toBeVisible()
  expect(screen.getByRole('alert')).toHaveTextContent("Reply helpThe AI's answer wasn't usable")
  expect(screen.getByText('Coach feedback rejected: suggestion_token_not_in_reply.')).not.toBeVisible()
  fireEvent.click(screen.getByText('Technical details'))
  expect(screen.getByText('Coach feedback rejected: suggestion_token_not_in_reply.')).toBeVisible()
})

it('renders nothing without a brief, a request or a failure', () => {
  const { container } = render(<ReplyHelp busy={false} errors={[]} onUse={() => {}} />)
  expect(container).toBeEmptyDOMElement()
})

it('keeps generation pending after the enqueue command resolves and never re-requests on reopen', async () => {
  const request = vi.fn().mockResolvedValue(undefined)
  show({onSuggestReply:request})
  fireEvent.click(screen.getByRole('button',{name:'Suggest a reply'}))
  await screen.findByText('Writing reply ideas…')
  await Promise.resolve()
  fireEvent.click(screen.getByRole('button',{name:'Suggest a reply'}))
  fireEvent.click(screen.getByRole('button',{name:'Suggest a reply'}))
  expect(request).toHaveBeenCalledOnce()
  expect(screen.getByText('Writing reply ideas…')).toBeVisible()
})

it('accepts an empty grammar result and fixture disclosure without any request', () => {
  const request=vi.fn()
  show({grammar:[],opened:['grammar'],onExplainGrammar:request})
  expect(screen.getByText('Nothing to flag in this reply.')).toBeVisible()
  expect(request).not.toHaveBeenCalled()
})

it('reconciles a failed command response with a later durable result', async () => {
  const request = vi.fn().mockRejectedValue(new Error('Command response lost'))
  const retry = vi.fn()
  const view = show({ onSuggestReply: request, onRetry: retry })
  fireEvent.click(screen.getByRole('button', { name: 'Suggest a reply' }))
  expect(await screen.findByRole('button', { name: 'Retry' })).toBeVisible()
  view.rerender(wrap({ onSuggestReply: request, onRetry: retry, replies }))
  expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull()
  expect(screen.queryByRole('alert')).toBeNull()
  expect(screen.getAllByRole('listitem')).toHaveLength(2)
  expect(request).toHaveBeenCalledOnce()
})

it('starts collapsed and stays collapsed when a brief arrives', () => {
  const view=render(wrap({brief:undefined,onSuggestReply:vi.fn()}))
  view.rerender(wrap({brief:base.brief}))
  expect(screen.getByRole('button', {name:'Help with this reply'})).toHaveAttribute('aria-expanded', 'false')
  expect(screen.queryByText(base.brief)).toBeNull()
  view.rerender(wrap({brief:'Another saved brief'}))
  expect(screen.queryByText('Another saved brief')).toBeNull()
})

it('shows independent durable failures and retries only the selected lane', () => {
  const retry=vi.fn().mockResolvedValue(undefined), request=vi.fn()
  show({onRetry:retry,onExplainGrammar:request,lanes:{brief:{state:'succeeded'},grammar:{state:'failed',error:'Invalid grammar',details:{requestId:'safe-id'}},replies:{state:null}}})
  fireEvent.click(screen.getByRole('button',{name:'Explain grammar'}))
  expect(request).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button',{name:'Retry'}))
  expect(retry).toHaveBeenCalledExactlyOnceWith('grammar')
  expect(screen.getByRole('button',{name:'Suggest a reply'})).toHaveAttribute('aria-expanded','false')
})

it('retains whole-passage translation and sound help without inserting or inventing glosses', () => {
  const onUse=vi.fn()
  show({replies:[{text:'新词。',translation:'New word.',romanization:'xīn cí',pronunciation:'shin tsuh'}],opened:['replies'],onUse})
  fireEvent.click(screen.getByRole('button',{name:'Translate'}))
  expect(screen.getByText('New word.')).toBeVisible()
  openMenus()
  fireEvent.click(screen.getByRole('button',{name:'Pronunciation'}))
  // The null settings fixture has no romanization-enabled language preference.
  expect(screen.getByText('shin tsuh')).toBeVisible()
  expect(onUse).not.toHaveBeenCalled()
  expect(backend.invoke).not.toHaveBeenCalled()
})

it('splits saved mixed-script grammar examples into source and separate reading aids', () => {
  const example = 'مَاذَا تَأْكُلُ؟ (mādhā ta’kulu?) – What are you eating?'
  const view = render(<ReplyHelp {...base} opened={['grammar']} grammar={[{ title: 'Question', quote: 'مَاذَا تَفْعَلُ؟', body: 'Ask and answer questions.', example, contrast: '' }]} />)
  const passages = view.container.querySelectorAll('.reading-passage .msg')
  expect(passages[1].querySelector('.target-text')?.textContent).toBe('مَاذَا تَأْكُلُ؟')
  fireEvent.click(screen.getByRole('button', { name: 'Translate' }))
  expect(screen.getByText('What are you eating?')).toHaveAttribute('dir', 'auto')
  expect(passages[1].querySelector('.target-text')?.textContent).not.toContain('mādhā')
  openMenus()
  fireEvent.click(screen.getByRole('button', { name: 'Pronunciation' }))
  expect(screen.getByText('mādhā ta’kulu?')).toHaveAttribute('dir', 'auto')
})


it('opens saved grammar in the coach controls immediately without requesting it again', () => {
  const request = vi.fn().mockResolvedValue(undefined)
  const card = { title: 'Saved construction', quote: '', body: 'Already available explanation.', example: '', contrast: '' }
  render(<ReplyHelp {...base} inline grammar={[card]} onExplainGrammar={request} />)
  fireEvent.click(screen.getByRole('button', { name: 'Explain grammar' }))
  expect(screen.getByText('Already available explanation.')).toBeVisible()
  expect(request).not.toHaveBeenCalled()
})


it('attaches native surface tags to saved help content rather than hidden request controls', () => {
  const view = render(<ReplyHelp {...base} grammar={grammar} replies={replies} opened={['grammar', 'replies']}
    graphTargets={{ brief: { 'data-graph-results': 'brief-result' }, grammar: { 'data-graph-results': 'grammar-result' }, replies: { 'data-graph-results': 'ideas-result' } }} />)
  expect(view.container.querySelector('[data-graph-results="brief-result"]')).toHaveTextContent(base.brief)
  expect(view.container.querySelector('[data-graph-results="grammar-result"]')).toHaveTextContent(grammar[0].title)
  expect(view.container.querySelector('[data-graph-results="ideas-result"]')).toHaveTextContent(replies[0].text)
})
