<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { Bot, LocateFixed } from 'lucide-vue-next'
import { RouterLink } from 'vue-router'
import { aiContextUsage, aiSourceRoute, type AiUiMessage } from '../aiUi'
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

function formatSourceTime(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'short', timeStyle: 'short' }).format(new Date(value))
}
</script>

<template>
  <div ref="viewport" class="min-h-0 overflow-y-auto px-3 py-4 sm:px-7 sm:py-6" aria-live="polite">
    <div v-if="!messages.length" class="grid min-h-full place-items-center text-center text-muted-color">
      <div>
        <Bot :size="32" class="mx-auto opacity-35" />
        <p class="mt-3 text-sm">可以直接提问，也可以输入 @ 引用一个聊天会话</p>
      </div>
    </div>
    <ol v-else class="mx-auto w-full max-w-3xl space-y-4 sm:space-y-6">
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
              ? 'max-w-[88%] rounded-md bg-primary px-3.5 py-2.5 text-primary-contrast sm:max-w-[82%]'
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
          <div
            v-if="message.role === 'assistant' && message.sources.length"
            class="mt-3 border-t border-surface-200 pt-3"
          >
            <p class="mb-1.5 text-xs font-medium text-surface-700">参考来源</p>
            <ol class="divide-y divide-surface-200 overflow-hidden rounded-md border border-surface-200">
              <li v-for="source in message.sources" :key="source.message_id">
                <RouterLink
                  :to="aiSourceRoute(source)"
                  class="flex min-w-0 items-start gap-2 px-2.5 py-2 text-left transition-colors hover:bg-surface-50 focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-primary"
                  :title="`定位到 ${source.label} 原文`"
                >
                  <span class="shrink-0 font-mono text-xs font-semibold text-primary">[{{ source.label }}]</span>
                  <span class="min-w-0 flex-1">
                    <span class="flex flex-wrap items-center gap-x-2 text-[11px] text-muted-color">
                      <span class="font-medium text-surface-700">{{ source.sender }}</span>
                      <time :datetime="source.sent_at">{{ formatSourceTime(source.sent_at) }}</time>
                    </span>
                    <span class="mt-0.5 line-clamp-2 block break-words text-xs text-surface-600">{{
                      source.excerpt
                    }}</span>
                  </span>
                  <LocateFixed :size="14" class="mt-0.5 shrink-0 text-muted-color" aria-hidden="true" />
                </RouterLink>
              </li>
            </ol>
          </div>
          <p
            v-if="message.role === 'assistant' && (roomTitle || message.context_message_count)"
            class="mt-2 text-[10px] text-muted-color"
          >
            <template v-if="roomTitle">{{ roomTitle }}</template>
            <template v-if="message.context_message_count">
              · 最近上下文
              {{ aiContextUsage(message.context_message_count, message.retrieved_message_count).recent }} 条
            </template>
            <template v-if="aiContextUsage(message.context_message_count, message.retrieved_message_count).retrieved">
              · 全房间检索证据
              {{ aiContextUsage(message.context_message_count, message.retrieved_message_count).retrieved }} 条
            </template>
          </p>
        </article>
      </li>
    </ol>
  </div>
</template>
