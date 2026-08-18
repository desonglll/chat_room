<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Toast from 'primevue/toast'
import { useToast } from 'primevue/usetoast'
import AuthDialog from './components/AuthDialog.vue'
import ChatPanel from './components/ChatPanel.vue'
import CreateRoomDialog from './components/CreateRoomDialog.vue'
import DiscoverRooms from './components/DiscoverRooms.vue'
import ForwardDialog from './components/ForwardDialog.vue'
import ManageRoomDialog from './components/ManageRoomDialog.vue'
import JoinRoomDialog from './components/JoinRoomDialog.vue'
import ProfilePage from './components/ProfilePage.vue'
import PreferencesDialog from './components/PreferencesDialog.vue'
import RoomSidebar from './components/RoomSidebar.vue'
import SettingsPage from './components/SettingsPage.vue'
import { DEFAULT_MAX_UPLOAD_BYTES, getCurrentUser, getPublicConfig, leaveRoom, listRoomMessages, listRooms, logoutUser, requestRoomJoin, storedMessageToBroadcast } from './api'
import { createBrowserNotifier } from './browserNotifications'
import { useAttachmentDownloads } from './composables/useAttachmentDownloads'
import { useChatSocket } from './composables/useChatSocket'
import { useUnreadSocket } from './composables/useUnreadSocket'
import { useAppPages } from './composables/useAppPages'
import { useAttachmentUpload } from './composables/useAttachmentUpload'
import { usePreferencesController } from './composables/usePreferencesController'
import { useTheme } from './composables/useTheme'
import { loadPreferences } from './preferences'
import { storageGet, storageSet } from './browserStorage'
import type { AuthSession, Room, RoomUpdateResult, User } from './types'

const SESSION_TOKEN_KEY = 'chat-room.session-token'
const SIDEBAR_COLLAPSED_KEY = 'chat-room.sidebar-collapsed'
const passwordKey = (roomId: string) => `chat-room.password.${roomId}`

const route = useRoute()
const router = useRouter()
const routeRoomId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''))
const rooms = ref<Room[]>([])
const selectedRoom = ref<Room | null>(null)
const sessionToken = ref(storageGet(window.localStorage, SESSION_TOKEN_KEY))
const currentUser = ref<User | null>(null)
const roomPassword = ref('')
const loading = ref(true)
const networkError = ref('')
const createOpen = ref(false)
const manageOpen = ref(false)
const forwardOpen = ref(false)
const forwardMessageIds = ref<string[]>([])
const authOpen = ref(false)
const joinOpen = ref(false)
const mobileView = ref<'rooms' | 'chat'>('rooms')
const maxUploadBytes = ref(DEFAULT_MAX_UPLOAD_BYTES)
const aiEnabled = ref(false)
const sidebarCollapsed = ref(storageGet(window.localStorage, SIDEBAR_COLLAPSED_KEY) === 'true')
const preferences = ref(loadPreferences())
useTheme(computed(() => preferences.value.theme))
const sidebarWidth = ref(340)
const loadingOlder = ref(false)
const hasMoreHistory = ref(true)
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
const preferenceController = usePreferencesController({
  preferences,
  user: currentUser,
  token: sessionToken,
  configureNotifications: notifier.configure,
  showSuccess: showToast,
  showError: (message) => toast.add({ severity: 'error', summary: message, life: 3200 }),
})
const { activePage, openProfile, openSettings, openDiscover, requireAccount, returnToChat } = useAppPages(
  currentUser,
  selectedRoom,
  mobileView,
  () => chat.authenticated.value,
  () => { authOpen.value = true },
  () => { preferenceController.open.value = true },
)
const discoverJoiningId = ref('')
const discoverError = ref('')
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
  // Keep the URL in sync with connection state: /rooms/:id only once actually
  // online, /rooms/:id/join otherwise — connecting/disconnecting later (not
  // just selecting the room) also needs to move between the two URLs.
  if (!selectedRoom.value || (route.name !== 'room' && route.name !== 'room-join')) return
  const target = online
    ? { name: 'room' as const, params: { id: selectedRoom.value.id } }
    : { name: 'room-join' as const, params: { id: selectedRoom.value.id } }
  if (router.resolve(target).fullPath !== route.fullPath) {
    void router.replace(target).catch(() => {})
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

function clearSelection(navigate = true): void {
  chat.close()
  selectedRoom.value = null
  roomPassword.value = ''
  manageOpen.value = false
  mobileView.value = 'rooms'
  if (navigate && route.name !== 'home') void router.push({ name: 'home' }).catch(() => {})
}

function selectRoom(room: Room, autoConnect = false): void {
  chat.close()
  selectedRoom.value = room
  activePage.value = 'chat'
  roomPassword.value = storageGet(window.sessionStorage, passwordKey(room.id))
  mobileView.value = 'chat'
  if (autoConnect && room.membership_status === 'active' && currentUser.value && sessionToken.value && (!room.has_password || roomPassword.value)) {
    joinSelectedRoom()
  }
}

watch(() => selectedRoom.value?.id, () => {
  hasMoreHistory.value = true
  loadingOlder.value = false
})

async function loadOlderMessages(): Promise<void> {
  const room = selectedRoom.value
  const oldest = chat.messages.value.find((message) => message.type === 'broadcast')
  if (!room || !sessionToken.value || !oldest || loadingOlder.value || !hasMoreHistory.value) return
  loadingOlder.value = true
  try {
    const page = await listRoomMessages(room.id, sessionToken.value, roomPassword.value, oldest.message_id, 50)
    hasMoreHistory.value = page.length === 50
    chat.prependHistory(page.map(storedMessageToBroadcast))
  } catch {
    // Leave hasMoreHistory as-is; scrolling up again will retry.
  } finally {
    loadingOlder.value = false
  }
}

watch(routeRoomId, (id) => {
  if (selectedRoom.value?.id === id) return
  if (!id) {
    if (selectedRoom.value) clearSelection(false)
    return
  }
  const room = rooms.value.find((item) => item.id === id)
  if (room) selectRoom(room, false)
})

function openJoinRoom(): void {
  requireAccount(() => { joinOpen.value = true })
}

function handleJoinedById(room: Room, password: string): void {
  joinOpen.value = false
  const existing = rooms.value.some((item) => item.id === room.id)
  rooms.value = existing
    ? rooms.value.map((item) => item.id === room.id ? room : item)
    : [...rooms.value, room]
  selectRoom(room)
  roomPassword.value = password
  if (password) storageSet(window.sessionStorage, passwordKey(room.id), password)
  if (room.membership_status === 'active') {
    joinSelectedRoom()
    showToast('已加入聊天室')
  } else {
    showToast('加入申请已提交')
  }
}

async function handleAccountDeleted(): Promise<void> {
  chat.close()
  unreadSocket.close()
  sessionToken.value = ''
  currentUser.value = null
  activePage.value = 'chat'
  mobileView.value = 'rooms'
  storageSet(window.localStorage, SESSION_TOKEN_KEY, '')
  await loadRoomList()
  showToast('账户已注销')
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
      const activeId = routeRoomId.value
      const restored = nextRooms.find((room) => room.id === activeId)
      // Restoring a room from the URL only re-selects it — it must not silently
      // perform the "join" action the user hadn't actually taken before refreshing.
      if (restored) selectRoom(restored, false)
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

async function joinDiscoveredRoom(room: Room): Promise<void> {
  if (!currentUser.value || !sessionToken.value) {
    authOpen.value = true
    return
  }
  discoverJoiningId.value = room.id
  discoverError.value = ''
  try {
    const membership = await requestRoomJoin(room.id, sessionToken.value, '')
    const updated = { ...room, membership_status: membership.status, membership_role: membership.role }
    rooms.value = rooms.value.some((item) => item.id === room.id)
      ? rooms.value.map((item) => item.id === room.id ? updated : item)
      : [...rooms.value, updated]
    if (membership.status === 'active') {
      selectRoom(updated)
      joinSelectedRoom()
      showToast('已加入聊天室')
    } else {
      showToast('加入申请已提交')
    }
  } catch (caught) {
    discoverError.value = caught instanceof Error ? caught.message : '加入失败'
  } finally {
    discoverJoiningId.value = ''
  }
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

const attachmentUpload = useAttachmentUpload({
  room: selectedRoom,
  token: sessionToken,
  password: roomPassword,
  authenticated: () => chat.authenticated.value,
  maxBytes: maxUploadBytes,
  append: (message) => chat.appendBroadcast(message, false),
  showError: (message) => toast.add({ severity: 'error', summary: message, life: 3200 }),
})

async function handleLeaveRoom(room: Room | null = selectedRoom.value): Promise<void> {
  if (!room || !sessionToken.value) return
  try {
    await leaveRoom(room.id, sessionToken.value)
    if (selectedRoom.value?.id === room.id) chat.close()
    await loadRoomList()
    showToast('已退出聊天室')
  } catch (caught) {
    toast.add({ severity: 'error', summary: caught instanceof Error ? caught.message : '退出失败', life: 3200 })
  }
}

function openRoomManage(room: Room): void {
  selectRoom(room)
  manageOpen.value = true
}

function openForward(messageIds: string[]): void {
  if (!messageIds.length) return
  forwardMessageIds.value = messageIds
  forwardOpen.value = true
}

function handleForwarded(): void {
  forwardOpen.value = false
  showToast('已转发')
}

function handleRead(messageId: string): void {
  chat.markRead(messageId)
}

onMounted(async () => {
  await Promise.all([restoreSession(), loadRuntimeConfig()])
  if (sessionToken.value) unreadSocket.connect(sessionToken.value)
  await loadRoomList()
})
</script>

<template>
  <div
    class="cr-canvas-ambient grid h-dvh w-full overflow-hidden transition-[grid-template-columns] duration-200 ease-out md:[grid-template-columns:var(--sidebar-cols)]"
    :style="{ '--sidebar-cols': sidebarCollapsed ? '76px minmax(0,1fr)' : `${sidebarWidth}px minmax(0,1fr)` }"
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
      @join="openJoinRoom"
      @discover="openDiscover"
      @authenticate="authOpen = true"
      @logout="handleLogout"
      @profile="openProfile"
      @settings="openSettings"
      @toggle-collapse="toggleSidebar"
      @resize="sidebarWidth = $event"
      @manage="openRoomManage"
      @leave-room="handleLeaveRoom"
    />
    <ProfilePage
      v-if="activePage === 'profile' && currentUser"
      :user="currentUser"
      :token="sessionToken"
      @back="returnToChat"
      @updated="preferenceController.profileUpdated"
    />
    <SettingsPage
      v-else-if="activePage === 'settings' && currentUser"
      :user="currentUser"
      :token="sessionToken"
      @back="returnToChat"
      @preferences="preferenceController.open.value = true"
      @deleted="handleAccountDeleted"
    />
    <DiscoverRooms
      v-else-if="activePage === 'discover'"
      :rooms="rooms"
      :user="currentUser"
      :loading="loading"
      :joining-id="discoverJoiningId"
      :error="discoverError"
      @back="returnToChat"
      @join="joinDiscoveredRoom"
      @authenticate="authOpen = true"
    />
    <ChatPanel
      v-else
      :room="selectedRoom"
      :user="currentUser"
      :password="roomPassword"
      :token="sessionToken"
      :status="chat.status.value"
      :status-label="chat.statusLabel.value"
      :authenticated="chat.authenticated.value"
      :history-ready="chat.historyReady.value"
      :error="chat.error.value"
      :messages="chat.messages.value"
      :members="chat.members.value"
      :participants="chat.participants.value"
      :read-receipts="chat.readReceipts.value"
      :current-user-id="chat.currentUserId.value"
      :visible="mobileView === 'chat'"
      :uploading="attachmentUpload.uploading.value"
      :upload-progress="attachmentUpload.progress.value"
      :downloading="downloading"
      :download-progress="downloadProgress"
      :max-upload-bytes="maxUploadBytes"
      :send-shortcut="preferences.sendShortcut"
      :focus-shortcut="preferences.focusShortcut"
      :typing-drafts="chat.typingDrafts.value"
      :poked-at="chat.pokedAt.value"
      :loading-older="loadingOlder"
      :has-more-history="hasMoreHistory"
      :ai-enabled="aiEnabled"
      :loading="loading"
      @back="mobileView = 'rooms'"
      @manage="manageOpen = true"
      @leave="handleLeaveRoom"
      @join="joinSelectedRoom"
      @request-join="handleJoinRequest"
      @authenticate="authOpen = true"
      @send="chat.send"
      @read="handleRead"
      @upload="attachmentUpload.upload"
      @recall="chat.recall"
      @edit="chat.edit"
      @typing="chat.sendTyping"
      @download="handleDownload"
      @cancel-download="cancelDownload"
      @update:password="roomPassword = $event"
      @forward="openForward"
      @poke="chat.poke"
      @load-older="loadOlderMessages"
    />

    <AuthDialog :open="authOpen" @close="authOpen = false" @authenticated="handleAuthenticated" />
    <ForwardDialog
      :open="forwardOpen"
      :message-ids="forwardMessageIds"
      :rooms="rooms"
      :token="sessionToken"
      @close="forwardOpen = false"
      @forwarded="handleForwarded"
    />
    <JoinRoomDialog :open="joinOpen" :token="sessionToken" @close="joinOpen = false" @joined="handleJoinedById" />
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
      :open="preferenceController.open.value"
      :user="currentUser"
      :preferences="preferences"
      :saving="preferenceController.saving.value"
      @close="preferenceController.open.value = false"
      @save="preferenceController.save"
    />

    <Toast position="top-right" />
  </div>
</template>
