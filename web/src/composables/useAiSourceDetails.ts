import { computed, type ComputedRef, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { AiUiMessage } from '../aiUi'
import type { AiThread } from '../types'

export function useAiSourceDetails(messages: Ref<AiUiMessage[]>, activeThread: ComputedRef<AiThread | null>) {
  const route = useRoute()
  const router = useRouter()
  const sourceMessage = computed(() => {
    if (route.name !== 'assistant-sources' || typeof route.params.messageId !== 'string') return null
    return messages.value.find((message) => message.id === route.params.messageId) || null
  })

  function requestedThreadId(): string {
    return typeof route.params.threadId === 'string' ? route.params.threadId : ''
  }

  function openSourceDetails(message: AiUiMessage): void {
    if (!activeThread.value) return
    void router
      .push({
        name: 'assistant-sources',
        params: { threadId: activeThread.value.id, messageId: message.id },
      })
      .catch(() => {})
  }

  async function closeSourceDetails(replace = false): Promise<void> {
    if (route.name !== 'assistant-sources') return
    const navigation = replace ? router.replace({ name: 'assistant' }) : router.push({ name: 'assistant' })
    await navigation.catch(() => {})
  }

  async function leaveSourceDetailsForThread(threadId: string): Promise<void> {
    if (route.name === 'assistant-sources' && requestedThreadId() !== threadId) {
      await closeSourceDetails(true)
    }
  }

  return {
    closeSourceDetails,
    leaveSourceDetailsForThread,
    openSourceDetails,
    requestedThreadId,
    sourceMessage,
  }
}
