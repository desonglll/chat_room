<script setup lang="ts">
import { computed, defineAsyncComponent, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import Toast from 'primevue/toast'
import { useToast } from 'primevue/usetoast'
import ChatPanel from './components/ChatPanel.vue'
import NetworkErrorBanner from './components/NetworkErrorBanner.vue'
import PrivacyLockScreen from './components/PrivacyLockScreen.vue'
import RoomSidebar from './components/RoomSidebar.vue'
import { leaveRoom } from './api'
import { createBrowserNotifier } from './browserNotifications'
import { useAttachmentDownloads } from './composables/useAttachmentDownloads'
import { useChatSocket } from './composables/useChatSocket'
import { useUnreadSocket } from './composables/useUnreadSocket'
import { useAppPages } from './composables/useAppPages'
import { useAppBootstrap } from './composables/useAppBootstrap'
import { useAttachmentUpload } from './composables/useAttachmentUpload'
import { usePreferencesController } from './composables/usePreferencesController'
import { useRoomMembership } from './composables/useRoomMembership'
import { useRoomHistory } from './composables/useRoomHistory'
import { useTheme } from './composables/useTheme'
import { useRoomRouteSync } from './composables/useRoomRouteSync'
import { loadPreferences } from './preferences'
import { canAutoConnectRoom, reconcileMembershipAuthFailure } from './roomMembershipState'
import { storageGet, storageSet } from './browserStorage'
import type { Room, RoomUpdateResult } from './types'

const SIDEBAR_COLLAPSED_KEY = 'chat-room.sidebar-collapsed'
const passwordKey = (roomId: string) => `chat-room.password.${roomId}`
const AuthDialog = defineAsyncComponent(() => import('./components/AuthDialog.vue'))
const CreateRoomDialog = defineAsyncComponent(() => import('./components/CreateRoomDialog.vue'))
const DiscoverRooms = defineAsyncComponent(() => import('./components/DiscoverRooms.vue'))
const ForwardDialog = defineAsyncComponent(() => import('./components/ForwardDialog.vue'))
const JoinRoomDialog = defineAsyncComponent(() => import('./components/JoinRoomDialog.vue'))
const ManageRoomDialog = defineAsyncComponent(() => import('./components/ManageRoomDialog.vue'))
const PreferencesDialog = defineAsyncComponent(() => import('./components/PreferencesDialog.vue'))
const ProfilePage = defineAsyncComponent(() => import('./components/ProfilePage.vue'))
const SettingsPage = defineAsyncComponent(() => import('./components/SettingsPage.vue'))

const route = useRoute()
const router = useRouter()
const roomPassword = ref('')
const createOpen = ref(false)
const manageOpen = ref(false)
const forwardOpen = ref(false)
const forwardMessageIds = ref<string[]>([])
const authOpen = ref(false)
const mobileView = ref<'rooms' | 'chat'>('rooms')
const sidebarCollapsed = ref(storageGet(window.localStorage, SIDEBAR_COLLAPSED_KEY) === 'true')
const preferences = ref(loadPreferences())
const privacyLockScreen = ref<{ lock: () => void } | null>(null)
useTheme(computed(() => preferences.value.theme))
const sidebarWidth = ref(340)
const toast = useToast()
const {
  aiEnabled,
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
} = useAppBootstrap({
  closeChat: () => chat.close(),
  closeUnread: () => unreadSocket.close(),
  connectUnread: (token) => unreadSocket.connect(token),
  clearSelection: () => clearSelection(),
  selectRoom: (room, autoConnect) => selectRoom(room, autoConnect),
  joinSelectedRoom: () => joinSelectedRoom(),
  afterAccountDeleted: () => {
    activePage.value = 'chat'
    mobileView.value = 'rooms'
  },
  showToast,
})
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
  () => {
    authOpen.value = true
  },
  () => {
    preferenceController.open.value = true
  },
)
const selectedId = computed(() => selectedRoom.value?.id)
const unreadSocket = useUnreadSocket((states) => {
  rooms.value = rooms.value.map((room) => {
    const state = states.get(room.id)
    return state
      ? {
          ...room,
          unread_count: state.unread_count,
          membership_status: state.membership_status,
          membership_role: state.membership_role,
        }
      : { ...room, membership_status: undefined, membership_role: undefined, unread_count: 0 }
  })
  if (selectedRoom.value) {
    selectedRoom.value = rooms.value.find((room) => room.id === selectedRoom.value?.id) || selectedRoom.value
  }
}, notifier.notify)
const {
  discoverError,
  discoverJoiningId,
  handleJoinedById,
  handleJoinRequest,
  joinDiscoveredRoom,
  joinOpen,
  joinSelectedRoom,
  openJoinRoom,
} = useRoomMembership({
  rooms,
  selectedRoom,
  currentUser,
  token: sessionToken,
  password: roomPassword,
  requireAccount,
  selectRoom,
  connect: chat.connect,
  setError: (message) => {
    chat.error.value = message
  },
  showToast,
})

restoreCachedSelection()

watch(chat.authFailureReason, (reason) => {
  const room = selectedRoom.value
  if (!reason || !room) return
  const updated = reconcileMembershipAuthFailure(room, reason)
  if (updated === room) return
  selectedRoom.value = updated
  rooms.value = rooms.value.map((item) => (item.id === updated.id ? updated : item))
})

useRoomRouteSync({ authenticated: chat.authenticated, room: selectedRoom, password: roomPassword })

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
  roomPassword.value = storageGet(window.sessionStorage, passwordKey(room.id))
  const reconnect = autoConnect && canAutoConnectRoom(room, currentUser.value, sessionToken.value, roomPassword.value)
  const preserveRoomRoute = reconnect && route.name === 'room' && routeRoomId.value === room.id
  if (!preserveRoomRoute) activePage.value = 'chat'
  mobileView.value = 'chat'
  if (reconnect) joinSelectedRoom()
}

const history = useRoomHistory({
  room: selectedRoom,
  token: sessionToken,
  password: roomPassword,
  messages: chat.messages,
  prepend: chat.prependHistory,
})

watch(routeRoomId, (id) => {
  if (selectedRoom.value?.id === id) return
  if (!id) {
    if (selectedRoom.value) clearSelection(false)
    return
  }
  const room = rooms.value.find((item) => item.id === id)
  if (room) selectRoom(room, false)
})

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
  rooms.value = rooms.value.map((room) => (room.id === result.room.id ? result.room : room))
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
  append: chat.appendUpload,
  update: chat.updateUpload,
  complete: chat.completeUpload,
  remove: chat.removeUpload,
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
    toast.add({
      severity: 'error',
      summary: caught instanceof Error ? caught.message : '退出失败',
      life: 3200,
    })
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
</script>

<template>
  <RouterView v-if="route.name === 'admin'" />
  <div
    v-else
    class="cr-canvas-ambient relative grid h-dvh w-full overflow-hidden transition-[grid-template-columns] duration-200 ease-out motion-reduce:transition-none md:[grid-template-columns:var(--sidebar-cols)]"
    :style="{
      '--sidebar-cols': sidebarCollapsed ? '76px minmax(0,1fr)' : `${sidebarWidth}px minmax(0,1fr)`,
    }"
    data-testid="app-shell"
  >
    <NetworkErrorBanner :message="networkError" @retry="loadRoomList" />

    <RoomSidebar
      :rooms="rooms"
      :selected-id="selectedId"
      :user="currentUser"
      :loading="showColdSkeleton"
      :refreshing="refreshingRooms"
      :visible="mobileView === 'rooms'"
      :collapsed="sidebarCollapsed"
      @select="selectRoom"
      @refresh="loadRoomList"
      @create="requestCreateRoom"
      @join="openJoinRoom"
      @discover="openDiscover"
      @authenticate="authOpen = true"
      @logout="handleLogout"
      @lock="privacyLockScreen?.lock()"
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
      :loading="showColdSkeleton"
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
      :pending-uploads="attachmentUpload.pendingUploads.value"
      :downloading="downloading"
      :download-progress="downloadProgress"
      :max-upload-bytes="maxUploadBytes"
      :send-shortcut="preferences.sendShortcut"
      :focus-shortcut="preferences.focusShortcut"
      :typing-drafts="chat.typingDrafts.value"
      :poked-at="chat.pokedAt.value"
      :loading-older="history.loading.value"
      :has-more-history="history.hasMore.value"
      :ai-enabled="aiEnabled"
      :loading="showColdSkeleton"
      :ensure-message="history.ensureMessage"
      @back="mobileView = 'rooms'"
      @manage="manageOpen = true"
      @leave="handleLeaveRoom"
      @join="joinSelectedRoom"
      @request-join="handleJoinRequest"
      @authenticate="authOpen = true"
      @send="chat.send"
      @read="chat.markRead"
      @upload="attachmentUpload.upload"
      @resume-upload="attachmentUpload.resume"
      @cancel-upload="attachmentUpload.cancel"
      @cancel-upload-task="attachmentUpload.cancelTask"
      @retry-upload-task="attachmentUpload.retry"
      @recall="chat.recall"
      @edit="chat.edit"
      @typing="chat.sendTyping"
      @download="handleDownload"
      @cancel-download="cancelDownload"
      @update:password="roomPassword = $event"
      @forward="openForward"
      @poke="chat.poke"
      @retry="chat.retry"
      @load-older="history.loadOlder"
    />

    <AuthDialog
      :open="authOpen"
      @close="authOpen = false"
      @authenticated="
        (session) => {
          authOpen = false
          handleAuthenticated(session)
        }
      "
    />
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
  <PrivacyLockScreen
    ref="privacyLockScreen"
    :token="sessionToken"
    :shortcut="preferences.privacyLockShortcut"
    @change="notifier.configure(!$event && preferences.notificationsEnabled, preferences.notificationDetails)"
    @logout="handleLogout"
  />
</template>
