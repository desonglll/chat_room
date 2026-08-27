import { ref, toValue, watch, type MaybeRefOrGetter } from 'vue'
import { selectedAiMessages, type AiSelectedMessage } from '../aiSelectedContext'
import type { DisplayMessage } from '../types'

export function useRoomAiPanel(
  messages: MaybeRefOrGetter<DisplayMessage[]>,
  roomId: MaybeRefOrGetter<string | undefined>,
) {
  const aiPanelOpen = ref(false)
  const catchUpRequest = ref(0)
  const aiContextMessages = ref<AiSelectedMessage[]>([])

  function toggleAssistant(): void {
    if (aiPanelOpen.value) aiPanelOpen.value = false
    else {
      aiContextMessages.value = []
      aiPanelOpen.value = true
    }
  }

  function askAssistant(messageIds: string[]): void {
    aiContextMessages.value = selectedAiMessages(toValue(messages), messageIds)
    aiPanelOpen.value = true
  }

  function handleAssistant(messageIds?: string[]): void {
    if (messageIds?.length) askAssistant(messageIds)
    else toggleAssistant()
  }

  function requestCatchUp(): void {
    aiContextMessages.value = []
    aiPanelOpen.value = true
    catchUpRequest.value += 1
  }

  function clearAiContext(): void {
    aiContextMessages.value = []
  }

  watch(
    () => toValue(roomId),
    () => {
      aiContextMessages.value = []
      aiPanelOpen.value = false
    },
  )

  return { aiContextMessages, aiPanelOpen, catchUpRequest, clearAiContext, handleAssistant, requestCatchUp }
}
