import { sentenceBlanks } from './sentence-blanks'

// ICU setup is reusable; source text and segmentation results are not retained.
const segmenters = new Map<string, Intl.Segmenter>()
function getSegmenter(locale: string | undefined, granularity: 'word' | 'sentence' | 'grapheme') {
  const key = JSON.stringify([locale || null, granularity])
  let value = segmenters.get(key)
  if (value) segmenters.delete(key)
  else value = new Intl.Segmenter(locale || undefined, { granularity })
  segmenters.set(key, value)
  if (segmenters.size > 32) segmenters.delete(segmenters.keys().next().value!)
  return value
}

/** Local navigation boundaries only. Meanings still come from validated source
 * annotations; Unicode segmentation is never presented as linguistic analysis. */
export function readingWords(text: string, locale?: string, template = false) {
  const segmenter = getSegmenter(locale, 'word')
  const parts: { start: number; end: number; word: boolean; blank: boolean }[] = []
  const appendWords = (start: number, end: number) => {
    for (const item of segmenter.segment(text.slice(start, end))) {
      parts.push({ start: start + item.index, end: start + item.index + item.segment.length, word: !!item.isWordLike, blank: false })
    }
  }
  let cursor = 0
  for (const blank of sentenceBlanks(text, template)) {
    appendWords(cursor, blank.start)
    parts.push({ ...blank, word: false, blank: true })
    cursor = blank.end
  }
  appendWords(cursor, text.length)
  return parts
}

/** Large reports remain inspectable without truncating a chosen occurrence.
 * Prefer its complete sentence; a very long sentence uses the selected word. */
export function readingPassage(text: string, start: number, end: number, locale?: string) {
  if (text.length <= 2048) return { text, start, end }
  for (const sentence of getSegmenter(locale, 'sentence').segment(text)) {
    if (sentence.index <= start && sentence.index + sentence.segment.length >= end) {
      if (sentence.segment.length <= 2048) return { text: sentence.segment, start: start - sentence.index, end: end - sentence.index }
      break
    }
  }
  return { text: text.slice(start, end), start: 0, end: end - start }
}

/** Expand touched words and graphemes without changing any source characters. */
export function wholeWordRange(text: string, start: number, end: number, locale?: string) {
  if (start === end) return { start, end }
  for (const part of getSegmenter(locale, 'grapheme').segment(text)) {
    const next = part.index + part.segment.length
    if (part.index < start && start < next) start = part.index
    if (part.index < end && end < next) end = next
  }
  for (const part of readingWords(text, locale)) {
    if (!part.word) continue
    if (part.start < start && start < part.end) start = part.start
    if (part.start < end && end < part.end) end = part.end
  }
  return { start, end }
}
