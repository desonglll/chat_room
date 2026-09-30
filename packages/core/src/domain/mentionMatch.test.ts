// TG-104: @mention token detection, candidate ranking, insertion.
import { describe, expect, test } from 'bun:test'
import { applyMention, findMentionQuery, matchMentionCandidates } from './mentionMatch'

const members = [
  { user_id: 'u1', username: 'alice' },
  { user_id: 'u2', username: 'malik' },
  { user_id: 'u3', username: 'Al' },
  { user_id: 'u4', username: 'bob' },
  { user_id: 'me', username: 'alfred' },
]

describe('findMentionQuery', () => {
  test('finds the token between @ and the caret', () => {
    expect(findMentionQuery('hi @al', 6)).toEqual({ start: 3, end: 6, query: 'al' })
    expect(findMentionQuery('@', 1)).toEqual({ start: 0, end: 1, query: '' })
    expect(findMentionQuery('@bo there', 3)).toEqual({ start: 0, end: 3, query: 'bo' })
  })

  test('rejects e-mails, finished tokens and overlong runs', () => {
    expect(findMentionQuery('mail a@b', 8)).toBeNull()
    expect(findMentionQuery('@al done', 8)).toBeNull()
    expect(findMentionQuery(`@${'x'.repeat(40)}`, 41)).toBeNull()
    expect(findMentionQuery('no mention', 10)).toBeNull()
  })

  test('works across a newline', () => {
    expect(findMentionQuery('line\n@b', 7)).toEqual({ start: 5, end: 7, query: 'b' })
  })
})

describe('matchMentionCandidates', () => {
  test('prefix matches first, then shorter, then substring; case-insensitive; excludes self', () => {
    const names = matchMentionCandidates(members, 'AL', { excludeUserId: 'me' }).map((m) => m.username)
    expect(names).toEqual(['Al', 'alice', 'malik'])
  })

  test('empty query lists everyone except self, capped by limit', () => {
    expect(matchMentionCandidates(members, '', { excludeUserId: 'me', limit: 2 })).toHaveLength(2)
    expect(matchMentionCandidates(members, '', { excludeUserId: 'me' })).toHaveLength(4)
  })

  test('deduplicates repeated members and skips nameless ones', () => {
    const list = [...members, { user_id: 'u1', username: 'alice' }, { user_id: 'u9', username: '' }]
    expect(matchMentionCandidates(list, 'ali').map((m) => m.user_id)).toEqual(['u1', 'u2'])
  })
})

describe('applyMention', () => {
  test('replaces the query with @username and a space, caret after it', () => {
    const text = 'hi @al how'
    const mention = findMentionQuery(text, 6)!
    expect(applyMention(text, mention, 'alice')).toEqual({ text: 'hi @alice how', caret: 10 })
  })

  test('appends the separator at the end of the text', () => {
    expect(applyMention('@b', { start: 0, end: 2, query: 'b' }, 'bob')).toEqual({ text: '@bob ', caret: 5 })
  })
})
