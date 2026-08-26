<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Check, ChevronDown, Circle, CircleX, LoaderCircle } from 'lucide-vue-next'
import type { AiCitationSource, AiRunTraceStep } from '../types'
import type { AiUiMessage } from '../aiUi'

const props = defineProps<{ message: AiUiMessage; now: number }>()
const active = computed(() => props.message.status === 'pending' || props.message.status === 'streaming')
const expanded = ref(active.value)
const lastStep = computed(() => props.message.trace.at(-1))
const summary = computed(() => {
  const first = props.message.trace[0]
  const last = lastStep.value
  if (!first || !last) return ''
  if (active.value) {
    const stage = elapsed(last)
    const total = duration(Date.parse(first.started_at), props.now)
    return `${last.label} · 本阶段 ${stage} · 总计 ${total}`
  }
  return `${props.message.trace.length} 步 · ${duration(Date.parse(first.started_at), Date.parse(last.completed_at || last.started_at))}`
})

watch(active, (value) => {
  expanded.value = value
})

function elapsed(step: AiRunTraceStep): string {
  const start = Date.parse(step.started_at)
  const end = step.completed_at ? Date.parse(step.completed_at) : props.now
  return duration(start, end)
}

function duration(start: number, end: number): string {
  const milliseconds = Math.max(0, end - start)
  if (milliseconds < 1_000) return `${milliseconds} ms`
  return `${(milliseconds / 1_000).toFixed(milliseconds < 10_000 ? 1 : 0)} s`
}

function scoreLabel(score: number | null | undefined, kind = 'vector'): string {
  if (score === null || score === undefined) return ''
  return `${kind === 'rerank' ? '重排分' : '向量分'} ${score.toFixed(3)}`
}

function sourcesForStep(step: AiRunTraceStep): AiCitationSource[] {
  if (step.key === 'context_attachments') {
    return props.message.sources.filter((source) => source.score_kind === 'attachment')
  }
  const ragSources = props.message.sources.filter((source) => source.score_kind !== 'attachment')
  if (step.key === 'rag_selected') return ragSources
  if (step.key === 'rag_candidates' && !props.message.trace.some((item) => item.key === 'rag_selected')) {
    return ragSources
  }
  return []
}
</script>

<template>
  <div v-if="message.trace.length" class="mt-3 border-y border-surface-200 py-1.5 text-xs">
    <button
      type="button"
      class="flex min-h-9 w-full items-center gap-2 rounded-sm px-1.5 text-left text-surface-700 hover:bg-surface-50 focus-visible:outline-2 focus-visible:outline-primary"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <LoaderCircle v-if="active" :size="14" class="animate-spin text-primary motion-reduce:animate-none" />
      <CircleX v-else-if="message.status === 'failed'" :size="14" class="text-red-600" />
      <Check v-else :size="14" class="text-primary" />
      <span class="font-medium">执行过程</span>
      <span class="min-w-0 flex-1 truncate text-muted-color">{{ summary }}</span>
      <ChevronDown :size="14" class="transition-transform" :class="expanded ? 'rotate-180' : ''" />
    </button>

    <ol v-if="expanded" class="grid grid-cols-1 gap-x-5 sm:grid-cols-2">
      <li
        v-for="(step, index) in message.trace"
        :key="`${step.key}-${index}`"
        class="border-t border-surface-100 px-1.5 py-1.5"
        :class="sourcesForStep(step).length ? 'sm:col-span-2' : ''"
      >
        <div class="grid min-w-0 grid-cols-[1rem_minmax(0,1fr)_auto] items-start gap-x-1.5">
          <span class="grid size-4 place-items-center pt-0.5">
            <LoaderCircle
              v-if="!step.completed_at && active"
              :size="12"
              class="animate-spin text-primary motion-reduce:animate-none"
            />
            <CircleX v-else-if="step.key === 'run_failed'" :size="12" class="text-red-600" />
            <Check v-else-if="step.completed_at" :size="11" class="text-primary" />
            <Circle v-else :size="9" class="text-surface-400" />
          </span>
          <div class="min-w-0 flex-1">
            <strong class="font-medium leading-4 text-surface-800">{{ index + 1 }}. {{ step.label }}</strong>
            <p v-if="step.detail" class="break-words text-[11px] leading-4 text-muted-color">
              {{ step.detail }}
            </p>
          </div>
          <time class="shrink-0 tabular-nums text-[10px] text-muted-color">{{ elapsed(step) }}</time>
        </div>

        <ol v-if="sourcesForStep(step).length" class="mt-1 grid grid-cols-1 gap-x-5 sm:grid-cols-2">
          <li v-for="source in sourcesForStep(step)" :key="source.label" class="border-t border-surface-100 py-1.5">
            <div class="flex flex-wrap items-center gap-x-2 text-[10px] text-muted-color">
              <span class="font-mono font-semibold text-primary">[{{ source.label }}]</span>
              <span class="font-medium text-surface-700">{{ source.sender }}</span>
              <span v-if="scoreLabel(source.score, source.score_kind)">
                {{ scoreLabel(source.score, source.score_kind) }}
              </span>
            </div>
            <p class="line-clamp-2 break-words text-[11px] leading-4 text-surface-600" :title="source.excerpt">
              {{ source.excerpt }}
            </p>
          </li>
        </ol>
      </li>
    </ol>
  </div>
</template>
