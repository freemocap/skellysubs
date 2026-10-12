import { describe, expect, it } from 'vitest'
import { humanizeId, operationVerb, shapeFields, shapeWords, TYPE_HUE_COUNT, typeHue, typeName } from './graph-types'

describe('typeHue', () => {
  it('is deterministic and within the hue range', () => {
    for (const contract of ['1:conversation.validated-context', 't:text', 'x']) {
      const hue = typeHue(contract)
      expect(hue).toBe(typeHue(contract))
      expect(hue).toBeGreaterThanOrEqual(0)
      expect(hue).toBeLessThan(TYPE_HUE_COUNT)
      expect(Number.isInteger(hue)).toBe(true)
    }
  })
  it('rejects an empty contract', () => {
    expect(() => typeHue('')).toThrow()
  })
})

describe('typeName and operationVerb', () => {
  it('turn the last dotted segment into words', () => {
    expect(typeName('1:conversation.validated-context')).toBe('validated context')
    expect(typeName('t:text')).toBe('t:text')
    expect(operationVerb('1:conversation.generate-prose')).toBe('Generate prose')
    expect(operationVerb('1:speech.synthesize')).toBe('Synthesize')
  })
  it('reject contracts with nothing to display', () => {
    expect(() => typeName('')).toThrow()
    expect(() => operationVerb('1:conversation.')).toThrow()
  })
})

describe('shapeWords', () => {
  it('names scalars, containers and record fields', () => {
    expect(shapeWords('Text')).toBe('text')
    expect(shapeWords({ List: 'Integer' })).toBe('list of integer')
    expect(shapeWords({ Nullable: { Map: 'Number' } })).toBe('map of number or nothing')
    expect(shapeWords({ Record: { targetLanguage: 'Text', replies: { List: 'Text' } } })).toBe('target language, replies')
    expect(shapeFields({ Record: { a: 'Text', b: 'Text' } })).toEqual(['a', 'b'])
    expect(shapeFields('Text')).toEqual([])
  })
  it('rejects an empty record', () => {
    expect(() => shapeWords({ Record: {} })).toThrow()
  })
})

describe('humanizeId', () => {
  it('turns underscores into spaces and capitalizes the first letter only', () => {
    expect(humanizeId('reply_source')).toBe('Reply source')
    expect(humanizeId('lookup')).toBe('Lookup')
    expect(humanizeId('LLM_call')).toBe('LLM call')
  })
  it('rejects an empty identifier', () => {
    expect(() => humanizeId('')).toThrow()
  })
})
