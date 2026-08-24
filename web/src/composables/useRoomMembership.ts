import { ref, type Ref } from 'vue'
import { requestRoomJoin } from '../api'
import { saveRoomPassword } from '../roomPasswordVault'
import type { Room, User } from '../types'

interface RoomMembershipOptions {
  rooms: Ref<Room[]>
  selectedRoom: Ref<Room | null>
  currentUser: Ref<User | null>
  token: Ref<string>
  password: Ref<string>
  rememberPasswords: () => boolean
  requireAccount: (action: () => void) => void
  selectRoom: (room: Room) => void
  connect: (room: Room, token: string, userId: string, password: string) => void
  setError: (message: string) => void
  showToast: (message: string) => void
}

export function useRoomMembership(options: RoomMembershipOptions) {
  const joinOpen = ref(false)
  const discoverJoiningId = ref('')
  const discoverError = ref('')

  function openJoinRoom(): void {
    options.requireAccount(() => {
      joinOpen.value = true
    })
  }

  function handleJoinedById(room: Room, password: string): void {
    joinOpen.value = false
    options.rooms.value = options.rooms.value.some((item) => item.id === room.id)
      ? options.rooms.value.map((item) => (item.id === room.id ? room : item))
      : [...options.rooms.value, room]
    options.selectRoom(room)
    options.password.value = password
    if (password) saveRoomPassword(room.id, password, options.rememberPasswords())
    if (room.membership_status === 'active') {
      joinSelectedRoom()
      options.showToast('已加入聊天室')
    } else {
      options.showToast('加入申请已提交')
    }
  }

  function joinSelectedRoom(): void {
    const room = options.selectedRoom.value
    if (!room) return
    if (!options.currentUser.value || !options.token.value) {
      options.requireAccount(() => {})
      return
    }
    if (room.membership_status !== 'active') {
      void handleJoinRequest()
      return
    }
    if (room.has_password && !options.password.value) {
      options.setError('请输入房间密码')
      return
    }
    options.connect(room, options.token.value, options.currentUser.value.id, options.password.value)
  }

  async function joinDiscoveredRoom(room: Room): Promise<void> {
    if (!options.currentUser.value || !options.token.value) {
      options.requireAccount(() => {})
      return
    }
    discoverJoiningId.value = room.id
    discoverError.value = ''
    try {
      const membership = await requestRoomJoin(room.id, options.token.value, '')
      const updated = {
        ...room,
        membership_status: membership.status,
        membership_role: membership.role,
      }
      options.rooms.value = options.rooms.value.some((item) => item.id === room.id)
        ? options.rooms.value.map((item) => (item.id === room.id ? updated : item))
        : [...options.rooms.value, updated]
      if (membership.status === 'active') {
        options.selectRoom(updated)
        joinSelectedRoom()
        options.showToast('已加入聊天室')
      } else {
        options.showToast('加入申请已提交')
      }
    } catch (caught) {
      discoverError.value = caught instanceof Error ? caught.message : '加入失败'
    } finally {
      discoverJoiningId.value = ''
    }
  }

  async function handleJoinRequest(): Promise<void> {
    const room = options.selectedRoom.value
    if (!room || !options.token.value) {
      options.requireAccount(() => {})
      return
    }
    if (room.has_password && !options.password.value) {
      options.setError('请输入房间密码')
      return
    }
    options.setError('')
    try {
      const membership = await requestRoomJoin(room.id, options.token.value, options.password.value)
      const updated = {
        ...room,
        membership_status: membership.status,
        membership_role: membership.role,
      }
      options.rooms.value = options.rooms.value.map((item) => (item.id === room.id ? updated : item))
      options.selectedRoom.value = updated
      if (membership.status === 'active') {
        joinSelectedRoom()
        options.showToast('已加入聊天室')
      } else {
        options.showToast('加入申请已提交')
      }
    } catch (caught) {
      options.setError(caught instanceof Error ? caught.message : '加入申请失败')
    }
  }

  return {
    discoverError,
    discoverJoiningId,
    handleJoinedById,
    handleJoinRequest,
    joinDiscoveredRoom,
    joinOpen,
    joinSelectedRoom,
    openJoinRoom,
  }
}
