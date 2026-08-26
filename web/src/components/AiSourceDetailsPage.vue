<script setup lang="ts">
import { computed } from 'vue'
import { ArrowLeft, LocateFixed } from 'lucide-vue-next'
import Button from 'primevue/button'
import { RouterLink } from 'vue-router'
import {
  aiSourceRoute,
  citedAiSources,
  formatLocalDateTime,
  localTimeZone,
  ragAiSources,
  type AiUiMessage,
} from '../aiUi'
import MessageAttachment from './MessageAttachment.vue'

const props = defineProps<{ message: AiUiMessage; roomTitle: string }>()
const emit = defineEmits<{ back: [] }>()

const citedLabels = computed(
  () => new Set(citedAiSources(props.message.content, props.message.sources).map((source) => source.label)),
)
const attachmentCount = computed(() => props.message.sources.filter((source) => source.attachment).length)

function scoreLabel(score: number | null | undefined, kind = 'vector'): string {
  if (score === null || score === undefined) return ''
  return `${kind === 'rerank' ? '重排分' : '向量分'} ${score.toFixed(3)}`
}

function previewAttachment(downloadUrl: string): void {
  window.open(downloadUrl, '_blank', 'noopener,noreferrer')
}
</script>

<template>
  <section class="grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)]">
    <header class="flex min-h-14 items-center gap-2 border-b border-surface-200 px-3 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回回答" title="返回回答" @click="emit('back')">
        <ArrowLeft :size="18" />
      </Button>
      <div class="min-w-0">
        <h2 class="text-sm font-semibold text-surface-900">来源与证据</h2>
        <p class="truncate text-[11px] text-muted-color">
          {{ citedLabels.size }} 条已引用 · {{ ragAiSources(message.sources).length }} 条检索证据 ·
          {{ attachmentCount }} 个附件 · {{ localTimeZone() }}
        </p>
      </div>
    </header>

    <div class="min-h-0 overflow-y-auto px-3 py-4 sm:px-7 sm:py-6">
      <div class="mx-auto w-full max-w-3xl">
        <p v-if="roomTitle" class="mb-3 text-xs text-muted-color">{{ roomTitle }}</p>
        <ol class="border-y border-surface-200">
          <li
            v-for="source in message.sources"
            :key="source.label"
            class="grid gap-2 border-b border-surface-200 px-1 py-4 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_auto] sm:gap-4"
          >
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
                <span class="font-mono font-semibold text-primary">[{{ source.label }}]</span>
                <span class="font-medium text-surface-800">{{ source.sender }}</span>
                <span
                  v-if="citedLabels.has(source.label)"
                  class="rounded-sm bg-primary-50 px-1.5 py-0.5 text-[10px] font-medium text-primary"
                >
                  已引用
                </span>
                <span v-if="scoreLabel(source.score, source.score_kind)" class="text-[11px] text-muted-color">
                  {{ scoreLabel(source.score, source.score_kind) }}
                </span>
              </div>
              <p class="mt-1.5 break-words text-sm leading-6 text-surface-700">{{ source.excerpt }}</p>
              <MessageAttachment
                v-if="source.attachment"
                :attachment="source.attachment"
                class="mt-2 w-full! max-w-md!"
                @preview-image="previewAttachment($event.download_url)"
              />
              <time :datetime="source.sent_at" class="mt-1 block text-[11px] text-muted-color">
                {{ formatLocalDateTime(source.sent_at) }}
              </time>
            </div>
            <RouterLink
              :to="aiSourceRoute(source)"
              class="inline-flex h-9 items-center gap-1.5 self-center rounded-sm px-2 text-xs font-medium text-primary hover:bg-primary-50 focus-visible:outline-2 focus-visible:outline-primary"
              :title="`定位到 ${source.label} 原文`"
            >
              <LocateFixed :size="14" aria-hidden="true" />
              定位原文
            </RouterLink>
          </li>
        </ol>
      </div>
    </div>
  </section>
</template>
