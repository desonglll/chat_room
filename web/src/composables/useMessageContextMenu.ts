import { ref } from 'vue'
import type { MenuItem } from 'primevue/menuitem'
import type { BroadcastMessage } from '../types'

interface MessageContextMenuOptions {
  currentUserId: () => string
  favoriteMessageIds: () => string[]
  pinnedMessageIds: () => string[]
  canPin: () => boolean
  aiEnabled: () => boolean
  retry: (messageId: string) => void
  reply: (message: BroadcastMessage) => void
  askAi: (messageId: string) => void
  forward: (message: BroadcastMessage) => void
  task: (message: BroadcastMessage) => void
  favorite: (message: BroadcastMessage) => void
  pin: (message: BroadcastMessage) => void
  edit: (message: BroadcastMessage) => void
  recall: (messageId: string) => void
}

export function useMessageContextMenu(options: MessageContextMenuOptions) {
  const contextMenu = ref()
  const contextMenuItems = ref<MenuItem[]>([])

  function copyText(content: string): void {
    void navigator.clipboard.writeText(content).catch(() => window.prompt('复制消息内容', content))
  }

  function openContextMenu(event: MouseEvent, message: BroadcastMessage): void {
    const isOwn = message.sender_id === options.currentUserId()
    const editsPinnedFavorite = Boolean(message.favorite_id && options.pinnedMessageIds().includes(message.message_id))
    const items: MenuItem[] = []
    if (!isSettled(message)) {
      if (message.delivery_state === 'failed') {
        items.push({ label: '重新发送', command: () => options.retry(message.message_id) })
      }
      if (message.content) items.push({ label: '复制', command: () => copyText(message.content) })
      show(event, items)
      return
    }
    if (!message.recalled_at) {
      items.push({ label: '回复', command: () => options.reply(message) })
      if (options.aiEnabled()) items.push({ label: '询问 AI', command: () => options.askAi(message.message_id) })
      if (message.content) items.push({ label: '复制', command: () => copyText(message.content) })
      items.push({ label: '转发', command: () => options.forward(message) })
      items.push({ label: '设为待办', command: () => options.task(message) })
      items.push({
        label: options.favoriteMessageIds().includes(message.message_id) ? '取消收藏' : '收藏',
        command: () => options.favorite(message),
      })
      if (options.canPin()) {
        items.push({
          label: options.pinnedMessageIds().includes(message.message_id) ? '取消置顶' : '置顶',
          command: () => options.pin(message),
        })
      }
    }
    if ((isOwn || editsPinnedFavorite) && message.content && !message.recalled_at) {
      items.push({ label: message.favorite_id ? '编辑收藏' : '编辑', command: () => options.edit(message) })
    }
    if (isOwn && !message.recalled_at) {
      items.push({ label: '撤回', command: () => options.recall(message.message_id) })
    }
    if (isOwn && message.recalled_at) {
      items.push({ label: '重新编辑', command: () => options.edit(message) })
    }
    show(event, items)
  }

  function show(event: MouseEvent, items: MenuItem[]): void {
    if (!items.length) return
    contextMenuItems.value = items
    contextMenu.value?.show(event)
  }

  return { contextMenu, contextMenuItems, isSettled, openContextMenu }
}

function isSettled(message: BroadcastMessage): boolean {
  return !message.delivery_state || message.delivery_state === 'sent'
}
