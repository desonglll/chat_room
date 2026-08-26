import type { Room } from './types'

interface RoomSystemEventHandlers {
  room: () => Room | null
  managing: () => boolean
  closeChat: () => void
  clearPassword: (roomId: string) => void
  clearSelection: () => void
  refreshConversations: () => void
  refreshRooms: () => void
  showToast: (message: string) => void
}

export function createRoomSystemEventHandler(handlers: RoomSystemEventHandlers) {
  return (content: string): void => {
    if (content.startsWith('room renamed to ')) {
      void Promise.all([handlers.refreshRooms(), handlers.refreshConversations()])
      return
    }
    if (content === 'room password changed' && handlers.room() && !handlers.managing()) {
      handlers.clearPassword(handlers.room()!.id)
      window.setTimeout(handlers.closeChat, 0)
      return
    }
    if (content === 'room deleted' && !handlers.managing()) {
      window.setTimeout(() => {
        handlers.clearSelection()
        handlers.refreshRooms()
        handlers.refreshConversations()
        handlers.showToast('聊天室已删除')
      }, 0)
      return
    }
    if (content === 'membership removed' || content === 'membership left') {
      handlers.closeChat()
      handlers.refreshRooms()
      handlers.refreshConversations()
    }
  }
}
