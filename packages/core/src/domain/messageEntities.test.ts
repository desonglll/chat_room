import { describe, expect, test } from 'bun:test'
import {
  customEmojiIds,
  entitiesForSend,
  insertCustomEmoji,
  isEmojiFallback,
  parseMessageEntities,
  reconcileEntities,
  segmentEntityText,
} from './messageEntities'

const emoji = (offset: number, length: number, id = 'e1') => ({
  type: 'custom_emoji',
  offset,
  length,
  custom_emoji_id: id,
})

describe('parseMessageEntities', () => {
  test('keeps valid entities in UTF-16 offsets and drops malformed or stale ones', () => {
    const text = 'hi 😺!'
    const parsed = parseMessageEntities(
      [
        emoji(3, 2),
        emoji(4, 1), // splits the surrogate pair
        emoji(0, 2), // covers "hi": stale after an edit
        emoji(5, 2), // past the end
        { type: 'bold', offset: 0, length: 2 },
        { type: 'custom_emoji', offset: 3, length: 2 }, // no id
        'nonsense',
        { type: 'italic', offset: -1, length: 1 },
      ],
      text,
    )
    expect(parsed).toEqual([{ type: 'bold', offset: 0, length: 2 }, emoji(3, 2)])
  })

  test('absent or non-array entities read as none; overlapping custom emoji keep the first', () => {
    expect(parseMessageEntities(undefined, 'x')).toEqual([])
    expect(parseMessageEntities({}, 'x')).toEqual([])
    expect(parseMessageEntities([emoji(0, 4, 'a'), emoji(2, 2, 'b')], '😺😸')).toEqual([emoji(0, 4, 'a')])
  })

  test('emoji fallbacks include flags, keycaps and ZWJ sequences but not words', () => {
    expect(isEmojiFallback('🇨🇳')).toBe(true)
    expect(isEmojiFallback('1️⃣')).toBe(true)
    expect(isEmojiFallback('👩‍💻')).toBe(true)
    expect(isEmojiFallback('hi')).toBe(false)
    expect(isEmojiFallback('😺 ')).toBe(false)
  })
})

describe('segmentEntityText', () => {
  test('splits text around custom emoji and ignores formatting entities', () => {
    const text = 'a😺b😸'
    const entities = parseMessageEntities(
      [{ type: 'bold', offset: 0, length: 1 }, emoji(1, 2, 'x'), emoji(4, 2, 'y')],
      text,
    )
    expect(segmentEntityText(text, entities)).toEqual([
      { kind: 'text', text: 'a' },
      { kind: 'custom_emoji', text: '😺', customEmojiId: 'x' },
      { kind: 'text', text: 'b' },
      { kind: 'custom_emoji', text: '😸', customEmojiId: 'y' },
    ])
    expect(customEmojiIds([...entities, emoji(6, 2, 'x')])).toEqual(['x', 'y'])
  })

  test('the concatenated segments are exactly the original text', () => {
    const text = '😺😺 and 😸'
    const segments = segmentEntityText(text, parseMessageEntities([emoji(0, 2), emoji(2, 2), emoji(9, 2)], text))
    expect(segments.map((segment) => segment.text).join('')).toBe(text)
    expect(segments.filter((segment) => segment.kind === 'custom_emoji')).toHaveLength(3)
  })
})

describe('composer helpers', () => {
  test('insertCustomEmoji replaces the selection and shifts later entities', () => {
    const start = { text: 'ab😸', entities: [emoji(2, 2, 'later')] }
    const inserted = insertCustomEmoji(start, { start: 1, end: 1 }, { id: 'new', fallback: '😺' })
    expect(inserted.text).toBe('a😺b😸')
    expect(inserted.caret).toBe(3)
    expect(inserted.entities).toEqual([emoji(1, 2, 'new'), emoji(4, 2, 'later')])
  })

  test('inserting the same fallback before an existing emoji keeps both ids apart', () => {
    const inserted = insertCustomEmoji(
      { text: '😺', entities: [emoji(0, 2, 'old')] },
      { start: 0, end: 0 },
      { id: 'new', fallback: '😺' },
    )
    expect(inserted.entities).toEqual([emoji(0, 2, 'new'), emoji(2, 2, 'old')])
  })

  test('reconcileEntities keeps untouched entities and drops edited ones', () => {
    const entities = [emoji(0, 2, 'a'), emoji(4, 2, 'b')]
    expect(reconcileEntities('😺xy😸', '😺xYZy😸', entities)).toEqual([emoji(0, 2, 'a'), emoji(6, 2, 'b')])
    expect(reconcileEntities('😺xy😸', '😺xy', entities)).toEqual([emoji(0, 2, 'a')])
    expect(reconcileEntities('😺xy😸', '😺xy😸', entities)).toEqual(entities)
  })

  test('entitiesForSend mirrors the server trim', () => {
    expect(entitiesForSend('  😺 hi ', [emoji(2, 2)])).toEqual({ text: '😺 hi', entities: [emoji(0, 2)] })
  })
})
