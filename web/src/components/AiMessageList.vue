<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { Bot } from 'lucide-vue-next'
import type { AiUiMessage } from '../aiUi'
import MarkdownContent from './MarkdownContent.vue'

defineProps<{ messages: AiUiMessage[]; roomTitle: string }>()

const viewport = ref<HTMLElement | null>(null)
let pendingFrame: number | null = null

async function scrollToLatest(smooth = false): Promise<void> {
  await nextTick()
  if (!viewport.value) return
  viewport.value.scrollTo({
    top: viewport.value.scrollHeight,
    behavior: smooth ? 'smooth' : 'auto',
  })
}

function scrollToLatestSoon(): void {
  if (pendingFrame !== null) return
  pendingFrame = requestAnimationFrame(() => {
    pendingFrame = null
    void scrollToLatest()
  })
}

defineExpose({ scrollToLatest, scrollToLatestSoon })
</script>

<template>
  <div ref="viewport" class="min-h-0 overflow-y-auto px-4 py-6 sm:px-7" aria-live="polite">
    <div v-if="!messages.length" class="grid min-h-full place-items-center text-center text-muted-color">
      <div>
        <Bot :size="32" class="mx-auto opacity-35" />
        <p class="mt-3 text-sm">可以直接提问，也可以输入 @ 引用一个聊天会话</p>
      </div>
    </div>
    <ol v-else class="mx-auto w-full max-w-3xl space-y-6">
      <li
        v-for="message in messages"
        :key="message.id"
        class="flex"
        :class="message.role === 'user' ? 'justify-end' : 'justify-start'"
      >
        <article
          class="min-w-0 text-sm leading-6"
          :class="
            message.role === 'user'
              ? 'max-w-[82%] rounded-md bg-primary px-3.5 py-2.5 text-primary-contrast'
              : 'w-full max-w-[46rem] text-surface-900'
          "
        >
          <p v-if="message.role === 'user'" class="whitespace-pre-wrap break-words">{{ message.content }}</p>
          <MarkdownContent v-else-if="message.content" :content="message.content" />
          <div
            v-else-if="message.status === 'pending' || message.status === 'streaming'"
            class="flex min-h-6 items-center gap-2 text-muted-color"
          >
            <span
              class="size-3.5 animate-spin rounded-full border-2 border-surface-300 border-t-primary motion-reduce:animate-none"
            />
            {{ message.status === 'streaming' ? '正在回答' : '正在连接' }}
          </div>
          <p v-else-if="message.status === 'failed'" class="text-sm text-red-600">AI 请求失败，请稍后重试</p>
          <p
            v-if="message.role === 'assistant' && (roomTitle || message.context_message_count)"
            class="mt-2 text-[10px] text-muted-color"
          >
            <template v-if="roomTitle">{{ roomTitle }}</template>
            <template v-if="message.context_message_count">
              · 本次注入 {{ message.context_message_count }} 条消息 · TOON + RAG</template
            >
          </p>
        </article>
      </li>
    </ol>
  </div>
</template>
