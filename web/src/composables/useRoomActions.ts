import type { Ref } from 'vue'
import { leaveRoom } from '../api'
import { conversationToRoom } from '../conversationState'
import { clearRoomPassword, readRoomPassword, saveRoomPassword } from '../roomPasswordVault'
import { canAutoConnectRoom } from '../roomMembershipState'
import type { ChatStatus, ConversationSummary, Room, RoomUpdateResult, User } from '../types'

interface RoomActionOptions {
  rooms: Ref<Room[]>
  selectedRoom: Ref<Room | null>
  currentUser: Ref<User | null>
  token: Ref<string>
  password: Ref<string>
  rememberPasswords: () => boolean
  mobileView: Ref<'rooms' | 'chat'>
  createOpen: Ref<boolean>
  manageOpen: Ref<boolean>
  newConversationOpen: Ref<boolean>
  routeName: () => unknown
  routeRoomId: Ref<string>
  chatStatus: () => ChatStatus
  closeChat: () => void
  joinSelectedRoom: () => void
  showChatPage: () => void
  requestAuthentication: () => void
  navigateHome: () => void
  refreshRooms: () => Promise<void>
  refreshConversations: () => Promise<void>
  removeConversation: (roomId: string) => void
  upsertConversation: (conversation: ConversationSummary) => void
  showSuccess: (message: string) => void
  showError: (message: string) => void
}

export function useRoomActions(options: RoomActionOptions) {
  function requestCreateRoom(): void {
    if (!options.currentUser.value) return options.requestAuthentication()
    options.createOpen.value = true
  }

  function clearSelection(navigate = true): void {
    options.closeChat()
    options.selectedRoom.value = null
    options.password.value = ''
    options.manageOpen.value = false
    options.mobileView.value = 'rooms'
    if (navigate && options.routeName() !== 'home') options.navigateHome()
  }

  function selectRoom(room: Room, autoConnect = false): void {
    options.closeChat()
    options.selectedRoom.value = room
    options.password.value = readRoomPassword(room.id, options.rememberPasswords())
    const reconnect =
      autoConnect && canAutoConnectRoom(room, options.currentUser.value, options.token.value, options.password.value)
    const preserveRoute = reconnect && options.routeName() === 'room' && options.routeRoomId.value === room.id
    if (!preserveRoute) options.showChatPage()
    options.mobileView.value = 'chat'
    if (reconnect) options.joinSelectedRoom()
  }

  function selectConversation(conversation: ConversationSummary, autoConnect = true): void {
    options.newConversationOpen.value = false
    options.upsertConversation(conversation)
    selectRoom(conversationToRoom(conversation), autoConnect)
  }

  function handleCreated(room: Room, password: string): void {
    options.createOpen.value = false
    options.rooms.value = [...options.rooms.value, room]
    if (password) saveRoomPassword(room.id, password, options.rememberPasswords())
    selectRoom(room)
    options.password.value = password
    options.showSuccess('聊天室已创建')
    void options.refreshConversations()
  }

  function handleUpdated(result: RoomUpdateResult): void {
    const hadSession = ['connecting', 'online', 'offline'].includes(options.chatStatus())
    options.rooms.value = options.rooms.value.map((room) => (room.id === result.room.id ? result.room : room))
    options.selectedRoom.value = result.room
    options.password.value = result.password
    saveRoomPassword(result.room.id, result.password, options.rememberPasswords())
    options.manageOpen.value = false
    if (result.passwordChanged) {
      selectRoom(result.room, hadSession)
      options.password.value = result.password
    }
    options.showSuccess('聊天室设置已保存')
    void options.refreshConversations()
  }

  async function handleDeleted(roomId: string): Promise<void> {
    options.manageOpen.value = false
    clearRoomPassword(roomId)
    clearSelection()
    await Promise.all([options.refreshRooms(), options.refreshConversations()])
    options.showSuccess('聊天室已删除')
  }

  async function handleLeaveRoom(room: Room | null = options.selectedRoom.value): Promise<void> {
    if (!room || !options.token.value) return
    try {
      await leaveRoom(room.id, options.token.value)
      options.removeConversation(room.id)
      if (options.selectedRoom.value?.id === room.id) clearSelection()
      await Promise.all([options.refreshRooms(), options.refreshConversations()])
      options.showSuccess('已退出聊天室')
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '退出失败')
    }
  }

  function openRoomManage(room: Room): void {
    selectRoom(room)
    options.manageOpen.value = true
  }

  return {
    clearSelection,
    handleCreated,
    handleDeleted,
    handleLeaveRoom,
    handleUpdated,
    openRoomManage,
    requestCreateRoom,
    selectConversation,
    selectRoom,
  }
}
