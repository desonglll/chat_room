import { ref } from 'vue'
import type { Router } from 'vue-router'
import type { BroadcastMessage } from '../types'

interface ChatPanelMessageCallbacks {
  edit: (messageId: string, content: string) => void
  recall: (messageId: string) => void
  send: (content: string, replyTo: string) => void
  upload: (files: File[], content: string, replyTo: string, isSensitive: boolean) => void
}

export function useChatPanelMessageActions(router: Router, callbacks: ChatPanelMessageCallbacks) {
  const replyingTo = ref<BroadcastMessage | null>(null)
  const editingTo = ref<BroadcastMessage | null>(null)

  function resetTargets(): void {
    replyingTo.value = null
    editingTo.value = null
  }

  function sendMessage(content: string, replyTo: string): void {
    callbacks.send(content, replyTo)
    replyingTo.value = null
  }

  function uploadFiles(files: File[], content: string, replyTo: string, isSensitive: boolean): void {
    callbacks.upload(files, content, replyTo, isSensitive)
    replyingTo.value = null
  }

  function recallMessage(messageId: string): void {
    if (replyingTo.value?.message_id === messageId) replyingTo.value = null
    callbacks.recall(messageId)
  }

  function startReply(message: BroadcastMessage): void {
    editingTo.value = null
    replyingTo.value = message
  }

  function startEdit(message: BroadcastMessage): void {
    if (message.favorite_id) {
      void router.push({ name: 'favorites', query: { edit: message.favorite_id } }).catch(() => {})
      return
    }
    replyingTo.value = null
    editingTo.value = message
  }

  function editMessage(messageId: string, content: string): void {
    callbacks.edit(messageId, content)
    editingTo.value = null
  }

  return {
    editMessage,
    editingTo,
    recallMessage,
    replyingTo,
    resetTargets,
    sendMessage,
    startEdit,
    startReply,
    uploadFiles,
  }
}
