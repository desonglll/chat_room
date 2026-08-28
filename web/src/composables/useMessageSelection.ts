import { computed, ref, toValue, type MaybeRefOrGetter } from 'vue'
import type { Attachment, DisplayMessage } from '../types'

interface MessageSelectionActions {
  download: (attachments: Attachment[]) => void
  favorite: (messageIds: string[]) => void
  forward: (messageIds: string[]) => void
  assistant: (messageIds: string[]) => void
}

export function useMessageSelection(messages: MaybeRefOrGetter<DisplayMessage[]>, actions: MessageSelectionActions) {
  const selecting = ref(false)
  const selectedMessageIds = ref<string[]>([])
  const selectedAttachments = computed(() =>
    toValue(messages).flatMap((message) =>
      message.type === 'broadcast' && message.attachment && selectedMessageIds.value.includes(message.message_id)
        ? [message.attachment]
        : [],
    ),
  )

  function toggleSelection(messageId: string): void {
    selecting.value = true
    selectedMessageIds.value = selectedMessageIds.value.includes(messageId)
      ? selectedMessageIds.value.filter((id) => id !== messageId)
      : [...selectedMessageIds.value, messageId]
  }

  function closeSelection(): void {
    selecting.value = false
    selectedMessageIds.value = []
  }

  function downloadSelected(): void {
    actions.download(selectedAttachments.value)
  }

  function forwardSelected(): void {
    actions.forward([...selectedMessageIds.value])
    closeSelection()
  }

  function favoriteSelected(): void {
    actions.favorite([...selectedMessageIds.value])
    closeSelection()
  }

  function askSelected(): void {
    actions.assistant([...selectedMessageIds.value])
    closeSelection()
  }

  return {
    closeSelection,
    askSelected,
    downloadSelected,
    favoriteSelected,
    forwardSelected,
    selectedAttachments,
    selectedMessageIds,
    selecting,
    toggleSelection,
  }
}
