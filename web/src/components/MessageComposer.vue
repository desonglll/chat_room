<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { File, LoaderCircle, Paperclip, Pencil, Send, Smile, Sparkles, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Popover from 'primevue/popover'
import Textarea from 'primevue/textarea'
import EmojiPicker from './EmojiPicker.vue'
import { shouldSubmitMessage } from '../composer'
import { formatUploadLimit, getAiSuggestions } from '../api'
import type { BroadcastMessage, RoomMember, SendShortcut } from '../types'

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
  participants: RoomMember[]
  roomId: string
  token: string
  aiEnabled: boolean
}>()

const emit = defineEmits<{
  send: [content: string, replyTo: string]
  upload: [files: File[], content: string, replyTo: string, isSensitive: boolean]
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
const pendingFilesSensitive = ref(false)
const fileError = ref('')
const mentionQuery = ref<string | null>(null)
const aiLoading = ref(false)
const aiError = ref('')
const aiSummary = ref('')
// The suggestion currently reflected in `draft` (so a chip click knows what
// to hand back to the chip row) and the remaining, not-yet-used suggestions.
const aiCurrent = ref('')
const aiRemaining = ref<string[]>([])
let aiTypewriterTimer: number | undefined
let typewriting = false
let mentionStart = 0
let composing = false
let pendingId = 0

const canSend = computed(() => Boolean(draft.value.trim() || pendingFiles.value.length))
const mentionMatches = computed(() => {
  if (mentionQuery.value === null) return []
  const query = mentionQuery.value.toLowerCase()
  return props.participants
    .filter((member) => member.username.toLowerCase().startsWith(query))
    .slice(0, 6)
})

function updateMentionState(): void {
  const input = messageInput.value?.$el
  if (!input) {
    mentionQuery.value = null
    return
  }
  const caret = input.selectionStart ?? draft.value.length
  const match = draft.value.slice(0, caret).match(/(?:^|\s)@([^\s@]*)$/)
  if (!match) {
    mentionQuery.value = null
    return
  }
  mentionStart = caret - match[1].length - 1
  mentionQuery.value = match[1]
}

function insertMention(username: string): void {
  const caret = messageInput.value?.$el.selectionStart ?? draft.value.length
  const before = draft.value.slice(0, mentionStart)
  const after = draft.value.slice(caret)
  const insertion = `@${username} `
  draft.value = `${before}${insertion}${after}`
  mentionQuery.value = null
  void nextTick(() => {
    const nextInput = messageInput.value?.$el
    const position = before.length + insertion.length
    nextInput?.focus()
    nextInput?.setSelectionRange(position, position)
  })
}

watch(draft, (content) => emit('typing', content))

// Suggestions are only "live" while the draft still matches what AI put
// there — any manual edit (including the user retyping over a suggestion)
// drops the chip row, since it no longer reflects an option the user can
// swap back in cleanly.
watch(draft, (content) => {
  if (typewriting) return
  if ((aiCurrent.value || aiRemaining.value.length) && content !== aiCurrent.value) {
    aiCurrent.value = ''
    aiRemaining.value = []
  }
})

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
  pendingFilesSensitive.value = false
}

function submitMessage(): void {
  if (composing || props.uploading || !canSend.value) return
  mentionQuery.value = null
  clearAiSuggestions()
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
    emit('upload', pendingFiles.value.map((item) => item.file), content, replyTo, pendingFilesSensitive.value)
  } else {
    emit('send', content, replyTo)
  }
  draft.value = ''
  clearFiles()
  emit('cancelReply')
}

function onComposerKeydown(event: KeyboardEvent): void {
  if (mentionQuery.value !== null && mentionMatches.value.length) {
    if (event.key === 'Escape') {
      mentionQuery.value = null
      return
    }
    if (event.key === 'Enter' || event.key === 'Tab') {
      event.preventDefault()
      insertMention(mentionMatches.value[0].username)
      return
    }
  }
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

function clearAiTypewriter(): void {
  window.clearInterval(aiTypewriterTimer)
  aiTypewriterTimer = undefined
  typewriting = false
}

function clearAiSuggestions(): void {
  clearAiTypewriter()
  aiCurrent.value = ''
  aiRemaining.value = []
  aiError.value = ''
  aiSummary.value = ''
}

// Reveals `text` into `draft` a few characters at a time — the "something is
// typing" feel the AI reply is meant to have, rather than appearing instantly.
function typewriteIntoDraft(text: string): void {
  clearAiTypewriter()
  typewriting = true
  draft.value = ''
  let index = 0
  aiTypewriterTimer = window.setInterval(() => {
    index = Math.min(text.length, index + 2)
    draft.value = text.slice(0, index)
    if (index >= text.length) clearAiTypewriter()
  }, 16)
}

async function openAiAssistant(): Promise<void> {
  if (aiLoading.value) return
  aiError.value = ''
  aiSummary.value = ''
  aiCurrent.value = ''
  aiRemaining.value = []
  aiLoading.value = true
  try {
    const result = await getAiSuggestions(props.roomId, props.token)
    aiSummary.value = result.summary
    const [first, ...rest] = result.suggestions
    aiRemaining.value = rest
    if (first) {
      aiCurrent.value = first
      typewriteIntoDraft(first)
    }
  } catch (error) {
    aiError.value = error instanceof Error ? error.message : 'AI 助手不可用'
  } finally {
    aiLoading.value = false
  }
}

// Swaps a chip into the draft, trading places with whatever suggestion was
// there before — so the user can keep cycling between options.
function useSuggestion(suggestion: string, index: number): void {
  clearAiTypewriter()
  const rest = aiRemaining.value.filter((_, candidate) => candidate !== index)
  if (aiCurrent.value) rest.unshift(aiCurrent.value)
  aiRemaining.value = rest
  aiCurrent.value = suggestion
  draft.value = suggestion
  void nextTick(() => {
    const input = messageInput.value?.$el
    input?.focus()
    input?.setSelectionRange(draft.value.length, draft.value.length)
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

onBeforeUnmount(() => {
  clearFiles()
  clearAiTypewriter()
})

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
      <div v-for="item in pendingFiles" :key="item.id" class="relative grid size-[72px] shrink-0 place-items-center overflow-hidden rounded-xl bg-surface-100 text-muted-color shadow-sm">
        <img v-if="item.previewKind === 'image'" class="size-full object-cover" :src="item.previewUrl" :alt="item.file.name">
        <video v-else-if="item.previewKind === 'video'" class="size-full object-cover" :src="item.previewUrl" muted playsinline preload="metadata" />
        <File v-else :size="24" />
        <span class="absolute inset-x-1 bottom-1 truncate rounded-sm bg-surface-900/75 px-1 py-0.5 text-[9px] text-white">{{ item.file.name }}</span>
        <button type="button" class="absolute right-1 top-1 grid size-6 place-items-center rounded bg-surface-0/90 text-surface-600 shadow-sm hover:bg-surface-0 hover:text-surface-900" aria-label="移除附件" title="移除附件" @click="removeFile(item.id)">
          <X :size="15" />
        </button>
      </div>
    </TransitionGroup>
    <label v-if="pendingFiles.length" class="flex items-center gap-2 px-3 pt-2 text-xs text-muted-color sm:px-7">
      <Checkbox v-model="pendingFilesSensitive" binary input-id="sensitiveContent" />
      <span for="sensitiveContent">包含敏感内容，接收方需点击确认才能查看</span>
    </label>

    <div class="flex items-center gap-1 px-3 py-3 sm:px-7">
      <input ref="fileInput" class="sr-only" type="file" multiple @change="selectFiles">
      <Button
        v-if="!editingTo"
        type="button"
        text
        rounded
        severity="secondary"
        class="!size-10 shrink-0"
        :disabled="uploading || pendingFiles.length >= 8"
        aria-label="添加附件"
        title="添加附件"
        @click="fileInput?.click()"
      >
        <LoaderCircle v-if="uploading" class="animate-spin" :size="19" />
        <Paperclip v-else :size="19" />
      </Button>
      <Button type="button" text rounded severity="secondary" class="!size-10 shrink-0" aria-label="插入表情" title="表情" @click="emojiPopover.toggle($event)">
        <Smile :size="19" />
      </Button>
      <Popover ref="emojiPopover">
        <EmojiPicker @select="insertEmoji" />
      </Popover>
      <Button
        v-if="aiEnabled && !editingTo"
        type="button"
        text
        rounded
        severity="secondary"
        class="!size-10 shrink-0"
        :aria-label="aiLoading ? 'AI 正在思考' : 'AI 助手：总结对话并建议回复'"
        :title="aiLoading ? 'AI 正在思考…' : 'AI 助手'"
        @click="openAiAssistant"
      >
        <LoaderCircle v-if="aiLoading" class="animate-spin" :size="19" />
        <Sparkles v-else :size="19" />
      </Button>
      <label class="sr-only" for="messageInput">消息</label>
      <div class="relative min-w-0 flex-1">
        <Textarea
          id="messageInput"
          ref="messageInput"
          v-model="draft"
          rows="1"
          maxlength="4096"
          auto-resize
          placeholder="输入消息"
          class="max-h-32 min-h-10 w-full overflow-y-auto! [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
          @paste="onPaste"
          @compositionstart="onCompositionStart"
          @compositionend="onCompositionEnd"
          @keydown="onComposerKeydown"
          @input="updateMentionState"
          @click="updateMentionState"
          @keyup="updateMentionState"
        />
        <ul
          v-if="mentionQuery !== null && mentionMatches.length"
          class="cr-glass absolute bottom-full left-0 z-10 mb-1.5 w-56 space-y-0.5 rounded-xl p-1 shadow-lg"
        >
          <li v-for="member in mentionMatches" :key="member.user_id">
            <button
              type="button"
              class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm transition-colors hover:bg-surface-100/80"
              @click="insertMention(member.username)"
            >
              <span>{{ member.avatar_emoji || '👤' }}</span>
              <span class="min-w-0 truncate">{{ member.username }}</span>
            </button>
          </li>
        </ul>
        <div
          v-else-if="aiLoading || aiError || aiSummary || aiRemaining.length"
          class="cr-glass absolute bottom-full left-0 z-10 mb-1.5 flex max-w-full flex-wrap items-center gap-1.5 rounded-xl p-2 shadow-lg"
        >
          <span v-if="aiLoading" class="flex items-center gap-1.5 text-xs text-muted-color">
            <Sparkles :size="13" class="animate-pulse text-primary" />
            AI 正在思考…
          </span>
          <span v-else-if="aiError" class="text-xs text-danger">{{ aiError }}</span>
          <template v-else>
            <p v-if="aiSummary" class="w-full text-[11px] text-muted-color">{{ aiSummary }}</p>
            <button
              v-for="(suggestion, index) in aiRemaining"
              :key="index"
              type="button"
              class="rounded-full border border-surface-200 px-2.5 py-1 text-xs hover:border-primary hover:bg-primary/5"
              @click="useSuggestion(suggestion, index)"
            >
              {{ suggestion }}
            </button>
          </template>
        </div>
      </div>
      <Button type="submit" rounded class="!size-10 shrink-0 transition-transform active:scale-90" :disabled="!canSend || uploading" aria-label="发送消息" title="发送消息">
        <Send :size="18" />
      </Button>
    </div>
    <small v-if="fileError" class="block px-3 pb-2 text-right text-danger sm:px-7">{{ fileError }}</small>
  </form>
</template>
