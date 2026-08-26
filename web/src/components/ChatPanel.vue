<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { UploadCloud } from 'lucide-vue-next'
import { useRouter } from 'vue-router'
import ChatAccessPanel from './ChatAccessPanel.vue'
import ChatRoomHeader from './ChatRoomHeader.vue'
import MessageComposer from './MessageComposer.vue'
import MessageList from './MessageList.vue'
import MessageSelectionBar from './MessageSelectionBar.vue'
import RoomConnectingView from './RoomConnectingView.vue'
import UploadStatusPanel from './UploadStatusPanel.vue'
import { shouldFocusComposer } from '../composer'
import { useRoomMessageNavigation } from '../composables/useRoomMessageNavigation'
import { useMessageSelection } from '../composables/useMessageSelection'
import { resolveRoomViewState } from '../roomViewState'
import type {
  Attachment,
  AttachmentUploadSession,
  BroadcastMessage,
  ChatStatus,
  ConversationSummary,
  DisplayMessage,
  FocusShortcut,
  ReadReceipt,
  Room,
  RoomMember,
  SendShortcut,
  SocialUser,
  TypingDraft,
  User,
} from '../types'
import type { DownloadProgress } from '../attachmentDownloads'
const ChatFilesDialog = defineAsyncComponent(() => import('./ChatFilesDialog.vue'))
const ImageViewerGallery = defineAsyncComponent(() => import('./ImageViewerGallery.vue'))
const ProfileCardDialog = defineAsyncComponent(() => import('./ProfileCardDialog.vue'))
const RoomMessageSearchDialog = defineAsyncComponent(() => import('./RoomMessageSearchDialog.vue'))
const RoomPinsBar = defineAsyncComponent(() => import('./RoomPinsBar.vue'))
const props = defineProps<{
  room: Room | null
  conversation: ConversationSummary | null
  user: User | null
  password: string
  rememberRoomPasswords: boolean
  contact: SocialUser | null
  setFriendRemark: (userId: string, remark: string) => Promise<void>
  token: string
  status: ChatStatus
  statusLabel: string
  authenticated: boolean
  historyReady: boolean
  error: string
  messages: DisplayMessage[]
  favoriteMessageIds: string[]
  members: RoomMember[]
  participants: RoomMember[]
  readReceipts: ReadReceipt[]
  currentUserId: string
  visible: boolean
  sendShortcut: SendShortcut
  focusShortcut: FocusShortcut
  typingDrafts: TypingDraft[]
  downloading: boolean
  downloadProgress: DownloadProgress | null
  maxUploadBytes: number
  pokedAt: number
  loadingOlder: boolean
  hasMoreHistory: boolean
  pendingUploads: AttachmentUploadSession[]
  aiEnabled: boolean
  aiPanelOpen: boolean
  loading: boolean
  ensureMessage: (messageId: string) => Promise<boolean>
}>()
const emit = defineEmits<{
  back: []
  manage: []
  leave: []
  join: []
  requestJoin: []
  authenticate: []
  send: [content: string, replyTo: string]
  read: [messageId: string]
  upload: [files: File[], content: string, replyTo: string, isSensitive: boolean]
  resumeUpload: [session: AttachmentUploadSession, file: File]
  cancelUpload: [session: AttachmentUploadSession]
  cancelUploadTask: [key: string]
  retryUploadTask: [key: string]
  recall: [messageId: string]
  edit: [messageId: string, content: string]
  forward: [messageIds: string[]]
  favorite: [messageIds: string[]]
  typing: [content: string]
  download: [attachments: Attachment[]]
  cancelDownload: []
  poke: [userId: string]
  retry: [messageId: string]
  reaction: [messageId: string, emoji: string, active: boolean]
  loadOlder: []
  removeFriend: []
  blockUser: []
  assistant: []
  'update:password': [password: string]
  'update:rememberRoomPasswords': [remember: boolean]
}>()

const replyingTo = ref<BroadcastMessage | null>(null)
const editingTo = ref<BroadcastMessage | null>(null)
const filesOpen = ref(false)
const previewImageId = ref('')
const viewProfileUserId = ref('')
const composerRef = ref<InstanceType<typeof MessageComposer> | null>(null)
const messageListRef = ref<InstanceType<typeof MessageList> | null>(null)
const roomPinsRef = ref<{ toggle: (messageId: string) => Promise<void> } | null>(null)
const pinnedMessageIds = ref<string[]>([])
const dragActive = ref(false)
const shaking = ref(false)
const viewState = computed(() =>
  resolveRoomViewState({
    room: props.room,
    status: props.status,
    authenticated: props.authenticated,
    loading: props.loading,
    messageCount: props.messages.length,
  }),
)
let dragDepth = 0
let shakeTimer: number | undefined
const router = useRouter()
const canPin = computed(
  () =>
    props.conversation?.kind === 'direct' ||
    (props.conversation?.kind === 'group' && ['owner', 'admin'].includes(props.room?.membership_role || '')),
)
const { searchOpen, locateMessage, locateSearchResult } = useRoomMessageNavigation({
  roomId: () => props.room?.id || '',
  ready: () => viewState.value === 'conversation' && props.historyReady,
  visible: () => props.visible,
  authenticated: () => props.authenticated,
  messageList: () => messageListRef.value,
  closeFiles: () => (filesOpen.value = false),
})
const {
  closeSelection,
  downloadSelected,
  favoriteSelected,
  forwardSelected,
  selectedAttachments,
  selectedMessageIds,
  selecting,
  toggleSelection,
} = useMessageSelection(() => props.messages, {
  download: (attachments) => emit('download', attachments),
  favorite: (messageIds) => emit('favorite', messageIds),
  forward: (messageIds) => emit('forward', messageIds),
})

watch(
  () => props.pokedAt,
  (value) => {
    if (!value) return
    shaking.value = false
    void requestAnimationFrame(() => {
      shaking.value = true
      window.clearTimeout(shakeTimer)
      shakeTimer = window.setTimeout(() => {
        shaking.value = false
      }, 600)
    })
  },
)
const galleryImages = computed(() =>
  props.messages.flatMap((message) =>
    message.type === 'broadcast' && message.attachment?.mime_type.startsWith('image/') && !message.recalled_at
      ? [message.attachment]
      : [],
  ),
)

watch(
  () => props.room?.id,
  () => {
    replyingTo.value = null
    editingTo.value = null
    filesOpen.value = false
    pinnedMessageIds.value = []
    previewImageId.value = ''
    selecting.value = false
    selectedMessageIds.value = []
  },
)

function sendMessage(content: string, replyTo: string): void {
  emit('send', content, replyTo)
  replyingTo.value = null
}

function uploadFiles(files: File[], content: string, replyTo: string, isSensitive: boolean): void {
  emit('upload', files, content, replyTo, isSensitive)
  replyingTo.value = null
}

function recallMessage(messageId: string): void {
  if (replyingTo.value?.message_id === messageId) replyingTo.value = null
  emit('recall', messageId)
}

function startReply(message: BroadcastMessage): void {
  editingTo.value = null
  replyingTo.value = message
}

function startEdit(message: BroadcastMessage): void {
  if (message.favorite_id) {
    void router.push({ name: 'favorites', query: { edit: message.favorite_id } }).catch(() => {})
    return
  }
  replyingTo.value = null
  editingTo.value = message
}

function editMessage(messageId: string, content: string): void {
  emit('edit', messageId, content)
  editingTo.value = null
}

function isEditableTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null
  return Boolean(element?.closest('input, textarea, select, button, [contenteditable="true"]'))
}

function handleGlobalKeydown(event: KeyboardEvent): void {
  if (
    !props.visible ||
    !props.authenticated ||
    !shouldFocusComposer(
      event,
      props.focusShortcut,
      isEditableTarget(event.target),
      Boolean(document.querySelector('.p-dialog-mask')),
    )
  )
    return
  event.preventDefault()
  composerRef.value?.focus()
}

function handleDragEnter(event: DragEvent): void {
  if (!event.dataTransfer?.types.includes('Files')) return
  dragDepth += 1
  dragActive.value = true
}

function handleDragLeave(): void {
  dragDepth = Math.max(0, dragDepth - 1)
  if (!dragDepth) dragActive.value = false
}

function handleDrop(event: DragEvent): void {
  dragDepth = 0
  dragActive.value = false
  const files = Array.from(event.dataTransfer?.files || [])
  if (files.length) composerRef.value?.addFiles(files)
}

onMounted(() => document.addEventListener('keydown', handleGlobalKeydown))
onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleGlobalKeydown)
  window.clearTimeout(shakeTimer)
})
</script>

<template>
  <main
    id="workspace-main"
    class="cr-chat-panel cr-chat-canvas absolute inset-0 flex min-h-0 min-w-0 flex-col transition-[transform,opacity,visibility] duration-[var(--cr-motion-enter)] [transition-timing-function:var(--cr-ease-drawer)] motion-reduce:transition-none md:relative md:inset-auto md:visible md:translate-x-0 md:opacity-100"
    :class="[
      visible
        ? 'visible translate-x-0 opacity-100'
        : 'invisible pointer-events-none translate-x-4 opacity-0 md:pointer-events-auto',
      { 'animate-poke-shake': shaking },
    ]"
  >
    <ChatRoomHeader
      v-if="room"
      :room="room"
      :alias="conversation?.alias || ''"
      :original-title="conversation?.title || room.name"
      :kind="conversation?.kind || 'group'"
      :peer="conversation?.peer || null"
      :status="status"
      :status-label="statusLabel"
      :authenticated="authenticated"
      :members="members"
      :current-user-id="currentUserId"
      :token="token"
      :ai-enabled="aiEnabled"
      :ai-panel-open="aiPanelOpen"
      @back="emit('back')"
      @manage="emit('manage')"
      @leave="emit('leave')"
      @files="filesOpen = true"
      @search="searchOpen = true"
      @view-profile="viewProfileUserId = $event"
      @toggle-selection="selecting = !selecting"
      @remove-friend="emit('removeFriend')"
      @block-user="emit('blockUser')"
      @assistant="emit('assistant')"
    />

    <ChatAccessPanel
      v-if="viewState === 'loading' || viewState === 'empty' || viewState === 'access'"
      :room="room"
      :user="user"
      :password="password"
      :remember-room-passwords="rememberRoomPasswords"
      :status="status"
      :error="error"
      :loading="loading"
      @join="emit('join')"
      @request-join="emit('requestJoin')"
      @authenticate="emit('authenticate')"
      @update:password="emit('update:password', $event)"
      @update:remember-room-passwords="emit('update:rememberRoomPasswords', $event)"
    />

    <RoomConnectingView v-else-if="viewState === 'connecting'" />

    <section
      v-else
      class="cr-conversation-stage relative flex min-h-0 flex-1 flex-col"
      @dragenter.prevent="handleDragEnter"
      @dragover.prevent
      @dragleave="handleDragLeave"
      @drop.prevent="handleDrop"
    >
      <Transition
        enter-active-class="transition-[opacity] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        enter-from-class="opacity-0"
        leave-active-class="transition-[opacity] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        leave-to-class="opacity-0"
      >
        <div
          v-if="dragActive"
          class="pointer-events-none absolute inset-3 z-30 grid place-items-center rounded-lg border-2 border-dashed border-primary bg-primary-50/95 text-primary shadow-lg"
        >
          <div class="text-center">
            <UploadCloud class="mx-auto" :size="34" />
            <strong class="mt-2 block text-sm">释放以添加附件</strong>
          </div>
        </div>
      </Transition>
      <RoomPinsBar
        v-if="conversation"
        ref="roomPinsRef"
        :room-id="room?.id || ''"
        :token="token"
        :can-pin="canPin"
        @update:message-ids="pinnedMessageIds = $event"
        @locate="locateMessage"
        @edit-favorite="router.push({ name: 'favorites', query: { edit: $event } })"
      />
      <MessageList
        ref="messageListRef"
        :room-id="room?.id || ''"
        :direct="conversation?.kind === 'direct'"
        :unread-count="room?.unread_count || 0"
        :messages="messages"
        :favorite-message-ids="favoriteMessageIds"
        :pinned-message-ids="pinnedMessageIds"
        :can-pin="canPin"
        :read-receipts="readReceipts"
        :participants="participants"
        :current-user-id="currentUserId"
        :visible="visible"
        :history-ready="historyReady"
        :selecting="selecting"
        :selected-message-ids="selectedMessageIds"
        :loading-older="loadingOlder"
        :has-more-history="hasMoreHistory"
        :ensure-message="ensureMessage"
        @read="emit('read', $event)"
        @reply="startReply"
        @recall="recallMessage"
        @edit="startEdit"
        @forward="(message) => emit('forward', [message.message_id])"
        @favorite="(message) => emit('favorite', [message.message_id])"
        @pin="(message) => roomPinsRef?.toggle(message.message_id)"
        @toggle-select="toggleSelection"
        @load-older="emit('loadOlder')"
        @preview-image="previewImageId = $event.id"
        @view-profile="viewProfileUserId = $event"
        @poke="emit('poke', $event)"
        @retry="emit('retry', $event)"
        @reaction="(messageId, emoji, active) => emit('reaction', messageId, emoji, active)"
        @cancel-upload="emit('cancelUploadTask', $event)"
        @retry-upload="emit('retryUploadTask', $event)"
      />
      <TransitionGroup
        v-if="typingDrafts.length && !selecting"
        tag="div"
        class="cr-typing-strip space-y-1 px-4 py-2 sm:px-6"
        enter-active-class="transition-[opacity,transform] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        enter-from-class="translate-y-1 opacity-0"
        leave-active-class="transition-[opacity,transform] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
        leave-to-class="opacity-0"
      >
        <div v-for="draft in typingDrafts" :key="draft.user_id" class="flex min-w-0 items-center gap-2 text-xs">
          <strong class="shrink-0 text-primary">{{ draft.username }} 正在输入</strong>
          <span class="truncate text-muted-color">{{ draft.content }}</span>
        </div>
      </TransitionGroup>
      <MessageSelectionBar
        v-if="selecting"
        :selected-count="selectedMessageIds.length"
        :attachment-count="selectedAttachments.length"
        :downloading="downloading"
        :download-progress="downloadProgress"
        @close="closeSelection"
        @forward="forwardSelected"
        @favorite="favoriteSelected"
        @download="downloadSelected"
        @cancel-download="emit('cancelDownload')"
      />
      <template v-else>
        <UploadStatusPanel
          :pending="pendingUploads"
          @resume="(session, file) => emit('resumeUpload', session, file)"
          @cancel="emit('cancelUpload', $event)"
        />
        <MessageComposer
          ref="composerRef"
          :key="room?.id || ''"
          :replying-to="replyingTo"
          :editing-to="editingTo"
          :send-shortcut="sendShortcut"
          :max-upload-bytes="maxUploadBytes"
          :participants="participants"
          :room-id="room?.id || ''"
          :token="token"
          :password="password"
          :ai-enabled="aiEnabled"
          :disabled="!authenticated"
          @cancel-reply="replyingTo = null"
          @cancel-edit="editingTo = null"
          @send="sendMessage"
          @edit="editMessage"
          @typing="emit('typing', $event)"
          @upload="uploadFiles"
        />
      </template>
    </section>
    <ChatFilesDialog
      :open="filesOpen"
      :room-id="room?.id || ''"
      :token="token"
      :password="password"
      :downloading="downloading"
      :download-progress="downloadProgress"
      @close="filesOpen = false"
      @download="emit('download', $event)"
      @cancel-download="emit('cancelDownload')"
      @locate-message="locateMessage"
    />
    <RoomMessageSearchDialog
      :open="searchOpen"
      :room-id="room?.id || ''"
      :token="token"
      :password="password"
      @close="searchOpen = false"
      @locate="locateSearchResult"
    />
    <ImageViewerGallery :images="galleryImages" :active-id="previewImageId" @close="previewImageId = ''" />
    <ProfileCardDialog
      :open="Boolean(viewProfileUserId)"
      :user-id="viewProfileUserId"
      :token="token"
      :room-id="conversation?.kind === 'direct' ? undefined : room?.id"
      :current-user-id="currentUserId"
      :contact="contact"
      :set-remark="setFriendRemark"
      @close="viewProfileUserId = ''"
      @remove-friend="emit('removeFriend')"
      @block-user="emit('blockUser')"
    />
  </main>
</template>
