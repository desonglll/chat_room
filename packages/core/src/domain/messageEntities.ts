/**
 * TG-304 message entities: pure rules shared by the renderer and the composer.
 *
 * The message text is the source of truth. A `custom_emoji` entity covers the fallback
 * emoji characters inside the text, so anything that ignores entities (an old client, a
 * copy to the clipboard, a failed image load) still shows a real emoji. Offsets are UTF-16
 * code units — JavaScript string indices — so nothing here converts.
 *
 * Received entities are untrusted and may be stale (an edit applied without its new
 * entities), so `parseMessageEntities` re-validates them against the text it will index.
 */
import type { MessageEntity } from '../types'

export const CUSTOM_EMOJI_ENTITY = 'custom_emoji'
/** Server ceiling, mirrored so the composer never sends what the server would drop. */
export const MAX_MESSAGE_ENTITIES = 100

export type EntitySegment =
  | { kind: 'text'; text: string }
  | { kind: 'custom_emoji'; text: string; customEmojiId: string }

export interface EntityText {
  text: string
  entities: MessageEntity[]
}

const PICTOGRAPHIC = /\p{Extended_Pictographic}|\p{Regional_Indicator}|⃣/u
const WHITESPACE = /\s/u

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null

const isLowSurrogate = (code: number) => code >= 0xdc00 && code <= 0xdfff

/** An index that does not split a surrogate pair. */
function onBoundary(text: string, index: number): boolean {
  return index === 0 || index === text.length || !isLowSurrogate(text.charCodeAt(index))
}

/** Whether `text` can be the fallback of a custom emoji: pictographic, no whitespace. */
export function isEmojiFallback(text: string): boolean {
  return text.length > 0 && PICTOGRAPHIC.test(text) && !WHITESPACE.test(text)
}

/**
 * The usable entities of `text`: well-formed, in bounds, on character boundaries, sorted by
 * offset (outer first). Custom emoji must cover emoji characters and never overlap one
 * another — a stale entity over ordinary text is dropped rather than drawn over it.
 */
export function parseMessageEntities(value: unknown, text: string): MessageEntity[] {
  if (!Array.isArray(value)) return []
  const valid: MessageEntity[] = []
  for (const raw of value) {
    if (!isRecord(raw) || typeof raw.type !== 'string') continue
    const { offset, length } = raw
    if (typeof offset !== 'number' || typeof length !== 'number') continue
    if (!Number.isInteger(offset) || !Number.isInteger(length) || offset < 0 || length <= 0) continue
    if (offset + length > text.length || !onBoundary(text, offset) || !onBoundary(text, offset + length)) continue
    const entity: MessageEntity = { type: raw.type, offset, length }
    if (raw.type === CUSTOM_EMOJI_ENTITY) {
      if (typeof raw.custom_emoji_id !== 'string' || raw.custom_emoji_id === '') continue
      if (!isEmojiFallback(text.slice(offset, offset + length))) continue
      entity.custom_emoji_id = raw.custom_emoji_id
    }
    if (typeof raw.url === 'string') entity.url = raw.url
    if (typeof raw.user_id === 'string') entity.user_id = raw.user_id
    if (typeof raw.language === 'string') entity.language = raw.language
    valid.push(entity)
  }
  valid.sort((a, b) => a.offset - b.offset || b.length - a.length)
  let emojiEnd = 0
  return valid.filter((entity) => {
    if (entity.type !== CUSTOM_EMOJI_ENTITY) return true
    if (entity.offset < emojiEnd) return false
    emojiEnd = entity.offset + entity.length
    return true
  })
}

export function hasCustomEmoji(entities: readonly MessageEntity[]): boolean {
  return entities.some((entity) => entity.type === CUSTOM_EMOJI_ENTITY)
}

/**
 * Split `text` into plain runs and custom emoji runs. Only `custom_emoji` is drawn today;
 * the other entity types are carried but render as their plain text. Expects the output of
 * `parseMessageEntities`.
 */
export function segmentEntityText(text: string, entities: readonly MessageEntity[]): EntitySegment[] {
  const segments: EntitySegment[] = []
  let cursor = 0
  for (const entity of entities) {
    if (entity.type !== CUSTOM_EMOJI_ENTITY || entity.custom_emoji_id === undefined) continue
    if (entity.offset < cursor) continue
    if (entity.offset > cursor) segments.push({ kind: 'text', text: text.slice(cursor, entity.offset) })
    const end = entity.offset + entity.length
    segments.push({ kind: 'custom_emoji', text: text.slice(entity.offset, end), customEmojiId: entity.custom_emoji_id })
    cursor = end
  }
  if (cursor < text.length) segments.push({ kind: 'text', text: text.slice(cursor) })
  return segments
}

/** Every distinct custom emoji id, in first-use order — what a renderer resolves. */
export function customEmojiIds(entities: readonly MessageEntity[]): string[] {
  const ids: string[] = []
  for (const entity of entities) {
    const id = entity.custom_emoji_id
    if (entity.type === CUSTOM_EMOJI_ENTITY && id !== undefined && !ids.includes(id)) ids.push(id)
  }
  return ids
}

/**
 * Carry entities across an arbitrary text edit (typing, paste, deletion) by diffing the
 * two texts: entities wholly before the changed span stay, wholly after it shift, and any
 * entity the edit touched is dropped (its characters are no longer what it described).
 */
export function reconcileEntities(previous: string, next: string, entities: readonly MessageEntity[]): MessageEntity[] {
  if (previous === next) return [...entities]
  let prefix = 0
  const shortest = Math.min(previous.length, next.length)
  while (prefix < shortest && previous[prefix] === next[prefix]) prefix++
  let suffix = 0
  while (suffix < shortest - prefix && previous[previous.length - 1 - suffix] === next[next.length - 1 - suffix]) {
    suffix++
  }
  return spliceEntities(entities, prefix, previous.length - suffix, next.length - suffix - prefix)
}

/**
 * Entities after replacing `[start, end)` with `inserted` code units: those wholly before
 * stay, those wholly after shift, those the replacement touched are dropped.
 */
export function spliceEntities(
  entities: readonly MessageEntity[],
  start: number,
  end: number,
  inserted: number,
): MessageEntity[] {
  const delta = inserted - (end - start)
  const kept: MessageEntity[] = []
  for (const entity of entities) {
    if (entity.offset + entity.length <= start) kept.push({ ...entity })
    else if (entity.offset >= end) kept.push({ ...entity, offset: entity.offset + delta })
  }
  return kept
}

/**
 * Replace the selection `[start, end)` with a custom emoji's fallback text and record the
 * entity. Returns the new state and the caret position after the inserted emoji.
 */
export function insertCustomEmoji(
  state: EntityText,
  selection: { start: number; end: number },
  emoji: { id: string; fallback: string },
): EntityText & { caret: number } {
  const start = Math.max(0, Math.min(selection.start, state.text.length))
  const end = Math.max(start, Math.min(selection.end, state.text.length))
  const text = state.text.slice(0, start) + emoji.fallback + state.text.slice(end)
  const entities = spliceEntities(state.entities, start, end, emoji.fallback.length)
  const caret = start + emoji.fallback.length
  if (entities.length < MAX_MESSAGE_ENTITIES && isEmojiFallback(emoji.fallback)) {
    entities.push({
      type: CUSTOM_EMOJI_ENTITY,
      offset: start,
      length: emoji.fallback.length,
      custom_emoji_id: emoji.id,
    })
    entities.sort((a, b) => a.offset - b.offset || b.length - a.length)
  }
  return { text, entities, caret }
}

/**
 * The entities to send with `text` after the server-side trim: shift by the trimmed
 * prefix and drop what falls outside. Mirrors the server so optimistic rendering matches.
 */
export function entitiesForSend(text: string, entities: readonly MessageEntity[]): EntityText {
  const trimmed = text.trim()
  const lead = text.length - text.trimStart().length
  const shifted = entities.map((entity) => ({ ...entity, offset: entity.offset - lead }))
  return { text: trimmed, entities: parseMessageEntities(shifted, trimmed).slice(0, MAX_MESSAGE_ENTITIES) }
}
