<script setup lang="ts">
import { computed } from 'vue'
import { BookOpen, Bot, ChevronRight, LoaderCircle } from 'lucide-vue-next'
import { aiContextUsage, citedAiSources, ragAiSources, referencedAiAttachments, type AiUiMessage } from '../aiUi'
import AiCitedAttachments from './AiCitedAttachments.vue'
import AiRunTrace from './AiRunTrace.vue'
import MarkdownContent from './MarkdownContent.vue'

const props = defineProps<{ message: AiUiMessage; roomTitle: string; now: number }>()
const emit = defineEmits<{ sources: [] }>()
const active = computed(() => ['pending', 'streaming'].includes(props.message.status))
const citedCount = computed(() => citedAiSources(props.message.content, props.message.sources).length)
</script>

<template>
  <div class="flex w-full items-start gap-3">
    <span
      class="mt-0.5 grid size-8 shrink-0 place-items-center rounded-md bg-primary-50 text-primary"
      aria-hidden="true"
    >
      <Bot :size="17" />
    </span>
    <article class="min-w-0 w-full max-w-[46rem] text-sm leading-6 text-surface-900">
      <header class="mb-2 flex min-h-6 items-center gap-2">
        <strong class="text-xs font-semibold text-surface-700">AI 助手</strong>
        <span v-if="active" class="flex items-center gap-1.5 text-[11px] text-muted-color">
          <LoaderCircle :size="13" class="animate-spin motion-reduce:animate-none" />正在回答
        </span>
      </header>
      <MarkdownContent
        v-if="message.content"
        :content="message.content"
        :sources="message.sources"
        :class="{ 'ai-streaming-markdown': active }"
      />
      <p v-else-if="active" class="flex min-h-8 items-center text-sm text-muted-color">正在组织回答…</p>
      <p v-else-if="message.status === 'failed'" class="text-sm text-red-600">AI 请求失败，请稍后重试</p>
      <AiCitedAttachments :content="message.content" :sources="message.sources" />
      <AiRunTrace :message="message" :now="now" />
      <div v-if="message.sources.length" class="mt-3 border-t border-surface-200 pt-3">
        <button
          type="button"
          class="flex min-h-10 w-full items-center gap-2 rounded-sm px-2 text-left text-xs text-surface-700 transition-colors hover:bg-surface-50 focus-visible:outline-2 focus-visible:outline-primary"
          @click="emit('sources')"
        >
          <BookOpen :size="15" class="shrink-0 text-primary" aria-hidden="true" />
          <span class="font-medium">来源与证据</span>
          <span class="text-muted-color">
            <template v-if="citedCount">
              {{ citedCount }} 条已引用 · {{ ragAiSources(message.sources).length }} 条检索证据 ·
              {{ referencedAiAttachments(message.content, message.sources).length }} 个附件
            </template>
            <template v-else>{{ ragAiSources(message.sources).length }} 条历史检索证据 · 回答未标注引用</template>
          </span>
          <ChevronRight :size="15" class="ml-auto shrink-0 text-muted-color" aria-hidden="true" />
        </button>
      </div>
      <p v-if="roomTitle || message.context_message_count" class="mt-2 text-[10px] text-muted-color">
        <template v-if="roomTitle">{{ roomTitle }} · 本次输入</template>
        <template v-if="message.context_message_count">
          · 聊天室上下文
          {{ aiContextUsage(message.context_message_count, message.retrieved_message_count).recent }} 条
        </template>
        <template v-if="aiContextUsage(message.context_message_count, message.retrieved_message_count).retrieved">
          + 历史检索证据
          {{ aiContextUsage(message.context_message_count, message.retrieved_message_count).retrieved }} 条
        </template>
      </p>
    </article>
  </div>
</template>

<style scoped>
@keyframes ai-stream-caret {
  0%,
  48% {
    opacity: 1;
  }
  49%,
  100% {
    opacity: 0.2;
  }
}

:deep(.ai-streaming-markdown > :last-child)::after {
  display: inline-block;
  width: 0.42em;
  height: 1em;
  margin-left: 0.22em;
  border-radius: 1px;
  background: var(--p-primary-color);
  content: '';
  vertical-align: -0.12em;
  animation: ai-stream-caret 900ms steps(1, end) infinite;
}

@media (prefers-reduced-motion: reduce) {
  :deep(.ai-streaming-markdown > :last-child)::after {
    animation: none;
    opacity: 0.65;
  }
}
</style>
