import { expect, it } from 'vitest'
import { readingWords, readingPassage, wholeWordRange } from './word-boundaries'
it('reuses ICU setup while segmenting each new source independently', () => {
  const NativeSegmenter = Intl.Segmenter
  let constructions = 0
  Object.defineProperty(Intl, 'Segmenter', { value: new Proxy(NativeSegmenter, { construct(target, args) {
    constructions++
    return Reflect.construct(target, args)
  } }) })
  try {
    readingWords('first source', 'en-CA')
    const count = constructions
    const text = 'cafe\u0301 👩‍💻 next'
    const words = readingWords(text, 'en-CA')
    expect(constructions).toBe(count)
    expect(words.map(part => text.slice(part.start, part.end)).join('')).toBe(text)
    expect(words.filter(part => part.word).map(part => text.slice(part.start, part.end))).toEqual(['cafe\u0301', 'next'])
  } finally { Object.defineProperty(Intl, 'Segmenter', { value: NativeSegmenter }) }
})
it.each(['sí, sí', 'الكتاب جميل', 'नमस्ते', 'വീട്ടിൽ', '你好世界', '👩‍💻 sí'])('preserves all source characters in %s', text => {
  const words=readingWords(text)
  expect(words.map(part=>text.slice(part.start,part.end)).join('')).toBe(text)
  expect(words.some(part=>part.word)).toBe(true)
})
it.each(['cafe\u0301', 'الكتاب', 'नमस्ते', 'വീട്ടിൽ', '中文', 'hello'])('expands a partial word without rewriting %s', word => {
  const text = `!${word}!`
  const range = wholeWordRange(text, 2, Math.max(3, word.length))
  expect(text.slice(range.start, range.end)).toBe(word)
})
it('preserves punctuation, exact boundaries and complete emoji graphemes', () => {
  expect(wholeWordRange('one, two!', 1, 7)).toEqual({ start: 0, end: 8 })
  expect(wholeWordRange('one, two!', 0, 3)).toEqual({ start: 0, end: 3 })
  expect(wholeWordRange('👩‍💻!', 1, 3)).toEqual({ start: 0, end: 5 })
  expect(wholeWordRange('one', 1, 1)).toEqual({ start: 1, end: 1 })
})
it('keeps the selected repeated occurrence in a bounded passage',()=>{
  const text='Earlier. '.repeat(300)+'sí, sí.'
  const start=text.lastIndexOf('sí')
  const part=readingPassage(text,start,start+2,'es')
  expect(part.text.length).toBeLessThanOrEqual(2048); expect(part.text.slice(part.start,part.end)).toBe('sí'); expect(part.start).toBe(part.text.lastIndexOf('sí'))
})

it('selects an early sentence without changing source offsets or long-sentence fallback', () => {
  const first = 'First café. '
  const text = first + 'Later sentence. '.repeat(300)
  expect(readingPassage(text, 6, 10)).toEqual({ text: first, start: 6, end: 10 })
  const long = 'word '.repeat(500) + '.'
  expect(readingPassage(long, 10, 14)).toEqual({ text: 'word', start: 0, end: 4 })
  // A selection crossing sentence boundaries still falls back to its exact text.
  const start = text.indexOf('café'), end = text.indexOf('Later') + 5
  expect(readingPassage(text, start, end)).toEqual({ text: text.slice(start, end), start: 0, end: end - start })
})
