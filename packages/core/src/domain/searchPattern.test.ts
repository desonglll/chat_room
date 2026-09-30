import { describe, expect, test } from 'bun:test'
import { likePattern, searchMatchRanges, textMatchesSearch } from './searchPattern'

describe('search pattern', () => {
  test('escapes SQL wildcards exactly like src/messages/search_pattern.rs', () => {
    // The pinned Rust test vector, byte for byte.
    expect(likePattern('50%_done\\ok')).toBe('%50\\%\\_done\\\\ok%')
    expect(likePattern('plain')).toBe('%plain%')
  })

  test('predicts the server match: case-insensitive contains', () => {
    expect(textMatchesSearch('Release PLAN tomorrow', 'plan')).toBe(true)
    expect(textMatchesSearch('nothing here', 'plan')).toBe(false)
    expect(textMatchesSearch('anything', '')).toBe(false)
  })

  test('finds every non-overlapping highlight range', () => {
    expect(searchMatchRanges('aba ABA', 'aba')).toEqual([
      { start: 0, end: 3 },
      { start: 4, end: 7 },
    ])
    expect(searchMatchRanges('aaaa', 'aa')).toEqual([
      { start: 0, end: 2 },
      { start: 2, end: 4 },
    ])
    expect(searchMatchRanges('text', '')).toEqual([])
  })
})
