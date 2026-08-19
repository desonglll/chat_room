import { ref, watch, type Ref } from 'vue'
import { listRoomMessages, storedMessageToBroadcast } from '../api'
import type { DisplayMessage, Room } from '../types'

interface RoomHistoryOptions {
  room: Ref<Room | null>
  token: Ref<string>
  password: Ref<string>
  messages: Ref<DisplayMessage[]>
  prepend: (messages: ReturnType<typeof storedMessageToBroadcast>[]) => void
}

export function useRoomHistory(options: RoomHistoryOptions) {
  const loading = ref(false)
  const hasMore = ref(true)

  watch(
    () => options.room.value?.id,
    () => {
      hasMore.value = true
      loading.value = false
    },
  )

  async function loadOlder(): Promise<void> {
    const room = options.room.value
    const oldest = options.messages.value.find((message) => message.type === 'broadcast')
    if (!room || !options.token.value || !oldest || loading.value || !hasMore.value) return

    loading.value = true
    try {
      const page = await listRoomMessages(room.id, options.token.value, options.password.value, oldest.message_id, 50)
      hasMore.value = page.length === 50
      options.prepend(page.map(storedMessageToBroadcast))
    } catch {
      // Scrolling to the top again retries the same page.
    } finally {
      loading.value = false
    }
  }

  return { hasMore, loadOlder, loading }
}
