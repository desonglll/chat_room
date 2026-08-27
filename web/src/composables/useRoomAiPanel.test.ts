import { describe, expect, test } from 'bun:test'
import { nextTick, ref } from 'vue'
import type { BroadcastMessage, DisplayMessage } from '../types'
import { useRoomAiPanel } from './useRoomAiPanel'

function message(id: string): BroadcastMessage {
  return {
    type: 'broadcast',
    message_id: id,
    sender_id: 'user-1',
    sender: 'Ada',
    sender_avatar: '',
    content: `body-${id}`,
    attachment: null,
    reply_to: null,
    recalled_at: null,
    edited_at: null,
    timestamp: '2026-08-27T10:00:00Z',
    forwarded_from: null,
    reactions: [],
  }
}

describe('room AI panel', () => {
  test('opens with selected messages and clears private context when the room changes', async () => {
    const messages = ref<DisplayMessage[]>([message('message-1'), message('message-2')])
    const roomId = ref('room-1')
    const panel = useRoomAiPanel(messages, roomId)

    panel.handleAssistant(['message-2', 'message-1'])
    expect(panel.aiPanelOpen.value).toBe(true)
    expect(panel.aiContextMessages.value.map((item) => item.messageId)).toEqual(['message-2', 'message-1'])

    roomId.value = 'room-2'
    await nextTick()
    expect(panel.aiPanelOpen.value).toBe(false)
    expect(panel.aiContextMessages.value).toEqual([])
  })

  test('catch-up opens without reusing selected-message context', () => {
    const panel = useRoomAiPanel(ref<DisplayMessage[]>([message('message-1')]), ref('room-1'))
    panel.handleAssistant(['message-1'])
    panel.requestCatchUp()

    expect(panel.aiPanelOpen.value).toBe(true)
    expect(panel.catchUpRequest.value).toBe(1)
    expect(panel.aiContextMessages.value).toEqual([])
  })
})
