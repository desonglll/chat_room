<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { Hash, Send } from 'lucide-vue-next'
import Button from 'primevue/button'
import Textarea from 'primevue/textarea'
import type { ConversationMentionRange, MentionableConversation } from '../assistantMentions'

defineProps<{
  prompt: string
  loading: boolean
  ready: boolean
  mentionRange: ConversationMentionRange | null
  candidates: MentionableConversation[]
  mentionIndex: number
}>()
const emit = defineEmits<{
  'update:prompt': [value: string]
  caret: [value: string, caret: number]
  keydown: [event: KeyboardEvent]
  choose: [room: MentionableConversation]
  submit: []
}>()
const input = ref<{ $el?: HTMLTextAreaElement } | null>(null)

function handleInput(event: Event): void {
  const textarea = event.target as HTMLTextAreaElement
  emit('update:prompt', textarea.value)
  emit('caret', textarea.value, textarea.selectionStart)
}

async function focusAt(caret: number): Promise<void> {
  await nextTick()
  input.value?.$el?.focus()
  input.value?.$el?.setSelectionRange(caret, caret)
}

defineExpose({ focusAt })
</script>

<template>
  <form
    id="ai-assistant-query-form"
    class="mx-auto flex w-full max-w-4xl items-end gap-2 border-t border-surface-200 px-3 py-2 sm:px-7 sm:py-3"
    @submit.prevent="emit('submit')"
  >
    <div class="relative min-w-0 flex-1">
      <div
        v-if="mentionRange"
        class="absolute bottom-[calc(100%+0.5rem)] left-0 z-20 w-[min(24rem,100%)] overflow-hidden rounded-md border border-surface-200 bg-surface-0 shadow-lg"
      >
        <p class="border-b border-surface-200 px-3 py-2 text-xs font-medium text-muted-color">选择会话</p>
        <ul v-if="candidates.length" role="listbox" class="max-h-64 overflow-y-auto p-1">
          <li v-for="(room, index) in candidates" :key="room.roomId" role="option">
            <button
              type="button"
              class="flex min-h-10 w-full items-center gap-2 rounded-sm px-2 text-left text-sm"
              :class="index === mentionIndex ? 'bg-primary-50 text-primary' : 'hover:bg-surface-100'"
              :aria-selected="index === mentionIndex"
              @mousedown.prevent="emit('choose', room)"
            >
              <Hash :size="15" class="shrink-0" /><span class="truncate">{{ room.title }}</span>
            </button>
          </li>
        </ul>
        <p v-else class="px-3 py-5 text-center text-sm text-muted-color">没有匹配的会话</p>
      </div>
      <Textarea
        ref="input"
        :model-value="prompt"
        auto-resize
        rows="1"
        maxlength="4000"
        fluid
        class="max-h-28 min-h-11 align-top"
        placeholder="发送消息，输入 @ 引用聊天会话"
        :disabled="loading || !ready"
        aria-label="向 AI 助手提问"
        aria-autocomplete="list"
        :aria-expanded="Boolean(mentionRange)"
        @input="handleInput"
        @click="handleInput"
        @keydown="emit('keydown', $event)"
      />
    </div>
    <Button
      type="submit"
      rounded
      aria-label="发送给 AI 助手"
      title="发送"
      class="size-11! shrink-0 p-0!"
      :loading="loading"
      :disabled="!ready || !prompt.trim()"
    >
      <Send v-if="!loading" :size="18" />
    </Button>
  </form>
</template>
