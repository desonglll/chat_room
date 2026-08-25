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
  watch(
    [() => messageIdFromRoute(route.name, route.params.id, route.query.message, roomId()), ready],
    async ([messageId, isReady]) => {
      if (!messageId || !isReady || handled === messageId) return
      handled = messageId
      await nextTick()
      await locate(messageId)
    },
    { immediate: true, flush: 'post' },
  )
}
