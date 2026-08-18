<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  ArrowLeft,
  Check,
  Copy,
  DoorOpen,
  Download,
  EllipsisVertical,
  Forward,
  ListChecks,
  LogIn,
  LogOut,
  UserRound,
  UploadCloud,
  X,
} from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import Menu from 'primevue/menu'
import Message from 'primevue/message'
import Password from 'primevue/password'
import Popover from 'primevue/popover'
import ProgressBar from 'primevue/progressbar'
import Skeleton from 'primevue/skeleton'
import IconSprite from './IconSprite.vue'
import { avatarColor } from '../avatarColor'
import MessageComposer from './MessageComposer.vue'
import MessageList from './MessageList.vue'
import ChatFilesDialog from './ChatFilesDialog.vue'
import ImageViewerGallery from './ImageViewerGallery.vue'
import ProfileCardDialog from './ProfileCardDialog.vue'
import { shouldFocusComposer } from '../composer'
import type {
  Attachment,
  BroadcastMessage,
  ChatStatus,
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
import type { ChunkedUploadProgress } from '../composables/useChunkedUpload'

const props = defineProps<{
  room: Room | null
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
  uploading: boolean
  sendShortcut: SendShortcut
  focusShortcut: FocusShortcut
  typingDrafts: TypingDraft[]
  downloading: boolean
  downloadProgress: DownloadProgress | null
  maxUploadBytes: number
  pokedAt: number
  loadingOlder: boolean
  hasMoreHistory: boolean
  uploadProgress: ChunkedUploadProgress | null
  aiEnabled: boolean
  loading: boolean
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
  recall: [messageId: string]
  edit: [messageId: string, content: string]
  forward: [messageIds: string[]]
  typing: [content: string]
  download: [attachments: Attachment[]]
  cancelDownload: []
  poke: [userId: string]
  loadOlder: []
  'update:password': [password: string]
}>()

const replyingTo = ref<BroadcastMessage | null>(null)
const editingTo = ref<BroadcastMessage | null>(null)
const filesOpen = ref(false)
const selecting = ref(false)
const selectedMessageIds = ref<string[]>([])
const previewImageId = ref('')
const viewProfileUserId = ref('')
const memberPopover = ref()
const moreMenu = ref()
const composerRef = ref<InstanceType<typeof MessageComposer> | null>(null)
const dragActive = ref(false)
const roomIdCopied = ref(false)
const shaking = ref(false)
let dragDepth = 0
let shakeTimer: number | undefined

watch(() => props.pokedAt, (value) => {
  if (!value) return
  shaking.value = false
  void requestAnimationFrame(() => {
    shaking.value = true
    window.clearTimeout(shakeTimer)
    shakeTimer = window.setTimeout(() => { shaking.value = false }, 600)
  })
})
const passwordModel = computed({
  get: () => props.password,
  set: (value: string) => emit('update:password', value),
})
const statusColor = computed(() => ({
  idle: 'bg-surface-300',
  connecting: 'bg-warning',
  online: 'bg-success',
  offline: 'bg-danger',
  failed: 'bg-danger',
})[props.status])
const canManage = computed(() => ['owner', 'admin'].includes(props.room?.membership_role || ''))
const moreMenuItems = computed(() => [
  { label: '多选消息', icon: 'select', command: () => { selecting.value = !selecting.value } },
  ...(canManage.value ? [{ label: '管理聊天室', icon: 'manage', command: () => emit('manage') }] : []),
  ...(props.room?.membership_role !== 'owner' ? [{ label: '退出聊天室', icon: 'leave', danger: true, command: () => emit('leave') }] : []),
])
const selectedAttachments = computed(() => props.messages.flatMap((message) =>
  message.type === 'broadcast'
    && message.attachment
    && selectedMessageIds.value.includes(message.message_id)
    ? [message.attachment]
    : [],
))
const galleryImages = computed(() => props.messages.flatMap((message) =>
  message.type === 'broadcast' && message.attachment?.mime_type.startsWith('image/') && !message.recalled_at
    ? [message.attachment]
    : [],
))

watch(() => props.room?.id, () => {
  replyingTo.value = null
  editingTo.value = null
  filesOpen.value = false
  previewImageId.value = ''
  selecting.value = false
  selectedMessageIds.value = []
})

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

function handleJoin(): void {
  if (props.room?.membership_status === 'active') emit('join')
  else emit('requestJoin')
}

function isEditableTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null
  return Boolean(element?.closest('input, textarea, select, button, [contenteditable="true"]'))
}

function handleGlobalKeydown(event: KeyboardEvent): void {
  if (!props.visible || !props.authenticated || !shouldFocusComposer(
    event,
    props.focusShortcut,
    isEditableTarget(event.target),
    Boolean(document.querySelector('.p-dialog-mask')),
  )) return
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

async function copyRoomId(): Promise<void> {
  if (!props.room) return
  try {
    await navigator.clipboard.writeText(props.room.id)
    roomIdCopied.value = true
    window.setTimeout(() => { roomIdCopied.value = false }, 1600)
  } catch {
    window.prompt('复制聊天室 ID', props.room.id)
  }
}

onMounted(() => document.addEventListener('keydown', handleGlobalKeydown))
onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleGlobalKeydown)
  window.clearTimeout(shakeTimer)
})
</script>

<template>
  <main
    class="min-h-0 min-w-0 flex-col bg-surface-50 md:flex"
    :class="[visible ? 'flex' : 'hidden', { 'animate-poke-shake': shaking }]"
  >
    <header class="flex h-[72px] shrink-0 items-center justify-between gap-3 border-b border-surface-200 bg-surface-0 px-3 sm:px-5">
      <div class="flex min-w-0 items-center gap-2 sm:gap-3">
        <Button class="md:hidden" text rounded severity="secondary" aria-label="返回房间列表" title="返回房间列表" @click="emit('back')">
          <ArrowLeft :size="20" />
        </Button>
        <div class="min-w-0">
          <div class="flex min-w-0 items-center gap-1.5">
            <h2 class="truncate text-[15px] font-semibold text-surface-900" :title="room?.description || undefined">{{ room?.name || '选择聊天室' }}</h2>
            <Button v-if="room" text rounded severity="secondary" size="small" :aria-label="roomIdCopied ? '聊天室 ID 已复制' : '复制聊天室 ID'" :title="roomIdCopied ? '已复制' : `复制 ID：${room.id}`" @click="copyRoomId">
              <Check v-if="roomIdCopied" :size="14" class="text-success" />
              <Copy v-else :size="14" />
            </Button>
          </div>
          <div class="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-muted-color">
            <span class="size-2 shrink-0 rounded-full" :class="statusColor" />
            <span class="shrink-0">{{ statusLabel }}</span>
            <button
              v-if="authenticated"
              type="button"
              class="shrink-0 cursor-pointer underline-offset-2 hover:text-primary hover:underline"
              @click="memberPopover.toggle($event)"
            >
              · {{ members.length }} 人在线
            </button>
            <span v-if="room" class="hidden shrink-0 sm:inline">· {{ room.has_password ? '私密房间' : '公开房间' }}</span>
            <code v-if="room" class="hidden truncate font-mono text-[10px] lg:inline">· {{ room.id }}</code>
          </div>
        </div>
      </div>

      <div v-if="room" class="flex shrink-0 items-center gap-1">
        <Popover ref="memberPopover">
          <div class="w-60">
            <div class="mb-2 flex items-center justify-between border-b border-surface-200 pb-3">
              <strong class="text-sm">在线成员</strong>
              <span class="text-xs text-muted-color">{{ members.length }}</span>
            </div>
            <ul class="max-h-72 space-y-1 overflow-y-auto p-0">
              <li
                v-for="member in members"
                :key="member.user_id"
                class="group flex min-h-10 items-center gap-2.5 rounded-md px-1.5 py-1 text-sm hover:bg-surface-50"
              >
                <button
                  type="button"
                  class="min-w-0 cursor-pointer rounded-full"
                  aria-label="查看用户资料"
                  @click="viewProfileUserId = member.user_id"
                >
                  <Avatar
                    :label="member.avatar_emoji || member.username.slice(0, 1).toUpperCase()"
                    shape="circle"
                    size="small"
                    class="shrink-0 text-white!"
                    :style="{ backgroundColor: avatarColor(member.user_id) }"
                  />
                </button>
                <span class="min-w-0 flex-1 truncate">
                  {{ member.user_id === currentUserId ? `${member.username}（你）` : member.username }}
                </span>
                <button
                  v-if="member.user_id !== currentUserId"
                  type="button"
                  class="shrink-0 rounded px-1.5 py-0.5 text-xs text-muted-color opacity-0 transition hover:bg-surface-200 hover:text-primary group-hover:opacity-100"
                  aria-label="拍一拍"
                  title="拍一拍"
                  @click="emit('poke', member.user_id)"
                >
                  拍一拍
                </button>
              </li>
            </ul>
          </div>
        </Popover>
        <Button text rounded severity="secondary" aria-label="查看在线成员" title="在线成员" @click="memberPopover.toggle($event)">
          <IconSprite name="members" :size="19" />
        </Button>
        <Button v-if="authenticated" text rounded severity="secondary" aria-label="聊天文件" title="聊天文件" @click="filesOpen = true">
          <IconSprite name="files" :size="19" />
        </Button>
        <Button v-if="authenticated" text rounded severity="secondary" aria-label="更多操作" title="更多操作" @click="moreMenu.toggle($event)">
          <EllipsisVertical :size="20" />
        </Button>
        <Menu ref="moreMenu" :model="moreMenuItems" :popup="true">
          <template #item="{ item, props: itemProps }">
            <a v-bind="itemProps.action" :class="{ 'text-danger!': item.danger }">
              <ListChecks v-if="item.icon === 'select'" :size="17" />
              <EllipsisVertical v-else-if="item.icon === 'manage'" :size="17" />
              <LogOut v-else-if="item.icon === 'leave'" :size="17" />
              <span>{{ item.label }}</span>
            </a>
          </template>
        </Menu>
      </div>
    </header>

    <section v-if="loading && !room" class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-6">
      <div v-for="index in 4" :key="index" class="flex items-start gap-2.5" :class="{ 'flex-row-reverse': index % 2 === 0 }">
        <Skeleton shape="circle" size="2rem" />
        <div class="space-y-2" :style="{ width: `${38 + (index * 7) % 30}%` }">
          <Skeleton height="0.8rem" width="40%" />
          <Skeleton height="2.4rem" border-radius="14px" />
        </div>
      </div>
    </section>

    <section v-else-if="!room" class="fade-in flex min-h-0 flex-1 flex-col items-center justify-center px-6 text-center">
      <span class="grid size-24 place-items-center rounded-2xl bg-gradient-to-br from-primary-50 to-surface-0 shadow-lg">
        <img src="/brand/echo-gate.svg" alt="" class="size-12" aria-hidden="true">
      </span>
      <strong class="mt-6 text-lg font-semibold">选择一个聊天室</strong>
      <p class="mt-1.5 text-sm text-muted-color">从左侧列表开始，或去发现公开聊天室</p>
    </section>

    <section v-else-if="!authenticated" class="fade-in flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-6">
      <form class="w-full max-w-[420px]" autocomplete="off" data-testid="join-form" @submit.prevent="handleJoin">
        <span class="grid size-12 place-items-center rounded-lg bg-primary-50 text-primary-700">
          <DoorOpen :size="23" />
        </span>
        <h3 class="mt-5 text-xl font-semibold">加入 {{ room.name }}</h3>
        <p class="mt-1.5 text-sm text-muted-color">
          {{ room.membership_status === 'pending'
            ? '申请正在等待管理员审核'
            : room.membership_status === 'invited'
              ? '管理员已邀请你加入'
              : room.join_policy === 'approval' ? '提交申请后由管理员审核' : '验证后可直接加入' }}
        </p>

        <div v-if="user" class="mt-6 flex min-h-12 items-center gap-3 rounded-lg border border-surface-200 bg-surface-0 px-3 text-sm">
          <UserRound :size="18" class="text-primary" />
          <span>以 <strong>{{ user.username }}</strong> 的身份加入</span>
        </div>

        <div v-if="room.has_password" class="mt-5 flex flex-col gap-2">
          <label for="joinPassword" class="text-sm font-medium">房间密码</label>
          <Password id="joinPassword" v-model="passwordModel" name="room-access-password" :feedback="false" toggle-mask fluid autocomplete="off" />
        </div>

        <Message v-if="room.membership_status === 'pending'" severity="info" size="small" :closable="false" class="mt-4">加入申请已提交</Message>
        <Message v-else-if="error" severity="error" size="small" :closable="false" class="mt-4">{{ error }}</Message>
        <Button v-if="user" class="mt-6 w-full" type="submit" :loading="status === 'connecting'" :disabled="room.membership_status === 'pending'">
          <LogIn :size="18" />
          <span>{{ room.membership_status === 'active' ? '进入聊天室' : room.membership_status === 'invited' ? '接受邀请并加入' : room.join_policy === 'approval' ? '申请加入聊天室' : '加入聊天室' }}</span>
        </Button>
        <Button v-else class="mt-6 w-full" type="button" @click="emit('authenticate')">
          <LogIn :size="18" />
          <span>登录或注册</span>
        </Button>
      </form>
    </section>

    <section
      v-else
      class="relative flex min-h-0 flex-1 flex-col"
      @dragenter.prevent="handleDragEnter"
      @dragover.prevent
      @dragleave="handleDragLeave"
      @drop.prevent="handleDrop"
    >
      <Transition
        enter-active-class="transition duration-150 ease-out"
        enter-from-class="opacity-0"
        leave-active-class="transition duration-100 ease-in"
        leave-to-class="opacity-0"
      >
        <div v-if="dragActive" class="pointer-events-none absolute inset-3 z-30 grid place-items-center rounded-lg border-2 border-dashed border-primary bg-primary-50/95 text-primary shadow-lg">
          <div class="text-center">
            <UploadCloud class="mx-auto" :size="34" />
            <strong class="mt-2 block text-sm">释放以添加附件</strong>
          </div>
        </div>
      </Transition>
      <MessageList
        :room-id="room.id"
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
        @read="emit('read', $event)"
        @reply="startReply"
        @recall="recallMessage"
        @edit="startEdit"
        @forward="(message) => emit('forward', [message.message_id])"
        @toggle-select="toggleSelection"
        @load-older="emit('loadOlder')"
        @preview-image="previewImageId = $event.id"
        @view-profile="viewProfileUserId = $event"
      />
      <TransitionGroup
        v-if="typingDrafts.length && !selecting"
        tag="div"
        class="space-y-1 border-t border-surface-100 bg-surface-0 px-3 py-2 sm:px-7"
        enter-active-class="transition duration-150"
        enter-from-class="translate-y-1 opacity-0"
        leave-active-class="transition duration-100"
        leave-to-class="opacity-0"
      >
        <div v-for="draft in typingDrafts" :key="draft.user_id" class="flex min-w-0 items-center gap-2 text-xs">
          <strong class="shrink-0 text-primary">{{ draft.username }} 正在输入</strong>
          <span class="truncate text-muted-color">{{ draft.content }}</span>
        </div>
      </TransitionGroup>
      <div v-if="selecting" class="flex min-h-[68px] shrink-0 items-center justify-between gap-3 border-t border-surface-200 bg-surface-0 px-3 sm:px-7">
        <Button text rounded severity="secondary" aria-label="退出多选" title="退出多选" @click="closeSelection"><X :size="19" /></Button>
        <div class="min-w-0 flex-1">
          <span class="text-sm text-muted-color">已选择 {{ selectedMessageIds.length }} 条消息</span>
          <div v-if="downloadProgress" class="mt-2 flex items-center gap-2">
            <ProgressBar :value="downloadProgress.percent" :show-value="false" class="h-1.5 min-w-24 flex-1" />
            <span class="shrink-0 text-xs text-muted-color">{{ downloadProgress.completedFiles }}/{{ downloadProgress.totalFiles }}</span>
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
      <MessageComposer
        v-else
        ref="composerRef"
        :key="room.id"
        :replying-to="replyingTo"
        :editing-to="editingTo"
        :uploading="uploading"
        :send-shortcut="sendShortcut"
        :max-upload-bytes="maxUploadBytes"
        :participants="participants"
        :upload-progress="uploadProgress"
        :room-id="room.id"
        :token="token"
        :ai-enabled="aiEnabled"
        @cancel-reply="replyingTo = null"
        @cancel-edit="editingTo = null"
        @send="sendMessage"
        @edit="editMessage"
        @typing="emit('typing', $event)"
        @upload="uploadFiles"
      />
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
    />
    <ImageViewerGallery :images="galleryImages" :active-id="previewImageId" @close="previewImageId = ''" />
    <ProfileCardDialog
      :open="Boolean(viewProfileUserId)"
      :user-id="viewProfileUserId"
      :token="token"
      :room-id="room?.id"
      :current-user-id="currentUserId"
      @close="viewProfileUserId = ''"
    />
  </main>
</template>

<style scoped>
@keyframes poke-shake {
  0%, 100% { transform: translateX(0); }
  20% { transform: translateX(-8px); }
  40% { transform: translateX(7px); }
  60% { transform: translateX(-5px); }
  80% { transform: translateX(3px); }
}

.animate-poke-shake {
  animation: poke-shake 0.6s ease-in-out;
}

@media (prefers-reduced-motion: reduce) {
  .animate-poke-shake {
    animation: none;
  }
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

.fade-in {
  animation: fade-in 0.18s ease-out;
}

@media (prefers-reduced-motion: reduce) {
  .fade-in {
    animation: none;
  }
}
</style>
