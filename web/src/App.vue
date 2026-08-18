<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Toast from 'primevue/toast'
import { useToast } from 'primevue/usetoast'
import AuthDialog from './components/AuthDialog.vue'
import ChatPanel from './components/ChatPanel.vue'
import CreateRoomDialog from './components/CreateRoomDialog.vue'
import ManageRoomDialog from './components/ManageRoomDialog.vue'
import PreferencesDialog from './components/PreferencesDialog.vue'
import RoomSidebar from './components/RoomSidebar.vue'
import { DEFAULT_MAX_UPLOAD_BYTES, getCurrentUser, getPublicConfig, leaveRoom, listRooms, logoutUser, requestRoomJoin, updateCurrentUser, uploadAttachment } from './api'
import { createBrowserNotifier } from './browserNotifications'
import { useAttachmentDownloads } from './composables/useAttachmentDownloads'
import { useChatSocket } from './composables/useChatSocket'
import { useUnreadSocket } from './composables/useUnreadSocket'
import { loadPreferences, storePreferences } from './preferences'
import type { AuthSession, ChatPreferences, Room, RoomUpdateResult, User } from './types'

const SESSION_TOKEN_KEY = 'chat-room.session-token'
const ACTIVE_ROOM_KEY = 'chat-room.active-room'
const SIDEBAR_COLLAPSED_KEY = 'chat-room.sidebar-collapsed'
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
const preferencesOpen = ref(false)
const savingPreferences = ref(false)
const mobileView = ref<'rooms' | 'chat'>('rooms')
const uploading = ref(false)
const maxUploadBytes = ref(DEFAULT_MAX_UPLOAD_BYTES)
const sidebarCollapsed = ref(storageGet(window.localStorage, SIDEBAR_COLLAPSED_KEY) === 'true')
const preferences = ref(loadPreferences())
let restoreAttempted = false
const toast = useToast()
const {
  cancel: cancelDownload,
  download: handleDownload,
  downloading,
  downloadProgress,
} = useAttachmentDownloads(() => selectedRoom.value?.name || 'chat-files')

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
  if (content === 'membership removed' || content === 'membership left') {
    chat.close({ preserveMessages: true })
    void loadRoomList()
  }
}

const chat = useChatSocket(handleSystemEvent)
const notifier = createBrowserNotifier((roomId) => {
  const room = rooms.value.find((candidate) => candidate.id === roomId)
  if (room) selectRoom(room)
})
notifier.configure(preferences.value.notificationsEnabled, preferences.value.notificationDetails)
const selectedId = computed(() => selectedRoom.value?.id)
const unreadSocket = useUnreadSocket((states) => {
  rooms.value = rooms.value.map((room) => {
    const state = states.get(room.id)
    return state ? {
      ...room,
      unread_count: state.unread_count,
      membership_status: state.membership_status,
      membership_role: state.membership_role,
    } : { ...room, membership_status: undefined, membership_role: undefined, unread_count: 0 }
  })
  if (selectedRoom.value) {
    selectedRoom.value = rooms.value.find((room) => room.id === selectedRoom.value?.id) || selectedRoom.value
  }
}, notifier.notify)

watch(chat.authenticated, (online) => {
  if (online && selectedRoom.value?.has_password) {
    storageSet(window.sessionStorage, passwordKey(selectedRoom.value.id), roomPassword.value)
  }
})

function showToast(message: string): void {
  toast.add({ severity: 'success', summary: message, life: 2600 })
}

function toggleSidebar(): void {
  sidebarCollapsed.value = !sidebarCollapsed.value
  storageSet(window.localStorage, SIDEBAR_COLLAPSED_KEY, String(sidebarCollapsed.value))
}

function requestCreateRoom(): void {
  if (!currentUser.value) {
    authOpen.value = true
    return
  }
  createOpen.value = true
}

async function handlePreferencesSave(next: ChatPreferences): Promise<void> {
  savingPreferences.value = true
  try {
    if (next.notificationsEnabled) {
      if (typeof Notification === 'undefined') throw new Error('当前浏览器不支持消息通知')
      const permission = Notification.permission === 'default'
        ? await Notification.requestPermission()
        : Notification.permission
      if (permission !== 'granted') throw new Error('浏览器没有授予通知权限')
    }
    if (currentUser.value && sessionToken.value && next.avatarEmoji !== currentUser.value.avatar_emoji) {
      currentUser.value = await updateCurrentUser(sessionToken.value, next.avatarEmoji)
    }
    preferences.value = { ...next, avatarEmoji: currentUser.value?.avatar_emoji || '' }
    storePreferences(preferences.value)
    notifier.configure(next.notificationsEnabled, next.notificationDetails)
    preferencesOpen.value = false
    showToast('偏好设置已保存')
  } catch (caught) {
    toast.add({ severity: 'error', summary: caught instanceof Error ? caught.message : '保存失败', life: 3200 })
  } finally {
    savingPreferences.value = false
  }
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
  if (autoConnect && room.membership_status === 'active' && currentUser.value && sessionToken.value && (!room.has_password || roomPassword.value)) {
    joinSelectedRoom()
  }
}

async function loadRoomList(): Promise<void> {
  loading.value = true
  try {
    const nextRooms = await listRooms(sessionToken.value)
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

async function loadRuntimeConfig(): Promise<void> {
  try {
    const config = await getPublicConfig()
    if (Number.isSafeInteger(config.max_upload_bytes) && config.max_upload_bytes > 0) {
      maxUploadBytes.value = config.max_upload_bytes
    }
  } catch {
    maxUploadBytes.value = DEFAULT_MAX_UPLOAD_BYTES
  }
}

function joinSelectedRoom(): void {
  if (!selectedRoom.value) return
  if (!currentUser.value || !sessionToken.value) {
    authOpen.value = true
    return
  }
  if (selectedRoom.value.membership_status !== 'active') {
    void handleJoinRequest()
    return
  }
  if (selectedRoom.value.has_password && !roomPassword.value) {
    chat.error.value = '请输入房间密码'
    return
  }
  chat.connect(selectedRoom.value, sessionToken.value, currentUser.value.id, roomPassword.value)
}

async function handleJoinRequest(): Promise<void> {
  const room = selectedRoom.value
  if (!room || !sessionToken.value) {
    authOpen.value = true
    return
  }
  if (room.has_password && !roomPassword.value) {
    chat.error.value = '请输入房间密码'
    return
  }
  try {
    const membership = await requestRoomJoin(room.id, sessionToken.value, roomPassword.value)
    const updated = { ...room, membership_status: membership.status, membership_role: membership.role }
    rooms.value = rooms.value.map((item) => item.id === room.id ? updated : item)
    selectedRoom.value = updated
    if (membership.status === 'active') {
      joinSelectedRoom()
      showToast('已加入聊天室')
    } else {
      showToast('加入申请已提交')
    }
  } catch (caught) {
    chat.error.value = caught instanceof Error ? caught.message : '加入申请失败'
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
  authOpen.value = false
  unreadSocket.connect(session.token)
  await loadRoomList()
  showToast(`已登录为 ${session.user.username}`)
  if (selectedRoom.value?.membership_status === 'active' && (!selectedRoom.value.has_password || roomPassword.value)) {
    joinSelectedRoom()
  }
}

async function handleLogout(): Promise<void> {
  const token = sessionToken.value
  chat.close()
  sessionToken.value = ''
  currentUser.value = null
  unreadSocket.close()
  storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
  if (token) {
    try { await logoutUser(token) } catch { /* The local session is already cleared. */ }
  }
  await loadRoomList()
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

async function handleUpload(files: File[], content = '', replyTo = ''): Promise<void> {
  const room = selectedRoom.value
  if (!room || !sessionToken.value || !chat.authenticated.value || uploading.value) return
  uploading.value = true
  try {
    for (const [index, file] of files.entries()) {
      const message = await uploadAttachment(
        room.id,
        file,
        sessionToken.value,
        roomPassword.value,
        index === 0 ? content : '',
        index === 0 ? replyTo : '',
        maxUploadBytes.value,
      )
      if (selectedRoom.value?.id === room.id) chat.appendBroadcast(message, false)
    }
  } catch (caught) {
    toast.add({
      severity: 'error',
      summary: caught instanceof Error ? caught.message : '文件上传失败',
      life: 3200,
    })
  } finally {
    uploading.value = false
  }
}

async function handleLeaveRoom(): Promise<void> {
  const room = selectedRoom.value
  if (!room || !sessionToken.value) return
  try {
    await leaveRoom(room.id, sessionToken.value)
    chat.close()
    await loadRoomList()
    showToast('已退出聊天室')
  } catch (caught) {
    toast.add({ severity: 'error', summary: caught instanceof Error ? caught.message : '退出失败', life: 3200 })
  }
}

function handleRead(messageId: string): void {
  if (!chat.markRead(messageId) || !selectedRoom.value) return
  const roomId = selectedRoom.value.id
  rooms.value = rooms.value.map((room) => room.id === roomId ? { ...room, unread_count: 0 } : room)
  selectedRoom.value = { ...selectedRoom.value, unread_count: 0 }
}

onMounted(async () => {
  await Promise.all([restoreSession(), loadRuntimeConfig()])
  if (sessionToken.value) unreadSocket.connect(sessionToken.value)
  await loadRoomList()
})
</script>

<template>
  <div
    class="grid h-dvh w-full overflow-hidden bg-surface-100 transition-[grid-template-columns] duration-200 ease-out"
    :class="sidebarCollapsed ? 'md:grid-cols-[76px_minmax(0,1fr)]' : 'md:grid-cols-[340px_minmax(0,1fr)]'"
    data-testid="app-shell"
  >
    <div v-if="networkError" class="fixed inset-x-0 top-3 z-50 mx-auto w-[min(92vw,560px)]" role="alert">
      <Message severity="error" :closable="false">
        <div class="flex items-center gap-3">
          <span class="min-w-0 flex-1">{{ networkError }}</span>
          <Button label="重试" size="small" severity="danger" outlined @click="loadRoomList" />
        </div>
      </Message>
    </div>

    <RoomSidebar
      :rooms="rooms"
      :selected-id="selectedId"
      :user="currentUser"
      :loading="loading"
      :visible="mobileView === 'rooms'"
      :collapsed="sidebarCollapsed"
      @select="selectRoom"
      @refresh="loadRoomList"
      @create="requestCreateRoom"
      @authenticate="authOpen = true"
      @logout="handleLogout"
      @settings="preferencesOpen = true"
      @toggle-collapse="toggleSidebar"
    />
    <ChatPanel
      :room="selectedRoom"
      :user="currentUser"
      :password="roomPassword"
      :token="sessionToken"
      :status="chat.status.value"
      :status-label="chat.statusLabel.value"
      :authenticated="chat.authenticated.value"
      :error="chat.error.value"
      :messages="chat.messages.value"
      :members="chat.members.value"
      :participants="chat.participants.value"
      :read-receipts="chat.readReceipts.value"
      :current-user-id="chat.currentUserId.value"
      :visible="mobileView === 'chat'"
      :uploading="uploading"
      :downloading="downloading"
      :download-progress="downloadProgress"
      :max-upload-bytes="maxUploadBytes"
      :send-shortcut="preferences.sendShortcut"
      :focus-shortcut="preferences.focusShortcut"
      :typing-drafts="chat.typingDrafts.value"
      @back="mobileView = 'rooms'"
      @manage="manageOpen = true"
      @leave="handleLeaveRoom"
      @join="joinSelectedRoom"
      @request-join="handleJoinRequest"
      @authenticate="authOpen = true"
      @send="chat.send"
      @read="handleRead"
      @upload="handleUpload"
      @recall="chat.recall"
      @edit="chat.edit"
      @typing="chat.sendTyping"
      @download="handleDownload"
      @cancel-download="cancelDownload"
      @update:password="roomPassword = $event"
    />

    <AuthDialog :open="authOpen" @close="authOpen = false" @authenticated="handleAuthenticated" />
    <CreateRoomDialog :open="createOpen" :token="sessionToken" @close="createOpen = false" @created="handleCreated" />
    <ManageRoomDialog
      :open="manageOpen"
      :room="selectedRoom"
      :credential="roomPassword"
      :token="sessionToken"
      @close="manageOpen = false"
      @updated="handleUpdated"
      @deleted="handleDeleted"
    />
    <PreferencesDialog
      :open="preferencesOpen"
      :user="currentUser"
      :preferences="preferences"
      :saving="savingPreferences"
      @close="preferencesOpen = false"
      @save="handlePreferencesSave"
    />

    <Toast position="top-right" />
  </div>
</template>
