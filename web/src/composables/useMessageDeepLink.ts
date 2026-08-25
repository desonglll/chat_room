import { nextTick, watch } from 'vue'
import { useRoute } from 'vue-router'
import { messageIdFromRoute } from '../messageDeepLink'

interface DeepLinkRoute {
  name: unknown
  params: Record<string, unknown>
  query: Record<string, unknown>
}

export function useMessageDeepLink(
  roomId: () => string,
  ready: () => boolean,
  locate: (messageId: string) => Promise<boolean>,
  route: DeepLinkRoute = useRoute(),
): void {
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
