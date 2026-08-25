import { nextTick, watch } from 'vue'
import { useRoute } from 'vue-router'
import { messageIdFromRoute } from '../messageDeepLink'

export function useMessageDeepLink(
  roomId: () => string,
  ready: () => boolean,
  locate: (messageId: string) => Promise<boolean>,
): void {
  const route = useRoute()
  let handled = ''
  let locating = ''
  const targetMessageId = () => messageIdFromRoute(route.name, route.params.id, route.query.message, roomId())
  watch(
    [targetMessageId, ready],
    async ([messageId, isReady]) => {
      if (!messageId) {
        handled = ''
        return
      }
      if (!isReady || handled === messageId || locating === messageId) return
      locating = messageId
      try {
        await nextTick()
        const found = await locate(messageId)
        if (found && targetMessageId() === messageId) handled = messageId
      } finally {
        if (locating === messageId) locating = ''
      }
    },
    { immediate: true, flush: 'post' },
  )
}
