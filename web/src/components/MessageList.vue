<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { ChevronDown, CornerUpLeft, Pencil, Undo2 } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Checkbox from 'primevue/checkbox'
import MessageAttachment from './MessageAttachment.vue'
import ReadReceiptStatus from './ReadReceiptStatus.vue'
import { useMessageViewport } from '../composables/useMessageViewport'
import type { Attachment, BroadcastMessage, DisplayMessage, ReadReceipt, ReplyPreview, RoomMember } from '../types'

const props = defineProps<{
  roomId: string
  messages: DisplayMessage[]
  readReceipts: ReadReceipt[]
  participants: RoomMember[]
  currentUserId: string
  visible: boolean
  historyReady: boolean
  selecting: boolean
  selectedMessageIds: string[]
}>()

const emit = defineEmits<{
  read: [messageId: string]
  reply: [message: BroadcastMessage]
  recall: [messageId: string]
  edit: [message: BroadcastMessage]
  toggleSelect: [messageId: string]
  previewImage: [attachment: Attachment]
}>()

const messageList = ref<HTMLElement | null>(null)
const highlightedId = ref('')
let highlightTimer: number | undefined

const broadcasts = computed(() => props.messages.filter(
  (message): message is BroadcastMessage => message.type === 'broadcast',
))

const readDetails = computed(() => {
  const positions = new Map(broadcasts.value.map((message, index) => [message.message_id, index]))
  const recipients = props.participants.filter((member) => member.user_id !== props.currentUserId)
  const details = new Map<string, { read: RoomMember[]; unread: RoomMember[] }>()
  if (!recipients.length) return details
  const receiptPositions = new Map(props.readReceipts.map((receipt) => [
    receipt.user_id,
    positions.get(receipt.message_id),
  ]))
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

const { handleScroll, scrollToFirstUnseen, unseenCount } = useMessageViewport({
  list: messageList,
  broadcasts,
  roomId: () => props.roomId,
  historyReady: () => props.historyReady,
  currentUserId: () => props.currentUserId,
  readReceipts: () => props.readReceipts,
  visible: () => props.visible,
  onRead: (messageId) => emit('read', messageId),
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

function scrollToMessage(messageId: string): void {
  const target = messageList.value?.querySelector<HTMLElement>(`[data-message-id="${messageId}"]`)
  if (!target) return
  target.scrollIntoView({ behavior: 'smooth', block: 'center' })
  highlightedId.value = messageId
  window.clearTimeout(highlightTimer)
  highlightTimer = window.setTimeout(() => { highlightedId.value = '' }, 1400)
}

onBeforeUnmount(() => {
  window.clearTimeout(highlightTimer)
})
</script>

<template>
  <div class="relative min-h-0 flex-1">
    <div ref="messageList" class="h-full overflow-y-auto px-3 py-5 sm:px-7" data-testid="message-list" aria-live="polite" @scroll.passive="handleScroll">
      <template v-for="message in messages" :key="message.type === 'broadcast' ? message.message_id : message.key">
      <div v-if="message.type === 'system'" class="my-4 flex items-center justify-center gap-3 text-center text-xs text-muted-color">
        <span class="h-px w-8 bg-surface-200" />
        <p>{{ message.content }}</p>
        <span class="h-px w-8 bg-surface-200" />
      </div>
      <div
        v-else
        class="group mb-4 flex items-start gap-2 rounded-lg transition-colors duration-200"
        :class="[
          message.sender_id === currentUserId ? 'flex-row-reverse justify-start' : 'justify-start',
          highlightedId === message.message_id ? 'bg-amber-100' : '',
        ]"
        :data-message-id="message.message_id"
      >
        <Checkbox
          v-if="selecting && message.attachment && !message.recalled_at"
          class="mt-7 shrink-0"
          binary
          :model-value="selectedMessageIds.includes(message.message_id)"
          :aria-label="`选择附件 ${message.attachment.file_name}`"
          @update:model-value="emit('toggleSelect', message.message_id)"
        />
        <Avatar
          :label="avatarLabel(message)"
          shape="circle"
          class="mt-5 shrink-0 bg-surface-200! text-surface-700!"
        />
        <div class="max-w-[86%] sm:max-w-[72%] lg:max-w-[720px]">
          <div class="mb-1 flex items-center gap-2 text-xs text-muted-color" :class="{ 'justify-end': message.sender_id === currentUserId }">
            <strong>{{ message.sender_id === currentUserId ? '你' : message.sender }}</strong>
            <time>{{ formatTime(message.timestamp) }}</time>
            <button
              v-if="!message.recalled_at && !selecting"
              type="button"
              class="grid size-6 place-items-center rounded text-muted-color opacity-100 transition hover:bg-surface-200 hover:text-primary sm:opacity-0 sm:group-hover:opacity-100"
              aria-label="回复消息"
              title="回复"
              @click="emit('reply', message)"
            >
              <CornerUpLeft :size="14" />
            </button>
            <button
              v-if="message.sender_id === currentUserId && message.content && !message.recalled_at && !selecting"
              type="button"
              class="grid size-6 place-items-center rounded text-muted-color opacity-100 transition hover:bg-surface-200 hover:text-primary sm:opacity-0 sm:group-hover:opacity-100"
              aria-label="编辑消息"
              title="编辑"
              @click="emit('edit', message)"
            >
              <Pencil :size="14" />
            </button>
            <button
              v-if="message.sender_id === currentUserId && !message.recalled_at && !selecting"
              type="button"
              class="grid size-6 place-items-center rounded text-muted-color opacity-100 transition hover:bg-red-50 hover:text-red-600 sm:opacity-0 sm:group-hover:opacity-100"
              aria-label="撤回消息"
              title="撤回"
              @click="emit('recall', message.message_id)"
            >
              <Undo2 :size="14" />
            </button>
          </div>
          <button
            v-if="message.reply_to"
            type="button"
            class="block w-full overflow-hidden rounded-md border-l-[3px] border-primary bg-surface-100 px-2.5 py-2 text-left text-surface-600 hover:bg-surface-200"
            @click="scrollToMessage(message.reply_to.message_id)"
          >
            <strong class="block truncate text-[11px] text-primary">{{ message.reply_to.sender }}</strong>
            <span class="mt-0.5 block truncate text-xs">{{ replySummary(message.reply_to) }}</span>
          </button>
          <p v-if="message.recalled_at" class="mt-1 rounded-md border border-dashed border-surface-300 px-3 py-2 text-sm italic text-muted-color">
            {{ message.sender_id === currentUserId ? '你撤回了一条消息' : `${message.sender} 撤回了一条消息` }}
          </p>
          <MessageAttachment v-else-if="message.attachment" class="mt-1" :attachment="message.attachment" @preview-image="emit('previewImage', $event)" />
          <p
            v-if="message.content && !message.recalled_at"
            class="mt-1 whitespace-pre-wrap break-words rounded-lg border px-3 py-2.5 text-[15px] leading-6 shadow-sm"
            :class="message.sender_id === currentUserId
              ? 'border-primary bg-primary text-primary-contrast'
              : 'border-surface-200 bg-surface-0 text-surface-900'"
          >
            {{ message.content }}
          </p>
          <small v-if="message.edited_at && !message.recalled_at" class="mt-0.5 block text-right text-[10px] text-muted-color">已编辑</small>
          <ReadReceiptStatus
            v-if="message.sender_id === currentUserId && !message.recalled_at && readDetails.get(message.message_id)"
            :read="readDetails.get(message.message_id)!.read"
            :unread="readDetails.get(message.message_id)!.unread"
          />
        </div>
      </div>
      </template>
    </div>
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="translate-y-2 opacity-0"
      leave-active-class="transition duration-150 ease-in"
      leave-to-class="translate-y-2 opacity-0"
    >
      <button
        v-if="unseenCount"
        type="button"
        class="absolute bottom-3 left-1/2 z-10 flex -translate-x-1/2 cursor-pointer items-center gap-1.5 rounded-full border border-primary-200 bg-surface-0 px-3 py-2 text-xs font-semibold text-primary shadow-lg transition hover:-translate-y-0.5 hover:shadow-xl"
        @click="scrollToFirstUnseen"
      >
        <ChevronDown :size="15" />
        下方 {{ unseenCount }} 条新消息
      </button>
    </Transition>
  </div>
</template>
