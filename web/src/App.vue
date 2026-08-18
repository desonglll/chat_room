<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import AuthDialog from './components/AuthDialog.vue'
import ChatPanel from './components/ChatPanel.vue'
import CreateRoomDialog from './components/CreateRoomDialog.vue'
import ManageRoomDialog from './components/ManageRoomDialog.vue'
import RoomSidebar from './components/RoomSidebar.vue'
import { getCurrentUser, listRooms, logoutUser, uploadAttachment } from './api'
import { useChatSocket } from './composables/useChatSocket'
import type { AuthSession, Room, RoomUpdateResult, User } from './types'

const SESSION_TOKEN_KEY = 'chat-room.session-token'
const ACTIVE_ROOM_KEY = 'chat-room.active-room'
const passwordKey = (roomId: string) => `chat-room.password.${roomId}`

function storageGet(storage: Storage, key: string): string {
  try { return storage.getItem(key) || '' } catch { return '' }
}

function storageSet(storage: Storage, key: string, value: string): void {
  try {
    if (value) storage.setItem(key, value)
    else storage.removeItem(key)
  } catch {
    // The app remains usable when browser storage is disabled.
  }
}

const rooms = ref<Room[]>([])
const selectedRoom = ref<Room | null>(null)
const sessionToken = ref(storageGet(window.localStorage, SESSION_TOKEN_KEY))
const currentUser = ref<User | null>(null)
const roomPassword = ref('')
const loading = ref(true)
const networkError = ref('')
const createOpen = ref(false)
const manageOpen = ref(false)
const authOpen = ref(false)
const mobileView = ref<'rooms' | 'chat'>('rooms')
const toast = ref('')
const uploading = ref(false)
let restoreAttempted = false
let toastTimer: number | undefined

function handleSystemEvent(content: string): void {
  if (content.startsWith('room renamed to ')) void loadRoomList()
  if (content === 'room password changed' && selectedRoom.value && !manageOpen.value) {
    storageSet(window.sessionStorage, passwordKey(selectedRoom.value.id), '')
    roomPassword.value = ''
    window.setTimeout(() => chat.close({ preserveMessages: true }), 0)
  }
  if (content === 'room deleted') {
    if (manageOpen.value) return
    window.setTimeout(() => {
      clearSelection()
      void loadRoomList()
      showToast('聊天室已删除')
    }, 0)
  }
}

const chat = useChatSocket(handleSystemEvent)
const selectedId = computed(() => selectedRoom.value?.id)

watch(chat.authenticated, (online) => {
  if (online && selectedRoom.value?.has_password) {
    storageSet(window.sessionStorage, passwordKey(selectedRoom.value.id), roomPassword.value)
  }
})

function showToast(message: string): void {
  toast.value = message
  window.clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => { toast.value = '' }, 2400)
}

function clearSelection(): void {
  chat.close()
  selectedRoom.value = null
  roomPassword.value = ''
  manageOpen.value = false
  mobileView.value = 'rooms'
  storageSet(window.sessionStorage, ACTIVE_ROOM_KEY, '')
}

function selectRoom(room: Room, autoConnect = false): void {
  chat.close()
  selectedRoom.value = room
  roomPassword.value = storageGet(window.sessionStorage, passwordKey(room.id))
  mobileView.value = 'chat'
  storageSet(window.sessionStorage, ACTIVE_ROOM_KEY, room.id)
  if (autoConnect && currentUser.value && sessionToken.value && (!room.has_password || roomPassword.value)) {
    joinSelectedRoom()
  }
}

async function loadRoomList(): Promise<void> {
  loading.value = true
  try {
    const nextRooms = await listRooms()
    rooms.value = nextRooms
    networkError.value = ''
    if (selectedRoom.value) {
      const updated = nextRooms.find((room) => room.id === selectedRoom.value?.id)
      if (updated) selectedRoom.value = updated
      else clearSelection()
    }
    if (!restoreAttempted) {
      restoreAttempted = true
      const activeId = storageGet(window.sessionStorage, ACTIVE_ROOM_KEY)
      const restored = nextRooms.find((room) => room.id === activeId)
      if (restored) selectRoom(restored, true)
      else if (activeId) storageSet(window.sessionStorage, ACTIVE_ROOM_KEY, '')
    }
  } catch (caught) {
    networkError.value = caught instanceof Error ? caught.message : '无法读取房间列表'
  } finally {
    loading.value = false
  }
}

function joinSelectedRoom(): void {
  if (!selectedRoom.value) return
  if (!currentUser.value || !sessionToken.value) {
    authOpen.value = true
    return
  }
  if (selectedRoom.value.has_password && !roomPassword.value) {
    chat.error.value = '请输入房间密码'
    return
  }
  chat.connect(selectedRoom.value, sessionToken.value, currentUser.value.id, roomPassword.value)
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

function handleAuthenticated(session: AuthSession): void {
  sessionToken.value = session.token
  currentUser.value = session.user
  storageSet(window.localStorage, SESSION_TOKEN_KEY, session.token)
  authOpen.value = false
  showToast(`已登录为 ${session.user.username}`)
  if (selectedRoom.value && (!selectedRoom.value.has_password || roomPassword.value)) {
    joinSelectedRoom()
  }
}

async function handleLogout(): Promise<void> {
  const token = sessionToken.value
  chat.close()
  sessionToken.value = ''
  currentUser.value = null
  storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
  if (token) {
    try { await logoutUser(token) } catch { /* The local session is already cleared. */ }
  }
  showToast('已退出登录')
}

function handleCreated(room: Room, password: string): void {
  createOpen.value = false
  rooms.value = [...rooms.value, room]
  if (password) storageSet(window.sessionStorage, passwordKey(room.id), password)
  selectRoom(room)
  roomPassword.value = password
  showToast('聊天室已创建')
}

function handleUpdated(result: RoomUpdateResult): void {
  const previousStatus = chat.status.value
  const hadSession = ['connecting', 'online', 'offline'].includes(previousStatus)
  rooms.value = rooms.value.map((room) => room.id === result.room.id ? result.room : room)
  selectedRoom.value = result.room
  roomPassword.value = result.password
  storageSet(window.sessionStorage, passwordKey(result.room.id), result.password)
  manageOpen.value = false
  if (result.passwordChanged) {
    selectRoom(result.room, hadSession)
    roomPassword.value = result.password
  }
  showToast('聊天室设置已保存')
}

async function handleDeleted(roomId: string): Promise<void> {
  manageOpen.value = false
  storageSet(window.sessionStorage, passwordKey(roomId), '')
  clearSelection()
  await loadRoomList()
  showToast('聊天室已删除')
}

async function handleUpload(files: File[]): Promise<void> {
  const room = selectedRoom.value
  if (!room || !sessionToken.value || !chat.authenticated.value || uploading.value) return
  uploading.value = true
  try {
    for (const file of files) {
      const message = await uploadAttachment(
        room.id,
        file,
        sessionToken.value,
        roomPassword.value,
      )
      if (selectedRoom.value?.id === room.id) chat.appendBroadcast(message)
    }
  } catch (caught) {
    showToast(caught instanceof Error ? caught.message : '文件上传失败')
  } finally {
    uploading.value = false
  }
}

onMounted(async () => {
  await restoreSession()
  await loadRoomList()
})
</script>

<template>
  <div class="app-shell" data-testid="app-shell">
    <div v-if="networkError" class="network-banner" role="alert">
      <span>{{ networkError }}</span>
      <button type="button" @click="loadRoomList">重试</button>
    </div>

    <RoomSidebar
      :rooms="rooms"
      :selected-id="selectedId"
      :user="currentUser"
      :loading="loading"
      :visible="mobileView === 'rooms'"
      @select="selectRoom"
      @refresh="loadRoomList"
      @create="createOpen = true"
      @authenticate="authOpen = true"
      @logout="handleLogout"
    />
    <ChatPanel
      :room="selectedRoom"
      :user="currentUser"
      :password="roomPassword"
      :status="chat.status.value"
      :status-label="chat.statusLabel.value"
      :authenticated="chat.authenticated.value"
      :error="chat.error.value"
      :messages="chat.messages.value"
      :current-user-id="chat.currentUserId.value"
      :visible="mobileView === 'chat'"
      :uploading="uploading"
      @back="mobileView = 'rooms'"
      @manage="manageOpen = true"
      @leave="selectedRoom && selectRoom(selectedRoom)"
      @join="joinSelectedRoom"
      @authenticate="authOpen = true"
      @send="chat.send"
      @upload="handleUpload"
      @update:password="roomPassword = $event"
    />

    <AuthDialog :open="authOpen" @close="authOpen = false" @authenticated="handleAuthenticated" />
    <CreateRoomDialog :open="createOpen" @close="createOpen = false" @created="handleCreated" />
    <ManageRoomDialog
      :open="manageOpen"
      :room="selectedRoom"
      :credential="roomPassword"
      @close="manageOpen = false"
      @updated="handleUpdated"
      @deleted="handleDeleted"
    />

    <div v-if="toast" class="toast" role="status">{{ toast }}</div>
  </div>
</template>
