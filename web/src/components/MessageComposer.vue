<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { File, LoaderCircle, Paperclip, Pencil, Send, Smile, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Popover from 'primevue/popover'
import Textarea from 'primevue/textarea'
import { shouldSubmitMessage } from '../composer'
import { formatUploadLimit } from '../api'
import type { BroadcastMessage, SendShortcut } from '../types'

interface PendingFile {
  id: number
  file: File
  previewUrl: string
  previewKind: 'image' | 'video' | 'file'
}

const props = defineProps<{
  replyingTo: BroadcastMessage | null
  editingTo: BroadcastMessage | null
  uploading: boolean
  sendShortcut: SendShortcut
  maxUploadBytes: number
}>()

const emit = defineEmits<{
  send: [content: string, replyTo: string]
  upload: [files: File[], content: string, replyTo: string]
  edit: [messageId: string, content: string]
  typing: [content: string]
  cancelReply: []
  cancelEdit: []
}>()

const messageInput = ref<{ $el: HTMLTextAreaElement } | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const emojiPopover = ref()
const draft = ref('')
const pendingFiles = ref<PendingFile[]>([])
const fileError = ref('')
let composing = false
let pendingId = 0
const EMOJIS = ['😀', '😄', '😂', '😊', '😍', '🥳', '😎', '🤓', '🤔', '😅', '😭', '😡', '👍', '👏', '🙏', '💪', '🎉', '❤️', '🔥', '✨', '🚀', '☕', '💡', '✅']

const canSend = computed(() => Boolean(draft.value.trim() || pendingFiles.value.length))

watch(draft, (content) => emit('typing', content))

watch(() => props.replyingTo?.message_id, (messageId) => {
  if (messageId) void nextTick(() => messageInput.value?.$el.focus())
})

watch(() => props.editingTo?.message_id, (messageId) => {
  if (!messageId || !props.editingTo) return
  clearFiles()
  draft.value = props.editingTo.content
  void nextTick(() => {
    const input = messageInput.value?.$el
    input?.focus()
    input?.setSelectionRange(input.value.length, input.value.length)
  })
})

function replySummary(message: BroadcastMessage): string {
  if (message.recalled_at) return '消息已撤回'
  return message.content || (message.attachment ? `[附件] ${message.attachment.file_name}` : '[消息]')
}

function addFiles(files: File[]): void {
  if (props.editingTo) return
  fileError.value = ''
  const remaining = Math.max(0, 8 - pendingFiles.value.length)
  const valid = files.filter((file) => {
    if (file.size > props.maxUploadBytes) {
      fileError.value = `${file.name} 超过 ${formatUploadLimit(props.maxUploadBytes)}，未添加`
      return false
    }
    if (!file.size) {
      fileError.value = `${file.name} 是空文件，未添加`
      return false
    }
    return true
  })
  const additions = valid.slice(0, remaining).map((file) => ({
    id: ++pendingId,
    file,
    previewUrl: file.type.startsWith('image/') || file.type.startsWith('video/')
      ? URL.createObjectURL(file)
      : '',
    previewKind: file.type.startsWith('image/')
      ? 'image' as const
      : file.type.startsWith('video/') ? 'video' as const : 'file' as const,
  }))
  pendingFiles.value.push(...additions)
}

function removeFile(id: number): void {
  const target = pendingFiles.value.find((item) => item.id === id)
  if (target?.previewUrl) URL.revokeObjectURL(target.previewUrl)
  pendingFiles.value = pendingFiles.value.filter((item) => item.id !== id)
}

function clearFiles(): void {
  for (const item of pendingFiles.value) {
    if (item.previewUrl) URL.revokeObjectURL(item.previewUrl)
  }
  pendingFiles.value = []
}

function submitMessage(): void {
  if (composing || props.uploading || !canSend.value) return
  const content = draft.value.trim()
  if (props.editingTo) {
    if (!content) return
    emit('edit', props.editingTo.message_id, content)
    draft.value = ''
    emit('cancelEdit')
    return
  }
  const replyTo = props.replyingTo?.message_id || ''
  if (pendingFiles.value.length) {
    emit('upload', pendingFiles.value.map((item) => item.file), content, replyTo)
  } else {
    emit('send', content, replyTo)
  }
  draft.value = ''
  clearFiles()
  emit('cancelReply')
}

function onComposerKeydown(event: KeyboardEvent): void {
  if (!shouldSubmitMessage(event, composing, props.sendShortcut)) return
  event.preventDefault()
  submitMessage()
}

function focus(): void {
  messageInput.value?.$el.focus()
}

function insertEmoji(emoji: string): void {
  const input = messageInput.value?.$el
  const start = input?.selectionStart ?? draft.value.length
  const end = input?.selectionEnd ?? start
  draft.value = `${draft.value.slice(0, start)}${emoji}${draft.value.slice(end)}`
  emojiPopover.value?.hide()
  void nextTick(() => {
    const nextInput = messageInput.value?.$el
    nextInput?.focus()
    nextInput?.setSelectionRange(start + emoji.length, start + emoji.length)
  })
}

function onCompositionStart(): void {
  composing = true
}

function onCompositionEnd(): void {
  composing = false
}

function selectFiles(event: Event): void {
  const input = event.target as HTMLInputElement
  addFiles(Array.from(input.files || []))
  input.value = ''
}

function onPaste(event: ClipboardEvent): void {
  if (props.editingTo) return
  const files = Array.from(event.clipboardData?.items || [])
    .filter((item) => item.kind === 'file')
    .map((item) => item.getAsFile())
    .filter((file): file is File => Boolean(file))
  if (!files.length) return
  event.preventDefault()
  addFiles(files)
}

function cancelEditing(): void {
  draft.value = ''
  emit('cancelEdit')
}

onBeforeUnmount(clearFiles)

defineExpose({ addFiles, focus })
</script>

<template>
  <form class="shrink-0 border-t border-surface-200 bg-surface-0" data-testid="chat-form" @submit.prevent="submitMessage">
    <div v-if="editingTo" class="flex items-center gap-3 px-3 pt-3 sm:px-7">
      <Pencil :size="16" class="shrink-0 text-primary" />
      <div class="min-w-0 flex-1">
        <strong class="block truncate text-xs text-primary">编辑已发送消息</strong>
        <span class="mt-0.5 block truncate text-xs text-muted-color">{{ editingTo.content }}</span>
      </div>
      <Button type="button" text rounded severity="secondary" aria-label="取消编辑" title="取消编辑" @click="cancelEditing">
        <X :size="17" />
      </Button>
    </div>
    <div v-if="replyingTo" class="flex items-center gap-3 px-3 pt-3 sm:px-7">
      <div class="min-w-0 flex-1 border-l-[3px] border-primary pl-2.5">
        <strong class="block truncate text-xs text-primary">回复 {{ replyingTo.sender }}</strong>
        <span class="mt-0.5 block truncate text-xs text-muted-color">{{ replySummary(replyingTo) }}</span>
      </div>
      <Button type="button" text rounded severity="secondary" aria-label="取消回复" title="取消回复" @click="emit('cancelReply')">
        <X :size="17" />
      </Button>
    </div>

    <TransitionGroup
      v-if="pendingFiles.length"
      tag="div"
      class="flex gap-2 overflow-x-auto px-3 pt-3 sm:px-7"
      aria-label="待发送附件"
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="translate-y-1 opacity-0"
      leave-active-class="transition duration-150 ease-in"
      leave-to-class="scale-95 opacity-0"
    >
      <div v-for="item in pendingFiles" :key="item.id" class="relative grid size-[72px] shrink-0 place-items-center overflow-hidden rounded-lg border border-surface-200 bg-surface-100 text-muted-color">
        <img v-if="item.previewKind === 'image'" class="size-full object-cover" :src="item.previewUrl" :alt="item.file.name">
        <video v-else-if="item.previewKind === 'video'" class="size-full object-cover" :src="item.previewUrl" muted playsinline preload="metadata" />
        <File v-else :size="24" />
        <span class="absolute inset-x-1 bottom-1 truncate rounded-sm bg-surface-900/75 px-1 py-0.5 text-[9px] text-white">{{ item.file.name }}</span>
        <button type="button" class="absolute right-1 top-1 grid size-6 place-items-center rounded bg-white/90 text-surface-600 shadow-sm hover:bg-white hover:text-surface-900" aria-label="移除附件" title="移除附件" @click="removeFile(item.id)">
          <X :size="15" />
        </button>
      </div>
    </TransitionGroup>

    <div class="flex items-end gap-2 px-3 py-3 sm:px-7">
      <input ref="fileInput" class="sr-only" type="file" multiple @change="selectFiles">
      <Button
        v-if="!editingTo"
        type="button"
        text
        rounded
        severity="secondary"
        :disabled="uploading || pendingFiles.length >= 8"
        aria-label="添加附件"
        title="添加附件"
        @click="fileInput?.click()"
      >
        <LoaderCircle v-if="uploading" class="animate-spin" :size="19" />
        <Paperclip v-else :size="19" />
      </Button>
      <Button type="button" text rounded severity="secondary" aria-label="插入表情" title="表情" @click="emojiPopover.toggle($event)">
        <Smile :size="19" />
      </Button>
      <Popover ref="emojiPopover">
        <div class="grid w-64 grid-cols-6 gap-1" aria-label="表情列表">
          <button
            v-for="emoji in EMOJIS"
            :key="emoji"
            type="button"
            class="grid aspect-square place-items-center rounded-md text-xl transition hover:-translate-y-0.5 hover:bg-surface-100"
            :aria-label="`插入 ${emoji}`"
            @click="insertEmoji(emoji)"
          >
            {{ emoji }}
          </button>
        </div>
      </Popover>
      <label class="sr-only" for="messageInput">消息</label>
      <Textarea
        id="messageInput"
        ref="messageInput"
        v-model="draft"
        rows="1"
        maxlength="4096"
        auto-resize
        placeholder="输入消息"
        class="max-h-32 min-h-10 min-w-0 flex-1 overflow-y-auto! [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        @paste="onPaste"
        @compositionstart="onCompositionStart"
        @compositionend="onCompositionEnd"
        @keydown="onComposerKeydown"
      />
      <Button type="submit" rounded :disabled="!canSend || uploading" aria-label="发送消息" title="发送消息">
        <Send :size="18" />
      </Button>
    </div>
    <small v-if="fileError" class="block px-3 pb-2 text-right text-red-600 sm:px-7">{{ fileError }}</small>
  </form>
</template>
