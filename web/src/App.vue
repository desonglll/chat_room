<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useToast } from 'primevue/usetoast'
import AppDialogs from './components/AppDialogs.vue'
import AiAssistantPage from './components/AiAssistantPage.vue'
import ChatPanel from './components/ChatPanel.vue'
import NetworkErrorBanner from './components/NetworkErrorBanner.vue'
import PrivacyLockScreen from './components/PrivacyLockScreen.vue'
import RoomSidebar from './components/RoomSidebar.vue'
import WorkspacePages from './components/WorkspacePages.vue'
import { createBrowserNotifier } from './browserNotifications'
import { conversationToRoom } from './conversationState'
import { useAttachmentDownloads } from './composables/useAttachmentDownloads'
import { useChatSocket } from './composables/useChatSocket'
import { useContacts } from './composables/useContacts'
import { useFavorites } from './composables/useFavorites'
import { useFavoriteMessageActions } from './composables/useFavoriteMessageActions'
import { useConversations } from './composables/useConversations'
import { useDirectConversationActions } from './composables/useDirectConversationActions'
import { useMessageForwarding } from './composables/useMessageForwarding'
import { useUnreadSocket } from './composables/useUnreadSocket'
import { useAppPages } from './composables/useAppPages'
import { useAppBootstrap } from './composables/useAppBootstrap'
import { useAttachmentUpload } from './composables/useAttachmentUpload'
import { usePreferencesController } from './composables/usePreferencesController'
import { useRoomMembership } from './composables/useRoomMembership'
import { useRoomHistory } from './composables/useRoomHistory'
import { useRoomActions } from './composables/useRoomActions'
import { useTheme } from './composables/useTheme'
import { useRoomRouteSync } from './composables/useRoomRouteSync'
import { loadPreferences } from './preferences'
import { reconcileMembershipAuthFailure } from './roomMembershipState'
import { storageGet, storageSet } from './browserStorage'
import { clearRoomPassword } from './roomPasswordVault'
import { createRoomSystemEventHandler } from './roomSystemEvents'
import type { ConversationSummary, Room } from './types'

const route = useRoute()
const router = useRouter()
const roomPassword = ref('')
const createOpen = ref(false)
const newConversationOpen = ref(false)
const manageOpen = ref(false)
const authOpen = ref(false)
const mobileView = ref<'rooms' | 'chat'>('rooms')
const sidebarCollapsed = ref(storageGet(window.localStorage, 'chat-room.sidebar-collapsed') === 'true')
const aiPanelOpen = ref(false)
const preferences = ref(loadPreferences())
const privacyLockScreen = ref<{ lock: () => void } | null>(null)
useTheme(computed(() => preferences.value.theme))
const sidebarWidth = ref(360)
const toast = useToast()
const {
  aiStatus,
  capabilities,
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
const selectedId = computed(() => selectedRoom.value?.id)
const conversationState = useConversations(sessionToken, selectedId)
const contacts = useContacts(sessionToken)
const favorites = useFavorites(sessionToken)
const favoriteMessages = useFavoriteMessageActions(favorites)
const selectedConversation = computed(
  () => conversationState.conversations.value.find((item) => item.room_id === selectedId.value) || null,
)
const selectedContact = computed(
  () => contacts.friends.value.find((item) => item.id === selectedConversation.value?.peer?.id) || null,
)
const forwardRooms = computed(() => conversationState.conversations.value.map(conversationToRoom))
const {
  cancel: cancelDownload,
  download: handleDownload,
  downloading,
  downloadProgress,
} = useAttachmentDownloads(() => selectedRoom.value?.name || 'chat-files')

const handleSystemEvent = createRoomSystemEventHandler({
  room: () => selectedRoom.value,
  managing: () => manageOpen.value,
  closeChat: () => chat.close({ preserveMessages: true }),
  clearPassword: (roomId) => {
    clearRoomPassword(roomId)
    roomPassword.value = ''
  },
  clearSelection: () => clearSelection(),
  refreshConversations: () => void conversationState.refresh(),
  refreshRooms: () => void loadRoomList(),
  showToast,
})

const chat = useChatSocket(handleSystemEvent)
const notifier = createBrowserNotifier((roomId) => {
  const conversation = conversationState.conversations.value.find((candidate) => candidate.room_id === roomId)
  if (conversation) selectConversation(conversation)
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
const {
  activePage,
  openProfile,
  openSettings,
  openDiscover,
  openContacts,
  openFavorites,
  requireAccount,
  returnToChat,
} = useAppPages(
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
const roomAiPanelVisible = computed(
  () => aiPanelOpen.value && activePage.value === 'chat' && Boolean(currentUser.value && selectedRoom.value),
)
const sidebarColumns = computed(() => {
  if (roomAiPanelVisible.value) return '60px minmax(20rem,1fr) minmax(22rem,32rem)'
  return sidebarCollapsed.value || activePage.value !== 'chat'
    ? '60px minmax(0,1fr)'
    : `${sidebarWidth.value}px minmax(0,1fr)`
})
const unreadSocket = useUnreadSocket(
  (states) => {
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
      const group = rooms.value.find((room) => room.id === selectedRoom.value?.id)
      if (group) selectedRoom.value = group
    }
    conversationState.applyUnread(states)
  },
  (message) => {
    conversationState.handleMessage(message)
    notifier.notify(message)
  },
  () => void contacts.refresh(),
)
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
  status: chat.status,
  rememberPasswords: () => preferences.value.rememberRoomPasswords,
  requireAccount,
  selectRoom: (room) => selectRoom(room),
  connect: chat.connect,
  setError: (message) => {
    chat.error.value = message
  },
  showToast,
})
const roomActions = useRoomActions({
  rooms,
  selectedRoom,
  currentUser,
  token: sessionToken,
  password: roomPassword,
  rememberPasswords: () => preferences.value.rememberRoomPasswords,
  mobileView,
  createOpen,
  manageOpen,
  newConversationOpen,
  routeName: () => route.name,
  routeRoomId,
  chatStatus: () => chat.status.value,
  closeChat: () => chat.close(),
  joinSelectedRoom,
  showChatPage: () => {
    activePage.value = 'chat'
  },
  requestAuthentication: () => {
    authOpen.value = true
  },
  navigateHome: () => void router.push({ name: 'home' }).catch(() => {}),
  refreshRooms: loadRoomList,
  refreshConversations: conversationState.refresh,
  removeConversation: conversationState.remove,
  upsertConversation: conversationState.upsert,
  showSuccess: showToast,
  showError: (message) => toast.add({ severity: 'error', summary: message, life: 3200 }),
})
const {
  clearSelection,
  handleCreated,
  handleDeleted,
  handleLeaveRoom,
  handleUpdated,
  openRoomManage,
  requestCreateRoom,
} = roomActions

restoreCachedSelection()

watch(chat.authFailureReason, (reason) => {
  const room = selectedRoom.value
  if (!reason || !room) return
  const updated = reconcileMembershipAuthFailure(room, reason)
  if (updated === room) return
  selectedRoom.value = updated
  rooms.value = rooms.value.map((item) => (item.id === updated.id ? updated : item))
})

useRoomRouteSync({
  authenticated: chat.authenticated,
  room: selectedRoom,
  password: roomPassword,
  rememberPasswords: () => preferences.value.rememberRoomPasswords,
})

function showToast(message: string): void {
  toast.add({ severity: 'success', summary: message, life: 2600 })
}

async function refreshWorkspace(): Promise<void> {
  await Promise.all([loadRoomList(), conversationState.refresh(), contacts.refresh()])
}

function toggleSidebar(): void {
  sidebarCollapsed.value = !sidebarCollapsed.value
  storageSet(window.localStorage, 'chat-room.sidebar-collapsed', String(sidebarCollapsed.value))
}

function openAssistant(): void {
  aiPanelOpen.value = false
  activePage.value = 'assistant'
  mobileView.value = 'chat'
}

const selectRoom = (room: Room, autoConnect = false) => roomActions.selectRoom(room, autoConnect)
const selectConversation = (conversation: ConversationSummary, autoConnect = true) =>
  roomActions.selectConversation(conversation, autoConnect)
const { changeDirectAccess, changeSelectedDirectAccess, openDirectConversation, setSelectedFriendRemark } =
  useDirectConversationActions({
    token: sessionToken,
    selectedConversation,
    selectConversation,
    clearSelection: () => clearSelection(),
    refreshConversations: conversationState.refresh,
    setRemark: contacts.setRemark,
  })

const history = useRoomHistory({
  room: selectedRoom,
  token: sessionToken,
  password: roomPassword,
  messages: chat.messages,
  prepend: chat.prependHistory,
})

watch([routeRoomId, conversationState.conversations], ([id]) => {
  if (selectedRoom.value?.id === id) return
  if (!id) {
    if (selectedRoom.value) clearSelection(false)
    return
  }
  const room = rooms.value.find((item) => item.id === id)
  if (room) selectRoom(room, typeof route.query.message === 'string')
  else {
    const conversation = conversationState.conversations.value.find((item) => item.room_id === id)
    if (conversation) selectConversation(conversation)
  }
})

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

const { forwardMessageIds, forwardOpen, handleForwarded, openForward } = useMessageForwarding(() => {
  showToast('已转发')
  void conversationState.refresh()
})
</script>

<template>
  <RouterView v-if="route.name === 'admin'" />
  <div
    v-else
    class="cr-app-shell cr-canvas-ambient relative grid h-dvh w-full overflow-hidden md:[grid-template-columns:var(--sidebar-cols)]"
    :style="{ '--sidebar-cols': sidebarColumns }"
    data-testid="app-shell"
  >
    <NetworkErrorBanner :message="networkError" @retry="loadRoomList" />
    <!-- prettier-ignore -->
    <a class="cr-skip-link" :href="activePage === 'chat' && mobileView === 'rooms' ? '#conversation-list' : '#workspace-main'">跳到主要内容</a>
    <!-- prettier-ignore -->
    <RoomSidebar
      :conversations="conversationState.conversations.value"
      :selected-id="selectedId"
      :user="currentUser"
      :loading="showColdSkeleton || conversationState.loading.value"
      :refreshing="refreshingRooms"
      :visible="activePage === 'chat' && mobileView === 'rooms'"
      :collapsed="sidebarCollapsed || activePage !== 'chat' || roomAiPanelVisible"
      :incoming-requests="contacts.incomingCount.value"
      :active-section="activePage" :set-alias="conversationState.setAlias" :update-preferences="conversationState.updatePreferences"
      @select="selectConversation"
      @clear="clearSelection"
      @refresh="refreshWorkspace"
      @new-chat="newConversationOpen = true"
      @create="requestCreateRoom"
      @join="openJoinRoom"
      @discover="openDiscover"
      @contacts="openContacts" @search="activePage = 'search'; mobileView = 'chat'"
      @favorites="openFavorites"
      @assistant="openAssistant"
      @chat="returnToChat"
      @authenticate="authOpen = true"
      @logout="handleLogout"
      @lock="privacyLockScreen?.lock()"
      @profile="openProfile"
      @settings="openSettings"
      @toggle-collapse="toggleSidebar"
      @resize="sidebarWidth = $event"
      @manage="openRoomManage"
      @leave-room="handleLeaveRoom" @success="showToast" @error="toast.add({ severity: 'error', summary: $event, life: 3200 })"
    />
    <!-- prettier-ignore -->
    <WorkspacePages v-if="activePage === 'discover' || (activePage !== 'chat' && currentUser)"
      :active-page="activePage" :user="currentUser" :token="sessionToken" :contacts="contacts" :favorites="favorites" :ai-status="aiStatus" :remember-room-passwords="preferences.rememberRoomPasswords" :max-upload-bytes="maxUploadBytes"
      :rooms="forwardRooms" :discover-joining-id="discoverJoiningId" :discover-error="discoverError"
      :start-chat="openDirectConversation" :remove-friend="(id) => changeDirectAccess(id, contacts.remove)" :block-user="(id) => changeDirectAccess(id, contacts.block)" :join-room="joinDiscoveredRoom"
      @back="returnToChat" @preferences="preferenceController.open.value = true" @deleted="handleAccountDeleted" @updated="preferenceController.profileUpdated"
      @new-chat="newConversationOpen = true" @authenticate="authOpen = true" @conversations-changed="conversationState.refresh" @success="showToast" @error="toast.add({ severity: 'error', summary: $event, life: 3200 })" />
    <Transition v-else name="room-switch" mode="out-in">
      <ChatPanel
        :key="selectedRoom?.id || 'empty-room'"
        :room="selectedRoom"
        :conversation="selectedConversation"
        :contact="selectedContact"
        :set-friend-remark="setSelectedFriendRemark"
        :user="currentUser"
        :password="roomPassword"
        :remember-room-passwords="preferences.rememberRoomPasswords"
        :token="sessionToken"
        :status="chat.status.value"
        :status-label="chat.statusLabel.value"
        :authenticated="chat.authenticated.value"
        :history-ready="chat.historyReady.value"
        :error="chat.error.value"
        :messages="chat.messages.value"
        :favorite-message-ids="favorites.messageIds.value"
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
        :ai-enabled="capabilities.ai"
        :ai-panel-open="roomAiPanelVisible"
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
        @update:remember-room-passwords="preferenceController.setRememberRoomPasswords"
        @forward="openForward"
        @favorite="favoriteMessages"
        @poke="chat.poke"
        @retry="chat.retry"
        @reaction="chat.react"
        @load-older="history.loadOlder"
        @remove-friend="changeSelectedDirectAccess(contacts.remove)"
        @block-user="changeSelectedDirectAccess(contacts.block)"
        @assistant="aiPanelOpen = !aiPanelOpen"
      />
    </Transition>
    <AiAssistantPage
      v-if="roomAiPanelVisible && selectedRoom"
      :key="`room-ai-${selectedRoom.id}`"
      embedded
      :initial-room-id="selectedRoom.id"
      :token="sessionToken"
      :rooms="forwardRooms"
      :ai-status="aiStatus"
      :remember-room-passwords="preferences.rememberRoomPasswords"
      @back="aiPanelOpen = false"
      @error="toast.add({ severity: 'error', summary: $event, life: 3200 })"
    />

    <!-- prettier-ignore -->
    <AppDialogs
      :auth-open="authOpen" :create-open="createOpen" :forward-open="forwardOpen"
      :forward-message-ids="forwardMessageIds" :forward-rooms="forwardRooms" :join-open="joinOpen"
      :manage-open="manageOpen" :new-conversation-open="newConversationOpen"
      :preferences-open="preferenceController.open.value" :preferences-saving="preferenceController.saving.value"
      :preferences="preferences" :room="selectedRoom" :room-password="roomPassword" :token="sessionToken"
      :friends="contacts.friends.value" :user="currentUser"
      @auth-close="authOpen = false" @authenticated="authOpen = false; handleAuthenticated($event)"
      @create-close="createOpen = false" @created="handleCreated" @forward-close="forwardOpen = false"
      @forwarded="handleForwarded" @join-close="joinOpen = false" @joined="handleJoinedById"
      @manage-close="manageOpen = false" @updated="handleUpdated" @deleted="handleDeleted"
      @new-conversation-close="newConversationOpen = false" @conversation-opened="selectConversation"
      @social-changed="contacts.refresh" @create-group="newConversationOpen = false; requestCreateRoom()"
      @preferences-close="preferenceController.open.value = false" @save-preferences="preferenceController.save"
    />
  </div>
  <PrivacyLockScreen
    ref="privacyLockScreen"
    :token="sessionToken"
    :shortcut="preferences.privacyLockShortcut"
    @change="notifier.configure(!$event && preferences.notificationsEnabled, preferences.notificationDetails)"
    @logout="handleLogout"
  />
</template>
