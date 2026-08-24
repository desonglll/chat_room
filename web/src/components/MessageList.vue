<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { ChevronDown } from 'lucide-vue-next'
import Checkbox from 'primevue/checkbox'
import ContextMenu from 'primevue/contextmenu'
import type { MenuItem } from 'primevue/menuitem'
import MessageAttachment from './MessageAttachment.vue'
import MessageDeliveryStatus from './MessageDeliveryStatus.vue'
import MessageHoverActions from './MessageHoverActions.vue'
import MessageReactionChips from './MessageReactionChips.vue'
import PendingUploadMessage from './PendingUploadMessage.vue'
import AppAvatar from './AppAvatar.vue'
import ReadReceiptStatus from './ReadReceiptStatus.vue'
import { useMessageViewport } from '../composables/useMessageViewport'
import { preferredScrollBehavior } from '../motionPreference'
import type { Attachment, BroadcastMessage, DisplayMessage, ReadReceipt, ReplyPreview, RoomMember } from '../types'

const props = defineProps<{
  roomId: string
  direct: boolean
  unreadCount: number
  messages: DisplayMessage[]
  readReceipts: ReadReceipt[]
  participants: RoomMember[]
  currentUserId: string
  visible: boolean
  historyReady: boolean
  selecting: boolean
  selectedMessageIds: string[]
  loadingOlder: boolean
  hasMoreHistory: boolean
  ensureMessage: (messageId: string) => Promise<boolean>
}>()

const emit = defineEmits<{
  read: [messageId: string]
  reply: [message: BroadcastMessage]
  recall: [messageId: string]
  edit: [message: BroadcastMessage]
  forward: [message: BroadcastMessage]
  favorite: [message: BroadcastMessage]
  toggleSelect: [messageId: string]
  previewImage: [attachment: Attachment]
  viewProfile: [userId: string]
  poke: [userId: string]
  retry: [messageId: string]
  cancelUpload: [key: string]
  retryUpload: [key: string]
  loadOlder: []
  reaction: [messageId: string, emoji: string, active: boolean]
}>()

const messageList = ref<HTMLElement | null>(null)
const highlightedId = ref('')
const contextMenu = ref()
const contextMenuItems = ref<MenuItem[]>([])
const avatarContextMenu = ref()
const avatarContextMenuItems = ref<MenuItem[]>([])
let highlightTimer: number | undefined

function copyText(content: string): void {
  void navigator.clipboard.writeText(content).catch(() => window.prompt('复制消息内容', content))
}

function openContextMenu(event: MouseEvent, message: BroadcastMessage): void {
  const isOwn = message.sender_id === props.currentUserId
  const items: MenuItem[] = []
  if (!isSettled(message)) {
    if (message.delivery_state === 'failed') {
      items.push({ label: '重新发送', command: () => emit('retry', message.message_id) })
    }
    if (message.content) items.push({ label: '复制', command: () => copyText(message.content) })
    contextMenuItems.value = items
    contextMenu.value?.show(event)
    return
  }
  if (!message.recalled_at) {
    items.push({ label: '回复', command: () => emit('reply', message) })
    if (message.content) items.push({ label: '复制', command: () => copyText(message.content) })
    items.push({ label: '转发', command: () => emit('forward', message) })
    items.push({ label: '收藏', command: () => emit('favorite', message) })
  }
  if (isOwn && message.content && !message.recalled_at) {
    items.push({ label: '编辑', command: () => emit('edit', message) })
  }
  if (isOwn && !message.recalled_at) {
    items.push({ label: '撤回', command: () => emit('recall', message.message_id) })
  }
  if (isOwn && message.recalled_at) {
    items.push({ label: '重新编辑', command: () => emit('edit', message) })
  }
  if (!items.length) return
  contextMenuItems.value = items
  contextMenu.value?.show(event)
}

function isSettled(message: BroadcastMessage): boolean {
  return !message.delivery_state || message.delivery_state === 'sent'
}

function toggleReaction(message: BroadcastMessage, emoji: string): void {
  const active = !(message.reactions || [])
    .find((reaction) => reaction.emoji === emoji)
    ?.user_ids.includes(props.currentUserId)
  emit('reaction', message.message_id, emoji, active)
}

function openAvatarContextMenu(event: MouseEvent, message: BroadcastMessage): void {
  if (!message.sender_id) return
  const items: MenuItem[] = [{ label: '查看资料', command: () => emit('viewProfile', message.sender_id as string) }]
  if (message.sender_id !== props.currentUserId) {
    items.unshift({ label: '拍一拍', command: () => emit('poke', message.sender_id as string) })
  }
  avatarContextMenuItems.value = items
  avatarContextMenu.value?.show(event)
}

const broadcasts = computed(() =>
  props.messages.filter((message): message is BroadcastMessage => message.type === 'broadcast'),
)
const uploadKeys = computed(() =>
  props.messages.filter((message) => message.type === 'upload').map((message) => message.key),
)

const readDetails = computed(() => {
  const positions = new Map(broadcasts.value.map((message, index) => [message.message_id, index]))
  const recipients = props.participants.filter((member) => member.user_id !== props.currentUserId)
  const details = new Map<string, { read: RoomMember[]; unread: RoomMember[] }>()
  if (!recipients.length) return details
  const receiptPositions = new Map(
    props.readReceipts.map((receipt) => [receipt.user_id, positions.get(receipt.message_id)]),
  )
  for (const [index, message] of broadcasts.value.entries()) {
    if (message.sender_id !== props.currentUserId) continue
    const read: RoomMember[] = []
    const unread: RoomMember[] = []
    for (const member of recipients) {
      const receiptPosition = receiptPositions.get(member.user_id)
      ;(receiptPosition !== undefined && receiptPosition >= index ? read : unread).push(member)
    }
    details.set(message.message_id, {
      read: read.sort((left, right) => left.username.localeCompare(right.username, 'zh-CN')),
      unread: unread.sort((left, right) => left.username.localeCompare(right.username, 'zh-CN')),
    })
  }
  return details
})

const { awayFromBottom, handleScroll, scrollToBottom, unseenCount, viewportReady } = useMessageViewport({
  list: messageList,
  broadcasts,
  roomId: () => props.roomId,
  unreadCount: () => props.unreadCount,
  historyReady: () => props.historyReady,
  currentUserId: () => props.currentUserId,
  readReceipts: () => props.readReceipts,
  visible: () => props.visible,
  onRead: (messageId) => emit('read', messageId),
  onLoadOlder: () => {
    if (!props.loadingOlder && props.hasMoreHistory) emit('loadOlder')
  },
})

watch(uploadKeys, async (nextKeys, previousKeys) => {
  const appended = nextKeys.some((key) => !previousKeys.includes(key))
  if (!appended || awayFromBottom.value) return
  await nextTick()
  const list = messageList.value
  if (list) list.scrollTo({ top: list.scrollHeight, behavior: preferredScrollBehavior() })
})

function formatTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(date)
}

function replySummary(reply: ReplyPreview): string {
  if (reply.recalled) return '消息已撤回'
  return reply.content || (reply.attachment_file_name ? `[附件] ${reply.attachment_file_name}` : '[消息]')
}

function avatarLabel(message: BroadcastMessage): string {
  return message.sender_avatar || message.sender.slice(0, 1).toUpperCase()
}

// Telegram-style grouping: consecutive messages from the same sender within a
// few minutes collapse into one visual block — avatar/name shown once, tighter
// spacing between bubbles in the block instead of full message spacing.
const GROUP_GAP_MS = 5 * 60 * 1000

function groupKey(message: DisplayMessage): string | null {
  if (message.type !== 'broadcast') return null
  return message.sender_id ?? message.sender
}

function sameGroup(a: DisplayMessage | undefined, b: DisplayMessage | undefined): boolean {
  if (!a || !b || a.type !== 'broadcast' || b.type !== 'broadcast') return false
  if (groupKey(a) !== groupKey(b)) return false
  return Math.abs(new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()) < GROUP_GAP_MS
}

function isGroupStart(index: number): boolean {
  return !sameGroup(props.messages[index - 1], props.messages[index])
}

function isGroupEnd(index: number): boolean {
  return !sameGroup(props.messages[index], props.messages[index + 1])
}

function displayKey(message: DisplayMessage): string {
  return message.type === 'broadcast' ? message.message_id : message.key
}

interface ContentSegment {
  text: string
  mention: boolean
}

function contentSegments(content: string): ContentSegment[] {
  if (!props.participants.length || !content.includes('@')) return [{ text: content, mention: false }]
  const usernames = props.participants
    .map((member) => member.username)
    .filter(Boolean)
    .sort((left, right) => right.length - left.length)
  if (!usernames.length) return [{ text: content, mention: false }]
  const pattern = new RegExp(
    `@(?:${usernames.map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')})(?![\\w])`,
    'g',
  )
  const segments: ContentSegment[] = []
  let lastIndex = 0
  for (const match of content.matchAll(pattern)) {
    const index = match.index ?? 0
    if (index > lastIndex) segments.push({ text: content.slice(lastIndex, index), mention: false })
    segments.push({ text: match[0], mention: true })
    lastIndex = index + match[0].length
  }
  if (lastIndex < content.length) segments.push({ text: content.slice(lastIndex), mention: false })
  return segments
}

function findMessage(messageId: string): HTMLElement | undefined {
  return Array.from(messageList.value?.querySelectorAll<HTMLElement>('[data-message-id]') || []).find(
    (element) => element.dataset.messageId === messageId,
  )
}

async function scrollToMessage(messageId: string): Promise<void> {
  let target = findMessage(messageId)
  if (!target && (await props.ensureMessage(messageId))) {
    await nextTick()
    target = findMessage(messageId)
  }
  if (!target) return
  target.scrollIntoView({ behavior: preferredScrollBehavior(), block: 'center' })
  highlightedId.value = messageId
  window.clearTimeout(highlightTimer)
  highlightTimer = window.setTimeout(() => {
    highlightedId.value = ''
  }, 1400)
}

defineExpose({ scrollToMessage })

onBeforeUnmount(() => {
  window.clearTimeout(highlightTimer)
})
</script>

<template>
  <div class="cr-message-viewport relative min-h-0 flex-1">
    <div
      ref="messageList"
      class="cr-message-list h-full overscroll-contain overflow-y-auto px-3 py-4 transition-opacity duration-100 [scrollbar-gutter:stable] motion-reduce:transition-none sm:px-5"
      :class="viewportReady ? 'visible opacity-100' : 'invisible opacity-0'"
      data-testid="message-list"
      aria-live="polite"
      :aria-hidden="!viewportReady"
      @scroll.passive="handleScroll"
    >
      <div v-if="loadingOlder" class="mb-3 flex justify-center">
        <span
          class="size-4 animate-spin rounded-full border-2 border-surface-300 border-t-primary motion-reduce:animate-none"
        />
      </div>
      <template v-for="(message, index) in messages" :key="displayKey(message)">
        <div
          v-if="message.type === 'system'"
          class="cr-system-message mx-auto my-5 flex w-full items-center justify-center gap-3 text-center text-xs"
          :class="{ 'motion-system': message.motion === 'system' }"
        >
          <span class="h-px w-8 bg-surface-200" />
          <p>{{ message.content }}</p>
          <span class="h-px w-8 bg-surface-200" />
        </div>
        <PendingUploadMessage
          v-else-if="message.type === 'upload'"
          :message="message"
          @cancel="emit('cancelUpload', $event)"
          @retry="emit('retryUpload', $event)"
        />
        <div
          v-else
          class="cr-message-row group mx-auto flex w-full items-start gap-2"
          :class="[
            message.sender_id === currentUserId
              ? 'cr-message-row--own justify-end'
              : 'cr-message-row--other justify-start',
            highlightedId === message.message_id ? 'message-highlight' : '',
            message.motion === 'incoming' ? 'motion-incoming' : '',
            message.motion === 'outgoing' ? 'motion-outgoing' : '',
            isGroupEnd(index) ? 'mb-3' : 'mb-0.5',
          ]"
          :data-message-id="message.message_id"
          @contextmenu.prevent="openContextMenu($event, message)"
        >
          <Checkbox
            v-if="selecting && !message.recalled_at && isSettled(message)"
            class="mt-7 shrink-0"
            binary
            :model-value="selectedMessageIds.includes(message.message_id)"
            aria-label="选择消息"
            @update:model-value="emit('toggleSelect', message.message_id)"
          />
          <button
            v-if="message.sender_id !== currentUserId"
            type="button"
            class="cr-message-avatar mt-5 shrink-0 cursor-pointer rounded-full outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
            :class="{ invisible: !isGroupStart(index) }"
            aria-label="查看用户资料"
            title="查看资料"
            @click="message.sender_id && emit('viewProfile', message.sender_id)"
            @contextmenu.stop.prevent="openAvatarContextMenu($event, message)"
          >
            <AppAvatar
              :avatar="message.sender_avatar"
              :fallback="avatarLabel(message)"
              :color-key="message.sender_id || message.sender"
              class="text-white!"
            />
          </button>
          <div class="cr-message-stack">
            <div
              v-if="isGroupStart(index)"
              class="cr-message-meta mb-1 flex items-center gap-2 text-xs"
              :class="{ 'justify-end': message.sender_id === currentUserId }"
            >
              <strong v-if="!direct && message.sender_id !== currentUserId">{{ message.sender }}</strong>
              <time>{{ formatTime(message.timestamp) }}</time>
            </div>
            <button
              v-if="message.reply_to"
              type="button"
              class="cr-reply-preview block w-full overflow-hidden rounded-md px-2.5 py-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-primary"
              @click="scrollToMessage(message.reply_to.message_id)"
            >
              <strong class="block truncate text-[11px] text-primary">{{ message.reply_to.sender }}</strong>
              <span class="mt-0.5 block truncate text-xs">{{ replySummary(message.reply_to) }}</span>
            </button>
            <div
              v-if="!message.recalled_at && message.forwarded_from"
              class="mt-1 text-[11px] text-muted-color"
              :class="{ 'text-right': message.sender_id === currentUserId }"
            >
              转发自 {{ message.forwarded_from.sender }} · {{ message.forwarded_from.room_name }}
            </div>
            <div
              v-if="message.recalled_at"
              class="mt-1 flex items-center gap-2 rounded-md border border-dashed border-surface-300 px-3 py-2 text-sm italic text-muted-color"
            >
              <span class="flex-1">
                {{ message.sender_id === currentUserId ? '你撤回了一条消息' : `${message.sender} 撤回了一条消息` }}
              </span>
              <button
                v-if="message.sender_id === currentUserId && message.content"
                type="button"
                class="shrink-0 rounded-sm not-italic text-primary outline-none hover:underline focus-visible:ring-2 focus-visible:ring-primary"
                @click="emit('edit', message)"
              >
                重新编辑
              </button>
            </div>
            <MessageAttachment
              v-else-if="message.attachment"
              class="mt-1"
              :attachment="message.attachment"
              @preview-image="emit('previewImage', $event)"
            />
            <p
              v-if="message.content && !message.recalled_at"
              class="cr-message-bubble mt-1 whitespace-pre-wrap break-words px-3 py-2.5 text-[15px] leading-6"
              :class="
                message.sender_id === currentUserId
                  ? [
                      'cr-bubble-outgoing cr-message-bubble--outgoing',
                      isGroupEnd(index) ? 'cr-message-bubble--end rounded-br-sm' : '',
                    ]
                  : [
                      'cr-bubble-incoming cr-message-bubble--incoming',
                      isGroupEnd(index) ? 'cr-message-bubble--end rounded-bl-sm' : '',
                    ]
              "
            >
              <template v-for="(segment, index) in contentSegments(message.content)" :key="index">
                <strong
                  v-if="segment.mention"
                  class="font-semibold text-primary"
                  :class="{ 'text-inherit! underline': message.sender_id === currentUserId }"
                  >{{ segment.text }}</strong
                >
                <template v-else>{{ segment.text }}</template>
              </template>
            </p>
            <MessageHoverActions
              :enabled="!message.recalled_at && !selecting && isSettled(message)"
              @reaction="toggleReaction(message, $event)"
              @reply="emit('reply', message)"
              @forward="emit('forward', message)"
              @favorite="emit('favorite', message)"
            />
            <MessageReactionChips
              v-if="!message.recalled_at && isSettled(message)"
              :reactions="message.reactions || []"
              :participants="participants"
              :current-user-id="currentUserId"
              :own="message.sender_id === currentUserId"
              @toggle="(emoji, active) => emit('reaction', message.message_id, emoji, active)"
            />
            <small
              v-if="message.edited_at && !message.recalled_at"
              class="mt-0.5 block text-right text-[10px] text-muted-color"
              >已编辑</small
            >
            <ReadReceiptStatus
              v-if="
                message.sender_id === currentUserId &&
                !message.recalled_at &&
                isSettled(message) &&
                readDetails.get(message.message_id)
              "
              :read="readDetails.get(message.message_id)!.read"
              :unread="readDetails.get(message.message_id)!.unread"
              :direct="direct"
            />
            <MessageDeliveryStatus
              v-else-if="message.sender_id === currentUserId && message.delivery_state"
              :state="message.delivery_state"
              @retry="emit('retry', message.message_id)"
            />
          </div>
        </div>
      </template>
    </div>
    <Transition
      enter-active-class="transition-[opacity,transform] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
      enter-from-class="translate-y-2 opacity-0"
      leave-active-class="transition-[opacity,transform] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
      leave-to-class="translate-y-2 opacity-0"
    >
      <button
        v-if="awayFromBottom"
        type="button"
        class="cr-scroll-latest absolute bottom-3 left-1/2 z-10 flex min-h-10 -translate-x-1/2 touch-manipulation cursor-pointer items-center gap-1.5 rounded-full px-3 py-2 text-xs font-semibold outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 motion-reduce:transform-none motion-reduce:transition-none"
        @click="scrollToBottom"
      >
        <ChevronDown :size="15" aria-hidden="true" />
        {{ unseenCount ? `下方 ${unseenCount} 条新消息` : '回到最新消息' }}
      </button>
    </Transition>
    <ContextMenu ref="contextMenu" :model="contextMenuItems" />
    <ContextMenu ref="avatarContextMenu" :model="avatarContextMenuItems" />
  </div>
</template>
