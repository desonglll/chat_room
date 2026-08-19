import type { Ref } from 'vue'
import {
  appendUploadMessage,
  completeUploadMessage,
  removeUploadMessage,
  updateUploadMessage,
} from '../chatUploadMessages'
import type { BroadcastMessage, DisplayMessage, UploadMessage } from '../types'

export function useChatUploadMessages(
  messages: Ref<DisplayMessage[]>,
  appendBroadcast: (message: BroadcastMessage) => void,
) {
  function appendUpload(message: UploadMessage): void {
    messages.value = appendUploadMessage(messages.value, message)
  }

  function updateUpload(key: string, patch: Partial<UploadMessage>): void {
    messages.value = updateUploadMessage(messages.value, key, patch)
  }

  function completeUpload(key: string, message: BroadcastMessage): void {
    const completed = completeUploadMessage(messages.value, key, message)
    if (completed) messages.value = completed
    else appendBroadcast(message)
  }

  function removeUpload(key: string): void {
    messages.value = removeUploadMessage(messages.value, key)
  }

  return { appendUpload, completeUpload, removeUpload, updateUpload }
}
