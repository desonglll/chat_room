// TG-107: typing copy for all nine actions, 1/2/3+ merging, private phrasing, 5 s expiry.
import { describe, expect, test } from 'bun:test'
import { TYPING_ACTIONS } from '../types'
import type { ActiveTypingAction, TypingActor } from './typingSummary'
import { TYPING_ACTION_COPY, TYPING_TTL_MS, summarizeTyping } from './typingSummary'

const actor = (
  user_id: string,
  username: string,
  action: ActiveTypingAction = 'typing',
  receivedAt = 0,
): TypingActor => ({
  user_id,
  username,
  action,
  receivedAt,
})

const group = (indicators: TypingActor[], now = 0) =>
  summarizeTyping({ indicators, now, chatType: 'group', currentUserId: 'me' })?.text ?? null

describe('summarizeTyping', () => {
  test('every displayable TG-007 action has its own non-empty copy', () => {
    const displayable = TYPING_ACTIONS.filter((action) => action !== 'cancel')
    expect(displayable).toHaveLength(9)
    expect(Object.keys(TYPING_ACTION_COPY).sort()).toEqual([...displayable].sort())
    const copies = displayable.map((action) => TYPING_ACTION_COPY[action])
    expect(new Set(copies).size).toBe(9)
    for (const action of displayable) {
      expect(group([actor('a', 'Alice', action)])).toBe(`Alice ${TYPING_ACTION_COPY[action]}`)
    }
    expect(TYPING_ACTION_COPY.recording_voice).toBe('正在录音')
    expect(TYPING_ACTION_COPY.recording_video_note).toBe('正在录制视频消息')
    expect(TYPING_ACTION_COPY.uploading_photo).toBe('正在发送图片')
  })

  test('merges one, two and three-plus actors', () => {
    expect(group([])).toBeNull()
    expect(group([actor('a', 'A')])).toBe('A 正在输入')
    expect(group([actor('a', 'A'), actor('b', 'B')])).toBe('A 和 B 正在输入')
    expect(group([actor('a', 'A'), actor('b', 'B'), actor('c', 'C')])).toBe('A 和其他 2 人正在输入')
    expect(group([actor('a', 'A'), actor('b', 'B'), actor('c', 'C'), actor('d', 'D')])).toBe('A 和其他 3 人正在输入')
  })

  test('same action keeps its copy; mixed actions fall back to the generic verb', () => {
    expect(group([actor('a', 'A', 'recording_voice'), actor('b', 'B', 'recording_voice')])).toBe('A 和 B 正在录音')
    expect(group([actor('a', 'A', 'recording_voice'), actor('b', 'B', 'uploading_photo')])).toBe('A 和 B 正在输入')
  })

  test('private chats omit the name, and the viewer never sees themself', () => {
    const summary = summarizeTyping({
      indicators: [actor('peer', 'Bob', 'uploading_photo')],
      now: 0,
      chatType: 'private',
      currentUserId: 'me',
    })
    expect(summary).toEqual({ text: '正在发送图片', action: 'uploading_photo', count: 1 })
    expect(group([actor('me', 'Me'), actor('b', 'B')])).toBe('B 正在输入')
    expect(group([actor('me', 'Me')])).toBeNull()
  })

  test('an actor without a username gets a placeholder name', () => {
    expect(group([actor('a', '  ')])).toBe('有人 正在输入')
  })

  test('clears within 5 s of the last frame', () => {
    expect(TYPING_TTL_MS).toBeLessThanOrEqual(5_000)
    const indicators = [actor('a', 'A', 'typing', 10_000)]
    expect(group(indicators, 10_000 + TYPING_TTL_MS - 1)).toBe('A 正在输入')
    expect(group(indicators, 10_000 + TYPING_TTL_MS)).toBeNull()
    expect(group(indicators, 15_000)).toBeNull()
    // One stale and one fresh actor: only the fresh one is named.
    expect(group([actor('a', 'A', 'typing', 0), actor('b', 'B', 'typing', 4_000)], 6_000)).toBe('B 正在输入')
  })
})
