// @vitest-environment jsdom
import { render, screen } from '@testing-library/react'
import { expect, it } from 'vitest'
import { CoachEntry } from './CoachEntry'
it('shows useful language explanations immediately, without internal metrics or nested disclosure', () => {
  const view = render(<CoachEntry source="Sí, me gusta cocinar." feedback={{issues: [], corrections: [], notes: [], meaningRecovered:'full',candidatesSent:46,itemsReturned:1,items:[{construct:'event_roles',outcome:'demonstrated',quote:'me gusta cocinar',rationale:'Me gusta followed by an infinitive means “I like doing something.” Cocinar means “to cook.”'}]}} />)
  expect(screen.getByText(/Me gusta followed/)).toBeVisible()
  expect(view.container.querySelector('details')).toBeNull()
  expect(view.container.textContent).not.toMatch(/Meaning recovered|Candidate constructs|Returned items|event_roles|demonstrated/)
})
it('shows original, correction and why only after the explicit answer is exposed', () => {
  const decision = {exposedMove:'explicit' as const,shown:{construct:'spelling',quote:'cosenar',move:'explicit' as const,text:'cocinar',explanation:'Cocinar means “to cook.” The verb ends in -ar.'},retryInvited:false,alsoNoticed:[],keptGoing:false}
  const view = render(<CoachEntry source={null} decision={decision} />)
  expect(view.container.querySelector('del.cor-removed')).toHaveTextContent('cosenar')
  expect(view.container.querySelector('.cor-replacement')).toHaveTextContent('cocinar')
  expect(screen.getByText(/Cocinar means/)).toBeVisible()
  view.rerender(<CoachEntry source={null} decision={{...decision,exposedMove:null}} />)
  expect(screen.queryByText('cocinar')).toBeNull()
  expect(screen.queryByText(/Cocinar means/)).toBeNull()
})

it('renders no suggestion for evidence-only success and deduplicates identical explanations', () => {
  const item = {construct:'information_exchange',outcome:'demonstrated' as const,quote:'¿Cómo estás?',rationale:''}
  const feedback = {issues: [], corrections: [], notes: [], meaningRecovered:'full' as const,candidatesSent:2,itemsReturned:2,items:[item,{...item,construct:'greeting'}]}
  const view = render(<CoachEntry source={null} feedback={feedback} />)
  expect(view.container.querySelectorAll('.coach-card')).toHaveLength(0)
  expect(view.container.textContent).toBe('No correction identified.')
  view.rerender(<CoachEntry source={null} feedback={{...feedback,items:feedback.items.map(i => ({...i,rationale:'One short tip.'}))}} />)
  expect(view.container.querySelectorAll('.coach-card')).toHaveLength(1)
})

it('shows every distinct disclosed correction and retains assessment notes', () => {
  const corrections = Array.from({ length: 8 }, (_, i) => ({ construct: 'test', quote: `source ${i}`, text: `replacement ${i}`, move: 'explicit' as const, explanation: `reason ${i}` }))
  const decision = { exposedMove: 'explicit' as const, shown: corrections[0], retryInvited: false, alsoNoticed: [], keptGoing: false }
  const feedback = { issues: [], corrections, notes: ['An unchanged replacement was omitted.'], meaningRecovered: 'full' as const, items: [], candidatesSent: 1, itemsReturned: 9 }
  const view = render(<CoachEntry source={null} decision={decision} feedback={feedback} />)
  expect(screen.getAllByLabelText('Coaching suggestion')).toHaveLength(8)
  expect(screen.getByText('replacement 7')).toBeVisible()
  expect(screen.getByText('An unchanged replacement was omitted.')).toBeInTheDocument()
  view.rerender(<CoachEntry source={null} decision={{ ...decision, exposedMove: null }} feedback={feedback} />)
  expect(screen.queryByText('replacement 7')).toBeNull()
})

it('narrows a sentence-length explicit correction to its changed words and keeps the full pair', () => {
  const shown = { construct: 'negation', quote: 'Ain’t no one ever talked about a bullet bus.', text: 'No one has ever talked about a bullet bus.', move: 'explicit' as const, explanation: 'Ain’t no one is a double negative.' }
  const view = render(<CoachEntry source={null} decision={{ exposedMove: 'explicit', shown, retryInvited: false, alsoNoticed: [], keptGoing: false }} />)
  const changes = screen.getByRole('list', { name: 'Changed words' })
  expect(changes.querySelector('del')).toHaveTextContent('Ain’t no one')
  expect(changes.querySelector('ins')).toHaveTextContent('No one has')
  expect(view.container.querySelector('details.cor-full')).toHaveAttribute('open')
  expect(view.container.querySelector('del.cor-removed')).toHaveTextContent(shown.quote)
  expect(view.container.querySelector('.cor-replacement')).toHaveTextContent(shown.text)
})

it('shows hints as the quote and coach text, without a replacement diff', () => {
  const shown = { construct: 'negation', quote: 'Ain’t no one ever talked about it.', text: 'Look at how many negative words this uses.', move: 'hint' as const }
  render(<CoachEntry source={null} decision={{ exposedMove: 'hint', shown, retryInvited: false, alsoNoticed: [], keptGoing: false }} />)
  expect(screen.queryByRole('list', { name: 'Changed words' })).toBeNull()
  expect(screen.getByText(shown.text)).toBeVisible()
})


it('does not call a pending disclosure clean and reveals the saved correction afterward', () => {
  const shown = { construct: 'form', quote: 'source', text: 'replacement', move: 'explicit' as const }
  const feedback = { issues: [{ quote: 'source', severity: 'partial' as const }], corrections: [], notes: [], meaningRecovered: 'full' as const, items: [], candidatesSent: 1, itemsReturned: 1 }
  const decision = { shown, exposedMove: null, retryInvited: false, alsoNoticed: [], keptGoing: false }
  const view = render(<CoachEntry source={null} feedback={feedback} decision={decision} />)
  expect(view.container).toHaveTextContent('1 suggestion')
  expect(view.container).not.toHaveTextContent('No correction identified.')
  expect(screen.queryByText('replacement')).toBeNull()
  view.rerender(<CoachEntry source={null} feedback={feedback} decision={{ ...decision, exposedMove: 'explicit' }} />)
  expect(view.container.querySelector('.cor-replacement')).toHaveTextContent('replacement')
})

it('does not call omitted evidence clean', () => {
  const feedback = { issues: [], corrections: [], notes: ['Unusable assessment item omitted.'], meaningRecovered: 'full' as const, items: [], candidatesSent: 1, itemsReturned: 0 }
  const view = render(<CoachEntry source={null} feedback={feedback} />)
  expect(view.container).not.toHaveTextContent('No correction identified.')
  expect(view.container).toHaveTextContent('Unusable assessment item omitted.')
})
