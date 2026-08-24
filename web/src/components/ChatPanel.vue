<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Download, Forward, UploadCloud, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import ProgressBar from 'primevue/progressbar'
import ChatAccessPanel from './ChatAccessPanel.vue'
import ChatRoomHeader from './ChatRoomHeader.vue'
import MessageComposer from './MessageComposer.vue'
import MessageList from './MessageList.vue'
import RoomConnectingView from './RoomConnectingView.vue'
import UploadStatusPanel from './UploadStatusPanel.vue'
import { shouldFocusComposer } from '../composer'
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
  TypingDraft,
  User,
} from '../types'
import type { DownloadProgress } from '../attachmentDownloads'

const ChatFilesDialog = defineAsyncComponent(() => import('./ChatFilesDialog.vue'))
const ImageViewerGallery = defineAsyncComponent(() => import('./ImageViewerGallery.vue'))
const ProfileCardDialog = defineAsyncComponent(() => import('./ProfileCardDialog.vue'))

const props = defineProps<{
  room: Room | null
  conversation: ConversationSummary | null
  user: User | null
  password: string
  token: string
  status: ChatStatus
  statusLabel: string
  authenticated: boolean
  historyReady: boolean
  error: string
  messages: DisplayMessage[]
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
  typing: [content: string]
  download: [attachments: Attachment[]]
  cancelDownload: []
  poke: [userId: string]
  retry: [messageId: string]
  reaction: [messageId: string, emoji: string, active: boolean]
  loadOlder: []
  removeFriend: []
  blockUser: []
  'update:password': [password: string]
}>()

const replyingTo = ref<BroadcastMessage | null>(null)
const editingTo = ref<BroadcastMessage | null>(null)
const filesOpen = ref(false)
const selecting = ref(false)
const selectedMessageIds = ref<string[]>([])
const previewImageId = ref('')
const viewProfileUserId = ref('')
const composerRef = ref<InstanceType<typeof MessageComposer> | null>(null)
const messageListRef = ref<InstanceType<typeof MessageList> | null>(null)
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
const selectedAttachments = computed(() =>
  props.messages.flatMap((message) =>
    message.type === 'broadcast' && message.attachment && selectedMessageIds.value.includes(message.message_id)
      ? [message.attachment]
      : [],
  ),
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
  replyingTo.value = null
  editingTo.value = message
}

function editMessage(messageId: string, content: string): void {
  emit('edit', messageId, content)
  editingTo.value = null
}

function toggleSelection(messageId: string): void {
  selectedMessageIds.value = selectedMessageIds.value.includes(messageId)
    ? selectedMessageIds.value.filter((id) => id !== messageId)
    : [...selectedMessageIds.value, messageId]
}

function closeSelection(): void {
  selecting.value = false
  selectedMessageIds.value = []
}

function downloadSelected(): void {
  emit('download', selectedAttachments.value)
}

function forwardSelected(): void {
  emit('forward', [...selectedMessageIds.value])
  closeSelection()
}

async function locateMessage(messageId: string): Promise<void> {
  filesOpen.value = false
  await nextTick()
  await messageListRef.value?.scrollToMessage(messageId)
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
    class="cr-chat-panel cr-chat-canvas absolute inset-0 flex min-h-0 min-w-0 flex-col transition-[transform,opacity,visibility] duration-200 ease-out motion-reduce:transition-none md:relative md:inset-auto md:visible md:translate-x-0 md:opacity-100"
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
      :kind="conversation?.kind || 'group'"
      :peer="conversation?.peer || null"
      :status="status"
      :status-label="statusLabel"
      :authenticated="authenticated"
      :members="members"
      :current-user-id="currentUserId"
      @back="emit('back')"
      @manage="emit('manage')"
      @leave="emit('leave')"
      @files="filesOpen = true"
      @view-profile="viewProfileUserId = $event"
      @toggle-selection="selecting = !selecting"
      @remove-friend="emit('removeFriend')"
      @block-user="emit('blockUser')"
    />

    <ChatAccessPanel
      v-if="viewState === 'loading' || viewState === 'empty' || viewState === 'access'"
      :room="room"
      :user="user"
      :password="password"
      :status="status"
      :error="error"
      :loading="loading"
      @join="emit('join')"
      @request-join="emit('requestJoin')"
      @authenticate="emit('authenticate')"
      @update:password="emit('update:password', $event)"
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
        enter-active-class="transition duration-150 ease-out motion-reduce:transition-none"
        enter-from-class="opacity-0"
        leave-active-class="transition duration-100 ease-out motion-reduce:transition-none"
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
      <MessageList
        ref="messageListRef"
        :room-id="room?.id || ''"
        :direct="conversation?.kind === 'direct'"
        :unread-count="room?.unread_count || 0"
        :messages="messages"
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
        class="space-y-1 border-t border-surface-200 bg-surface-0 px-3 py-2 sm:px-5"
        enter-active-class="transition duration-150 motion-reduce:transition-none"
        enter-from-class="translate-y-1 opacity-0"
        leave-active-class="transition duration-100 motion-reduce:transition-none"
        leave-to-class="opacity-0"
      >
        <div v-for="draft in typingDrafts" :key="draft.user_id" class="flex min-w-0 items-center gap-2 text-xs">
          <strong class="shrink-0 text-primary">{{ draft.username }} 正在输入</strong>
          <span class="truncate text-muted-color">{{ draft.content }}</span>
        </div>
      </TransitionGroup>
      <div
        v-if="selecting"
        class="flex min-h-16 shrink-0 items-center justify-between gap-3 border-t border-surface-200 bg-surface-0 px-3 sm:px-5"
      >
        <Button text rounded severity="secondary" aria-label="退出多选" title="退出多选" @click="closeSelection"
          ><X :size="19"
        /></Button>
        <div class="min-w-0 flex-1">
          <span class="text-sm text-muted-color">已选择 {{ selectedMessageIds.length }} 条消息</span>
          <div v-if="downloadProgress" class="mt-2 flex items-center gap-2">
            <ProgressBar :value="downloadProgress.percent" :show-value="false" class="h-1.5 min-w-24 flex-1" />
            <span class="shrink-0 text-xs text-muted-color"
              >{{ downloadProgress.completedFiles }}/{{ downloadProgress.totalFiles }}</span
            >
            <Button size="small" text severity="danger" @click="emit('cancelDownload')">取消</Button>
          </div>
        </div>
        <Button :disabled="!selectedMessageIds.length" severity="secondary" outlined @click="forwardSelected">
          <Forward :size="17" />
          <span>转发</span>
        </Button>
        <Button :disabled="!selectedAttachments.length" :loading="downloading" @click="downloadSelected">
          <Download :size="17" />
          <span>保存</span>
        </Button>
      </div>
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
    <ImageViewerGallery :images="galleryImages" :active-id="previewImageId" @close="previewImageId = ''" />
    <ProfileCardDialog
      :open="Boolean(viewProfileUserId)"
      :user-id="viewProfileUserId"
      :token="token"
      :room-id="conversation?.kind === 'direct' ? undefined : room?.id"
      :current-user-id="currentUserId"
      @close="viewProfileUserId = ''"
    />
  </main>
</template>

<style scoped>
@keyframes poke-shake {
  0%,
  100% {
    transform: translateX(0);
  }
  20% {
    transform: translateX(-8px);
  }
  40% {
    transform: translateX(7px);
  }
  60% {
    transform: translateX(-5px);
  }
  80% {
    transform: translateX(3px);
  }
}

.animate-poke-shake {
  animation: poke-shake 0.28s var(--cr-ease-in-out);
}

@media (prefers-reduced-motion: reduce) {
  .animate-poke-shake {
    animation: none;
  }
}
</style>
