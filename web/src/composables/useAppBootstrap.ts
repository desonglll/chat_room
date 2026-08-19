import { computed, onMounted, ref, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { DEFAULT_MAX_UPLOAD_BYTES, getCurrentUser, getPublicConfig, listRooms, logoutUser } from '../api'
import { storageGet, storageSet } from '../browserStorage'
import type { AuthSession, Room, User } from '../types'

const SESSION_TOKEN_KEY = 'chat-room.session-token'

interface AppBootstrapOptions {
  closeChat: () => void
  closeUnread: () => void
  connectUnread: (token: string) => void
  clearSelection: () => void
  selectRoom: (room: Room, autoConnect: boolean) => void
  joinSelectedRoom: () => void
  afterAccountDeleted: () => void
  showToast: (message: string) => void
}

export function shouldReconnectRestoredRoom(routeName: unknown): boolean {
  return routeName === 'room'
}

export function useAppBootstrap(options: AppBootstrapOptions) {
  const route = useRoute()
  const router = useRouter()
  const routeRoomId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''))
  const rooms = ref<Room[]>([])
  const selectedRoom = ref<Room | null>(null)
  const sessionToken = ref(storageGet(window.localStorage, SESSION_TOKEN_KEY))
  const currentUser = ref<User | null>(null)
  const loading = ref(true)
  const networkError = ref('')
  const maxUploadBytes = ref(DEFAULT_MAX_UPLOAD_BYTES)
  const aiEnabled = ref(false)
  let restoreAttempted = false

  async function loadRoomList(): Promise<void> {
    loading.value = true
    try {
      const nextRooms = await listRooms(sessionToken.value)
      rooms.value = nextRooms
      networkError.value = ''
      if (selectedRoom.value) {
        const updated = nextRooms.find((room) => room.id === selectedRoom.value?.id)
        if (updated) selectedRoom.value = updated
        else options.clearSelection()
      }
      if (!restoreAttempted) {
        restoreAttempted = true
        const activeId = routeRoomId.value
        const restored = nextRooms.find((room) => room.id === activeId)
        if (restored) options.selectRoom(restored, shouldReconnectRestoredRoom(route.name))
        else if (activeId) void router.replace({ name: 'home' }).catch(() => {})
      }
    } catch (caught) {
      networkError.value = caught instanceof Error ? caught.message : '无法读取房间列表'
    } finally {
      loading.value = false
    }
  }

  async function loadRuntimeConfig(): Promise<void> {
    try {
      const config = await getPublicConfig()
      if (Number.isSafeInteger(config.max_upload_bytes) && config.max_upload_bytes > 0) {
        maxUploadBytes.value = config.max_upload_bytes
      }
      aiEnabled.value = Boolean(config.ai_enabled)
    } catch {
      maxUploadBytes.value = DEFAULT_MAX_UPLOAD_BYTES
      aiEnabled.value = false
    }
  }

  async function restoreSession(): Promise<void> {
    if (!sessionToken.value) return
    try {
      currentUser.value = await getCurrentUser(sessionToken.value)
    } catch {
      sessionToken.value = ''
      currentUser.value = null
      storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
    }
  }

  async function handleAuthenticated(session: AuthSession): Promise<void> {
    sessionToken.value = session.token
    currentUser.value = session.user
    storageSet(window.localStorage, SESSION_TOKEN_KEY, session.token)
    options.connectUnread(session.token)
    await loadRoomList()
    options.showToast(`已登录为 ${session.user.username}`)
    if (selectedRoom.value?.membership_status === 'active') options.joinSelectedRoom()
  }

  async function handleLogout(): Promise<void> {
    const token = sessionToken.value
    options.closeChat()
    sessionToken.value = ''
    currentUser.value = null
    options.closeUnread()
    storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
    if (token) {
      try {
        await logoutUser(token)
      } catch {
        // The local session is already cleared.
      }
    }
    await loadRoomList()
    options.showToast('已退出登录')
  }

  async function handleAccountDeleted(): Promise<void> {
    options.closeChat()
    options.closeUnread()
    sessionToken.value = ''
    currentUser.value = null
    storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
    options.afterAccountDeleted()
    await loadRoomList()
    options.showToast('账户已注销')
  }

  onMounted(async () => {
    await Promise.all([restoreSession(), loadRuntimeConfig()])
    if (sessionToken.value) options.connectUnread(sessionToken.value)
    await loadRoomList()
  })

  return {
    aiEnabled,
    currentUser,
    handleAccountDeleted,
    handleAuthenticated,
    handleLogout,
    loading,
    loadRoomList,
    maxUploadBytes,
    networkError,
    rooms,
    routeRoomId,
    selectedRoom,
    sessionToken,
  }
}
