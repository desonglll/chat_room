import { computed, onMounted, ref, watch, type Ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { DEFAULT_MAX_UPLOAD_BYTES, getCurrentUser, getPublicConfig, listRooms, logoutUser } from '../api'
import { storageGet, storageSet } from '../browserStorage'
import { clearBootstrapSnapshot, readBootstrapSnapshot, writeBootstrapSnapshot } from '../bootstrapSnapshot'
import { useDelayedVisibility } from './useDelayedVisibility'
import type { AiRuntimeStatus, AuthSession, Room, User } from '../types'

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

export function shouldReconnectRestoredRoom(
  routeName: unknown,
  selectedRoomId = '',
  routeRoomId = selectedRoomId,
): boolean {
  return routeName === 'room' && Boolean(selectedRoomId) && selectedRoomId === routeRoomId
}

export function useAppBootstrap(options: AppBootstrapOptions) {
  const route = useRoute()
  const router = useRouter()
  const routeRoomId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''))
  const initialToken = storageGet(window.localStorage, SESSION_TOKEN_KEY)
  const snapshot = readBootstrapSnapshot(window.sessionStorage, Boolean(initialToken))
  const rooms = ref<Room[]>(snapshot?.rooms || [])
  const selectedRoom = ref<Room | null>(null)
  const sessionToken = ref(initialToken)
  const currentUser = ref<User | null>(snapshot?.user || null)
  const booting = ref(true)
  const refreshingRooms = ref(Boolean(snapshot))
  const coldStartPending = computed(() => booting.value && rooms.value.length === 0)
  const showColdSkeleton = useDelayedVisibility(coldStartPending)
  const networkError = ref('')
  const maxUploadBytes = ref(DEFAULT_MAX_UPLOAD_BYTES)
  const capabilities = ref({ ai: false })
  const aiStatus = ref<AiRuntimeStatus>('disabled')
  let restoreAttempted = false
  let restoredFromSnapshot = false

  function restoreCachedSelection(): void {
    if (restoreAttempted || !routeRoomId.value) return
    const restored = rooms.value.find((room) => room.id === routeRoomId.value)
    if (!restored) return
    restoreAttempted = true
    restoredFromSnapshot = true
    options.selectRoom(restored, false)
  }

  async function loadRoomList(): Promise<void> {
    if (!booting.value) refreshingRooms.value = true
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
        if (restored) options.selectRoom(restored, shouldReconnectRestoredRoom(route.name, restored.id, activeId))
        else if (activeId && !sessionToken.value) void router.replace({ name: 'home' }).catch(() => {})
      }
    } catch (caught) {
      networkError.value = caught instanceof Error ? caught.message : '无法读取房间列表'
    } finally {
      booting.value = false
      refreshingRooms.value = false
    }
  }

  async function loadRuntimeConfig(): Promise<void> {
    try {
      const config = await getPublicConfig()
      if (Number.isSafeInteger(config.max_upload_bytes) && config.max_upload_bytes > 0) {
        maxUploadBytes.value = config.max_upload_bytes
      }
      capabilities.value.ai = Boolean(config.ai_enabled)
      aiStatus.value = config.ai_status
    } catch {
      maxUploadBytes.value = DEFAULT_MAX_UPLOAD_BYTES
      capabilities.value = { ai: false }
      aiStatus.value = 'disabled'
    }
  }

  async function restoreSession(): Promise<void> {
    if (!sessionToken.value) return
    try {
      currentUser.value = await getCurrentUser(sessionToken.value)
    } catch {
      sessionToken.value = ''
      currentUser.value = null
      rooms.value = []
      clearBootstrapSnapshot(window.sessionStorage)
      storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
    }
  }

  async function handleAuthenticated(session: AuthSession): Promise<void> {
    clearBootstrapSnapshot(window.sessionStorage)
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
    clearBootstrapSnapshot(window.sessionStorage)
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
    clearBootstrapSnapshot(window.sessionStorage)
    storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
    options.afterAccountDeleted()
    await loadRoomList()
    options.showToast('账户已注销')
  }

  onMounted(async () => {
    await Promise.all([restoreSession(), loadRuntimeConfig()])
    if (sessionToken.value) options.connectUnread(sessionToken.value)
    await loadRoomList()
    if (
      restoredFromSnapshot &&
      selectedRoom.value &&
      shouldReconnectRestoredRoom(route.name, selectedRoom.value.id, routeRoomId.value)
    ) {
      options.selectRoom(selectedRoom.value, true)
    }
  })

  watch([sessionToken, currentUser, rooms], () => {
    if (booting.value || !sessionToken.value) return
    writeBootstrapSnapshot(window.sessionStorage, currentUser.value, rooms.value)
  })

  return {
    capabilities,
    aiStatus,
    currentUser,
    handleAccountDeleted,
    handleAuthenticated,
    handleLogout,
    refreshingRooms,
    loadRoomList,
    maxUploadBytes,
    networkError,
    rooms,
    routeRoomId,
    restoreCachedSelection,
    selectedRoom,
    sessionToken,
    showColdSkeleton,
  }
}
