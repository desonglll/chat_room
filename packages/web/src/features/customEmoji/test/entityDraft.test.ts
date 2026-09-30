import { describe, expect, test } from 'bun:test'
import { createEntityDraft } from '../useEntityDraft'
import { STATUS_DURATIONS, expiryFor } from '../statusDurations'

describe('createEntityDraft', () => {
  test('tracks picked emoji through typing and yields trimmed text + entities for send', () => {
    const draft = createEntityDraft()
    draft.sync('hi ')
    const inserted = draft.insert('hi ', { start: 3, end: 3 }, { id: 'e1', emoji: '😺' })
    expect(inserted).toEqual({
      text: 'hi 😺',
      caret: 5,
      entities: [{ type: 'custom_emoji', offset: 3, length: 2, custom_emoji_id: 'e1' }],
    })
    draft.sync('hi 😺 ') // typed after it
    draft.sync('  hi 😺 ') // then before it
    expect(draft.forSend('  hi 😺 ')).toEqual({
      text: 'hi 😺',
      entities: [{ type: 'custom_emoji', offset: 3, length: 2, custom_emoji_id: 'e1' }],
    })
    draft.sync('  hi ') // deleted the emoji
    expect(draft.entities()).toEqual([])
    draft.reset()
    expect(draft.forSend('')).toEqual({ text: '', entities: [] })
  })
})

describe('status durations', () => {
  test('forever has no expiry; the others are relative to now', () => {
    const now = Date.parse('2026-10-01T00:00:00Z')
    expect(expiryFor(STATUS_DURATIONS[0]!, now)).toBeNull()
    expect(expiryFor(STATUS_DURATIONS[1]!, now)).toBe('2026-10-01T01:00:00.000Z')
  })
})
