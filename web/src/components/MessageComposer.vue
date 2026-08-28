<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { LoaderCircle, Paperclip, Sparkles } from 'lucide-vue-next'
import Button from 'primevue/button'
import ComposerInput from './ComposerInput.vue'
import ComposerContext from './ComposerContext.vue'
import PendingAttachmentStrip from './PendingAttachmentStrip.vue'
import { shouldSubmitMessage } from '../composer'
import { formatUploadLimit } from '../api'
import { useAiComposerSuggestions } from '../composables/useAiComposerSuggestions'
import { useComposerMentions } from '../composables/useComposerMentions'
import { useConversationDraft, type ConversationDraftContext } from '../composables/useConversationDraft'
import type { BroadcastMessage, DisplayMessage, RoomMember, SendShortcut } from '../types'

interface PendingFile {
  id: number
  file: File
  previewUrl: string
  previewKind: 'image' | 'video' | 'file'
}

const props = defineProps<{
  replyingTo: BroadcastMessage | null
  editingTo: BroadcastMessage | null
  sendShortcut: SendShortcut
  maxUploadBytes: number
  participants: RoomMember[]
  draftContext: ConversationDraftContext
  roomId: string
  messages: DisplayMessage[]
  token: string
  password: string
  aiEnabled: boolean
  disabled: boolean
}>()

const emit = defineEmits<{
  send: [content: string, replyTo: string]
  upload: [files: File[], content: string, replyTo: string, isSensitive: boolean]
  edit: [messageId: string, content: string]
  typing: [content: string]
  cancelEdit: []
  'update:replyingTo': [message: BroadcastMessage | null]
}>()

const messageInput = ref<{
  element: () => HTMLTextAreaElement | null
  focus: () => void
  focusAt: (caret: number) => Promise<void>
} | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const draft = ref('')
const pendingFiles = ref<PendingFile[]>([])
const pendingFilesSensitive = ref(false)
const fileError = ref('')
let composing: boolean = false
let pendingId = 0

const {
  insert: insertMention,
  matches: mentionMatches,
  query: mentionQuery,
  update: updateMentionState,
} = useComposerMentions({
  draft,
  input: () => messageInput.value?.element() || null,
  participants: () => props.participants,
})

const {
  clear: clearAiSuggestions,
  current: aiCurrent,
  error: aiError,
  loading: aiLoading,
  open: openAiAssistant,
  remaining: aiRemaining,
  summary: aiSummary,
  useSuggestion,
} = useAiComposerSuggestions({
  draft,
  roomId: () => props.roomId,
  token: () => props.token,
  password: () => props.password,
  focusDraftEnd: () => {
    void nextTick(() => {
      const input = messageInput.value?.element()
      input?.focus()
      input?.setSelectionRange(draft.value.length, draft.value.length)
    })
  },
})

const canSend = computed(() => !props.disabled && Boolean(draft.value.trim() || pendingFiles.value.length))

watch(draft, (content) => emit('typing', content))

useConversationDraft(props, {
  draft,
  updateReply: (message) => emit('update:replyingTo', message),
  editingLoaded: () => {
    clearFiles()
    const input = messageInput.value?.element()
    input?.focus()
    input?.setSelectionRange(input.value.length, input.value.length)
  },
})

watch(
  () => props.replyingTo?.message_id,
  (messageId) => {
    if (messageId) void nextTick(() => messageInput.value?.focus())
  },
)

function addFiles(files: File[]): void {
  if (props.disabled || props.editingTo) return
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
    previewUrl: file.type.startsWith('image/') || file.type.startsWith('video/') ? URL.createObjectURL(file) : '',
    previewKind: file.type.startsWith('image/')
      ? ('image' as const)
      : file.type.startsWith('video/')
        ? ('video' as const)
        : ('file' as const),
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
  if (props.disabled || composing || !canSend.value) return
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
    emit(
      'upload',
      pendingFiles.value.map((item) => item.file),
      content,
      replyTo,
      pendingFilesSensitive.value,
    )
  } else {
    emit('send', content, replyTo)
  }
  draft.value = ''
  clearFiles()
  emit('update:replyingTo', null)
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

function setComposing(active: boolean): void {
  composing = active
}

function focus(): void {
  messageInput.value?.focus()
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
})

defineExpose({ addFiles, focus })
</script>

<template>
  <ComposerInput
    ref="messageInput"
    v-model="draft"
    :disabled="disabled"
    :can-send="canSend"
    placeholder="输入消息…"
    @submit="submitMessage"
    @keydown="onComposerKeydown"
    @paste="onPaste"
    @composition="setComposing"
    @caret="updateMentionState"
  >
    <template #context>
      <ComposerContext
        :editing="editingTo"
        :replying="replyingTo"
        @cancel-edit="cancelEditing"
        @cancel-reply="emit('update:replyingTo', null)"
      />
      <PendingAttachmentStrip v-model:sensitive="pendingFilesSensitive" :files="pendingFiles" @remove="removeFile" />
    </template>
    <template #leading-tools>
      <input ref="fileInput" class="sr-only" type="file" multiple @change="selectFiles" />
      <Button
        v-if="!editingTo"
        type="button"
        text
        rounded
        severity="secondary"
        class="cr-composer-tool !size-10 shrink-0"
        :disabled="disabled || pendingFiles.length >= 8"
        aria-label="添加附件"
        title="添加附件"
        @click="fileInput?.click()"
      >
        <Paperclip :size="19" />
      </Button>
    </template>
    <template #trailing-tools>
      <Button
        v-if="aiEnabled && !editingTo"
        type="button"
        text
        rounded
        severity="secondary"
        class="cr-composer-tool !size-10 shrink-0"
        :disabled="disabled"
        :aria-label="aiLoading ? 'AI 正在思考' : 'AI 助手：总结对话并建议回复'"
        :title="aiLoading ? 'AI 正在思考…' : 'AI 助手'"
        @click="openAiAssistant"
      >
        <LoaderCircle v-if="aiLoading" class="animate-spin" :size="19" />
        <Sparkles v-else :size="19" />
      </Button>
    </template>
    <template #popover>
      <ul
        v-if="mentionQuery !== null && mentionMatches.length"
        class="cr-composer-popover cr-glass absolute bottom-full left-0 z-10 mb-2 w-56 space-y-0.5 rounded-md p-1 shadow-lg"
      >
        <li v-for="member in mentionMatches" :key="member.user_id">
          <button
            type="button"
            class="flex min-h-10 w-full touch-manipulation items-center gap-2 rounded px-2 py-1.5 text-left text-sm outline-none transition-colors duration-[var(--cr-motion-normal)] [transition-timing-function:ease] hover:bg-surface-100/80 focus-visible:ring-2 focus-visible:ring-primary motion-reduce:transition-none"
            @click="insertMention(member.username)"
          >
            <span>{{ member.avatar_emoji || '👤' }}</span>
            <span class="min-w-0 truncate">{{ member.username }}</span>
          </button>
        </li>
      </ul>
      <div
        v-else-if="aiLoading || aiError || aiSummary || aiRemaining.length"
        class="cr-composer-popover cr-glass absolute bottom-full left-0 z-10 mb-2 flex max-w-full flex-wrap items-center gap-1.5 rounded-md p-2 shadow-lg"
      >
        <span v-if="aiLoading" class="flex items-center gap-1.5 text-xs text-muted-color">
          <Sparkles :size="13" class="animate-pulse text-primary" />
          {{ aiCurrent || aiRemaining.length ? '正在补充建议…' : 'AI 正在思考…' }}
        </span>
        <span v-if="aiError" class="text-xs text-danger">{{ aiError }}</span>
        <p v-if="aiSummary" class="w-full text-[11px] text-muted-color">{{ aiSummary }}</p>
        <button
          v-for="(suggestion, index) in aiRemaining"
          :key="index"
          type="button"
          class="min-h-8 touch-manipulation rounded-full border border-surface-200 px-2.5 py-1 text-xs outline-none transition-colors duration-[var(--cr-motion-normal)] [transition-timing-function:ease] hover:border-primary hover:bg-primary/5 focus-visible:ring-2 focus-visible:ring-primary motion-reduce:transition-none"
          @click="useSuggestion(suggestion, index)"
        >
          {{ suggestion }}
        </button>
      </div>
    </template>
    <template #footer>
      <small v-if="fileError" class="cr-composer-width block px-3 pt-1 text-right text-danger sm:px-1">
        {{ fileError }}
      </small>
    </template>
  </ComposerInput>
</template>
