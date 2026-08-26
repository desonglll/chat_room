import { ref, watch, type Ref } from 'vue'
import { storedMessageToBroadcast } from '../api'
import { listRoomMessageContext, listRoomMessages } from '../roomMessagesApi'
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
  let generation = 0
  let activeLoad: Promise<number> | null = null

  watch(
    () => options.room.value?.id,
    () => {
      generation += 1
      hasMore.value = true
      loading.value = false
      activeLoad = null
    },
  )

  async function fetchOlder(): Promise<number> {
    const room = options.room.value
    const oldest = options.messages.value.find((message) => message.type === 'broadcast')
    if (!room || !options.token.value || !oldest || !hasMore.value) return 0

    const requestGeneration = generation
    loading.value = true
    try {
      const page = await listRoomMessages(room.id, options.token.value, options.password.value, oldest.message_id, 50)
      if (requestGeneration !== generation || options.room.value?.id !== room.id) return 0
      hasMore.value = page.length === 50
      options.prepend(page.map(storedMessageToBroadcast))
      return page.length
    } catch {
      // Scrolling to the top again retries the same page.
      return 0
    } finally {
      if (requestGeneration === generation) loading.value = false
    }
  }

  function loadOlder(): Promise<number> {
    if (activeLoad) return activeLoad
    const request = fetchOlder()
    activeLoad = request
    void request.finally(() => {
      if (activeLoad === request) activeLoad = null
    })
    return request
  }

  async function ensureMessage(messageId: string): Promise<boolean> {
    const containsMessage = () =>
      options.messages.value.some((message) => message.type === 'broadcast' && message.message_id === messageId)
    if (containsMessage()) return true
    const room = options.room.value
    if (!room || !options.token.value) return false
    try {
      const context = await listRoomMessageContext(room.id, messageId, options.token.value, options.password.value)
      options.prepend(context.map(storedMessageToBroadcast))
    } catch {
      return false
    }
    return containsMessage()
  }

  return { ensureMessage, hasMore, loadOlder, loading }
}
