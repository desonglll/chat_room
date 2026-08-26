import { ref } from 'vue'

export function useMessageForwarding(afterForwarded: () => void) {
  const forwardOpen = ref(false)
  const forwardMessageIds = ref<string[]>([])

  function openForward(messageIds: string[]): void {
    if (!messageIds.length) return
    forwardMessageIds.value = messageIds
    forwardOpen.value = true
  }

  function handleForwarded(): void {
    forwardOpen.value = false
    afterForwarded()
  }

  return { forwardMessageIds, forwardOpen, handleForwarded, openForward }
}
