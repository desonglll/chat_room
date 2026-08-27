import { describe, expect, test } from 'bun:test'
import type { BroadcastMessage } from '../types'
import { useMessageContextMenu } from './useMessageContextMenu'

const message: BroadcastMessage = {
  type: 'broadcast',
  message_id: 'message-1',
  sender_id: 'user-2',
  sender: 'Lin',
  sender_avatar: '',
  content: 'decision',
  attachment: null,
  reply_to: null,
  recalled_at: null,
  edited_at: null,
  timestamp: '2026-08-27T10:00:00Z',
  forwarded_from: null,
  reactions: [],
}

function menu(aiEnabled: boolean, askAi: (messageId: string) => void) {
  return useMessageContextMenu({
    currentUserId: () => 'user-1',
    favoriteMessageIds: () => [],
    pinnedMessageIds: () => [],
    canPin: () => false,
    aiEnabled: () => aiEnabled,
    retry: () => {},
    reply: () => {},
    askAi,
    forward: () => {},
    task: () => {},
    favorite: () => {},
    pin: () => {},
    edit: () => {},
    recall: () => {},
  })
}

describe('message context menu', () => {
  test('offers exact-message AI context only when AI is available', () => {
    let selected = ''
    const enabled = menu(true, (messageId) => (selected = messageId))
    enabled.contextMenu.value = { show: () => {} }
    enabled.openContextMenu({} as MouseEvent, message)
    enabled.contextMenuItems.value.find((item) => item.label === '询问 AI')?.command?.({} as never)
    expect(selected).toBe('message-1')

    const disabled = menu(false, () => {})
    disabled.contextMenu.value = { show: () => {} }
    disabled.openContextMenu({} as MouseEvent, message)
    expect(disabled.contextMenuItems.value.some((item) => item.label === '询问 AI')).toBe(false)
  })
})
