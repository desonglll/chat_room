import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useMessageDeepLink } from './useMessageDeepLink'

interface MessageListTarget {
  scrollToMessage: (messageId: string) => Promise<boolean>
}

export function useRoomMessageNavigation(options: {
  roomId: () => string
  ready: () => boolean
  visible: () => boolean
  authenticated: () => boolean
  messageList: () => MessageListTarget | null
  closeFiles: () => void
}) {
  const searchOpen = ref(false)
  const router = useRouter()

  async function locateMessage(messageId: string): Promise<boolean> {
    options.closeFiles()
    await nextTick()
    return (await options.messageList()?.scrollToMessage(messageId)) || false
  }

  async function locateSearchResult(messageId: string): Promise<void> {
    const roomId = options.roomId()
    if (!roomId) return
    searchOpen.value = false
    await router
      .push({ name: 'room', params: { id: roomId }, query: { message: messageId }, hash: `#message-${messageId}` })
      .catch(() => {})
    await locateMessage(messageId)
  }

  function handleShortcut(event: KeyboardEvent): void {
    if (!options.visible() || !options.authenticated() || !(event.metaKey || event.ctrlKey)) return
    if (event.key.toLowerCase() !== 'f') return
    event.preventDefault()
    searchOpen.value = true
  }

  useMessageDeepLink(options.roomId, options.ready, locateMessage)
  watch(options.roomId, () => (searchOpen.value = false))
  onMounted(() => document.addEventListener('keydown', handleShortcut))
  onBeforeUnmount(() => document.removeEventListener('keydown', handleShortcut))
  return { searchOpen, locateMessage, locateSearchResult }
}
