/** TG-802: the chat-list line for every media kind, server-named and timeline-derived. */
import { describe, expect, test } from 'bun:test'
import type { ConversationLastMessage, PreviewMediaKind } from '@tg/core'
import { buildChatPreview } from './chatPreview'
import { previewMediaKind } from './previewMediaKind'

const last = (overrides: Partial<ConversationLastMessage>): ConversationLastMessage => ({
  message_id: 'm1',
  sender_id: 'u2',
  sender: 'alice',
  content: '',
  attachment_file_name: null,
  recalled: false,
  created_at: '2026-10-01T09:05:00Z',
  ...overrides,
})

const line = (overrides: Partial<ConversationLastMessage>) =>
  buildChatPreview({ chatType: 'private', lastMessage: last(overrides), currentUserId: 'u1', draftText: '' })

describe('server-named media kinds', () => {
  const cases: [PreviewMediaKind, string, string][] = [
    ['voice', 'voice', '语音消息'],
    ['video_note', 'videoNote', '视频消息'],
    ['sticker', 'sticker', '贴纸'],
    ['gif', 'gif', 'GIF'],
    ['poll', 'poll', '投票'],
    ['location', 'location', '位置'],
    ['live_location', 'liveLocation', '实时位置'],
    ['contact', 'contact', '联系人'],
    ['album', 'album', '相册'],
    ['photo', 'photo', '图片'],
    ['video', 'video', '视频'],
    ['audio', 'audio', '音频'],
  ]
  for (const [kind, icon, text] of cases) {
    test(`${kind} → ${text}`, () => {
      expect(line({ media_kind: kind, attachment_file_name: 'x.bin' })).toMatchObject({ media: icon, text })
    })
  }

  test('a voice message is no longer guessed as audio from its extension', () => {
    expect(line({ media_kind: 'voice', attachment_file_name: 'voice.webm' })).toMatchObject({ media: 'voice' })
  })
  test('a caption or poll question wins over the label', () => {
    expect(line({ media_kind: 'poll', content: 'Lunch?' })).toMatchObject({ media: 'poll', text: 'Lunch?' })
  })
  test('a file shows its name', () => {
    expect(line({ media_kind: 'file', attachment_file_name: 'plan.pdf' })).toMatchObject({ text: 'plan.pdf' })
  })
  test('without media_kind the old file-name guess still applies', () => {
    expect(line({ attachment_file_name: 'pic.png' })).toMatchObject({ media: 'photo' })
  })
})

describe('previewMediaKind (live timeline)', () => {
  test('mirrors the server classifier', () => {
    expect(previewMediaKind({})).toBeNull()
    expect(previewMediaKind({ media_kind: 'voice', attachment: { mime_type: 'audio/ogg' } })).toBe('voice')
    expect(previewMediaKind({ media_kind: 'sticker' })).toBe('sticker')
    expect(previewMediaKind({ media_kind: 'contact' })).toBe('contact')
    expect(previewMediaKind({ media_kind: 'location', location: { latitude: 1, longitude: 2, updated_at: '' } })).toBe(
      'location',
    )
    expect(previewMediaKind({ location: { latitude: 1, longitude: 2, live_until: 'later', updated_at: '' } })).toBe(
      'live_location',
    )
    expect(previewMediaKind({ attachment: { mime_type: 'image/gif' } })).toBe('gif')
    expect(previewMediaKind({ attachment: { mime_type: 'image/png' }, grouped_id: 'g' })).toBe('album')
    expect(previewMediaKind({ attachment: { mime_type: 'application/pdf' }, grouped_id: 'g' })).toBe('file')
    expect(previewMediaKind({ attachment: { mime_type: 'video/mp4' } })).toBe('video')
  })
  test('a poll wins', () => {
    expect(previewMediaKind({ poll: {} as never, attachment: { mime_type: 'image/png' } })).toBe('poll')
  })
})
