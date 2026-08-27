import { describe, expect, it, mock } from 'bun:test'
import type { MenuItem } from 'primevue/menuitem'
import { conversationPreferenceMenuItems } from './conversationPreferenceMenu'
import type { ConversationSummary } from './types'

const conversation = {
  room_id: 'room-1',
  kind: 'group',
  title: '研发群',
  alias: '',
  avatar_emoji: '',
  description: '',
  group: null,
  peer: null,
  unread_count: 0,
  pending_join_requests: 0,
  preferences: {
    room_id: 'room-1',
    is_pinned: false,
    is_archived: false,
    notification_level: 'all',
    muted_until: null,
    updated_at: '2026-08-19T08:00:00Z',
  },
  last_message: null,
  last_activity_at: '2026-08-19T08:00:00Z',
  created_at: '2026-08-18T08:00:00Z',
} satisfies ConversationSummary

function run(item: MenuItem | undefined): void {
  const command = item?.command as (() => void) | undefined
  command?.()
}

describe('conversation preference menu', () => {
  it('offers every preference command and emits its minimal patch', () => {
    const save = mock(() => {})
    const items = conversationPreferenceMenuItems(conversation, false, save)

    expect(items.map((item) => item.label)).toEqual(['置顶会话', '归档会话', '通知设置', '静音时长'])
    run(items[0])
    run(items[1])
    run(items[2]?.items?.[1])
    run(items[3]?.items?.[3])

    expect(save.mock.calls.map(([patch]) => patch)).toEqual([
      { is_pinned: true },
      { is_archived: true },
      { notification_level: 'mentions' },
      { muted_until: null },
    ])
  })
})
