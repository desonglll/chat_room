// TG-204: topic list rules — visibility, order after a local upsert, previews, row actions.
import { describe, expect, test } from 'bun:test'
import { ApiError } from '@tg/core'
import { makeTopic } from './topicFixtures'
import {
  removeTopic,
  topicActions,
  topicErrorText,
  topicInitial,
  topicPreview,
  upsertTopic,
  visibleTopics,
} from './topicListModel'

const general = makeTopic('g', { is_general: true, title: 'General' })

describe('visibleTopics', () => {
  test('a hidden General is shown dimmed to managers and not at all to members', () => {
    const topics = [makeTopic('a'), { ...general, is_hidden: true }]
    expect(visibleTopics({ topics, can_manage: true }).map((row) => [row.topic.id, row.dimmed])).toEqual([
      ['g', true],
      ['a', false],
    ])
    expect(visibleTopics({ topics, can_manage: false }).map((row) => row.topic.id)).toEqual(['a'])
  })

  test('a visible General is a normal row, ordered after pinned topics', () => {
    const topics = [makeTopic('a'), general, makeTopic('p', { is_pinned: true, pinned_at: '2026-10-01T00:00:00Z' })]
    expect(visibleTopics({ topics, can_manage: false }).map((row) => row.topic.id)).toEqual(['p', 'g', 'a'])
  })
})

test('upsert keeps the server order; remove drops the row', () => {
  const list = { chat_id: 'c', is_forum: true, can_create: true, can_manage: true, topics: [general, makeTopic('a')] }
  const pinned = upsertTopic(list, makeTopic('a', { is_pinned: true, pinned_at: '2026-10-01T01:00:00Z' }))
  expect(pinned.topics.map((row) => row.id)).toEqual(['a', 'g'])
  expect(upsertTopic(list, makeTopic('new', { created_at: '2026-10-02T00:00:00Z' })).topics.map((r) => r.id)).toEqual([
    'g',
    'new',
    'a',
  ])
  expect(removeTopic(list, 'a').topics).toHaveLength(1)
})

test('preview: "sender: text", whitespace folded; a new topic says so', () => {
  const last = { message_id: 'm', sender: '小明', content: ' 你好\n世界 ', created_at: '2026-10-01T00:00:00Z' }
  expect(topicPreview(makeTopic('a', { last_message: last }))).toEqual({ sender: '小明', text: '你好 世界' })
  expect(topicPreview(makeTopic('a'))).toEqual({ sender: '', text: '话题已创建' })
  expect(topicPreview(makeTopic('a', { last_message: { ...last, content: '' } })).text).toBe('[媒体]')
})

describe('topicActions', () => {
  test('a plain member may only mute', () => {
    expect(topicActions(makeTopic('a'), false)).toEqual(['mute'])
  })

  test('the creator edits and closes; unread adds mark-as-read', () => {
    const last = { message_id: 'm', sender: 's', content: 'c', created_at: '2026-10-01T00:00:00Z' }
    expect(topicActions(makeTopic('a', { can_edit: true, unread_count: 2, last_message: last }), false)).toEqual([
      'read',
      'mute',
      'edit',
      'close',
    ])
  })

  test('a manager pins, deletes other topics and hides General (never deletes it)', () => {
    const managed = makeTopic('a', { can_edit: true, is_closed: true, is_pinned: true, muted: true })
    expect(topicActions(managed, true)).toEqual(['unpin', 'unmute', 'edit', 'reopen', 'delete'])
    expect(topicActions({ ...general, can_edit: true, is_hidden: true }, true)).toEqual([
      'pin',
      'mute',
      'edit',
      'close',
      'unhide',
    ])
  })
})

test('error text follows the bare status; initials take the first character', () => {
  expect(topicErrorText(new ApiError(403, '/x', ''))).toBe('没有权限执行此操作')
  expect(topicErrorText(new ApiError(500, '/x', ''))).toBe('操作失败，请重试')
  expect(topicErrorText(new Error('offline'))).toBe('网络异常，请重试')
  expect(topicInitial('  hello')).toBe('H')
  expect(topicInitial('😀x')).toBe('😀')
  expect(topicInitial('')).toBe('#')
})
