<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Bookmark, Check, Clock3, LocateFixed, Sparkles, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Drawer from 'primevue/drawer'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Select from 'primevue/select'
import Tag from 'primevue/tag'
import { listAiModels } from '../aiThreadApi'
import {
  createAiExtraction,
  getAiExtraction,
  updateAiExtractionCandidate,
  type AiExtractionCandidate,
  type AiExtractionRun,
} from '../aiExtractionApi'
import type { AiModelChoice } from '../types'

const props = defineProps<{ open: boolean; roomId: string; token: string; password: string }>()
const emit = defineEmits<{ close: []; locate: [messageId: string] }>()
const fromAt = ref('')
const toAt = ref('')
const models = ref<AiModelChoice[]>([])
const modelId = ref<string | null>(null)
const run = ref<AiExtractionRun | null>(null)
const loading = ref(false)
const savingId = ref('')
const error = ref('')
let pollTimer: number | undefined
const proposedCount = computed(() => run.value?.candidates.filter((item) => item.status === 'proposed').length || 0)
const modelOptions = computed(() =>
  models.value.filter((model) => model.ready).map((model) => ({ label: model.label, value: model.id })),
)

watch(
  () => [props.open, props.roomId] as const,
  ([open]) => {
    stopPolling()
    if (!open) return
    resetRange()
    run.value = null
    error.value = ''
    void loadModels()
  },
)
onBeforeUnmount(stopPolling)

function resetRange(): void {
  const end = new Date()
  const start = new Date(end.getTime() - 24 * 60 * 60 * 1000)
  fromAt.value = localInput(start)
  toAt.value = localInput(end)
}

function localInput(date: Date): string {
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16)
}

async function loadModels(): Promise<void> {
  try {
    models.value = await listAiModels(props.token)
    modelId.value = modelOptions.value[0]?.value || null
  } catch {
    models.value = []
  }
}

async function extract(): Promise<void> {
  const start = new Date(fromAt.value)
  const end = new Date(toAt.value)
  if (!fromAt.value || !toAt.value || start >= end) {
    error.value = '请选择有效的开始和结束时间'
    return
  }
  loading.value = true
  error.value = ''
  run.value = null
  try {
    run.value = await createAiExtraction(
      props.roomId,
      props.token,
      props.password,
      start.toISOString(),
      end.toISOString(),
      modelId.value,
    )
    schedulePoll()
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : 'AI 提取失败'
  } finally {
    loading.value = false
  }
}

function schedulePoll(): void {
  stopPolling()
  if (!run.value || !['queued', 'running'].includes(run.value.status) || !props.open) return
  pollTimer = window.setTimeout(() => void poll(), 800)
}

async function poll(): Promise<void> {
  if (!run.value) return
  try {
    run.value = await getAiExtraction(run.value.id, props.token, props.password)
    if (run.value.status === 'failed') error.value = run.value.error_message || 'AI 提取失败'
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '读取提取结果失败'
    return
  }
  schedulePoll()
}

function stopPolling(): void {
  if (pollTimer !== undefined) window.clearTimeout(pollTimer)
  pollTimer = undefined
}

async function update(candidate: AiExtractionCandidate, action: 'confirm' | 'dismiss'): Promise<void> {
  if (!run.value || candidate.status !== 'proposed') return
  savingId.value = candidate.id
  error.value = ''
  try {
    const updated = await updateAiExtractionCandidate(candidate, action, props.token, props.password)
    run.value.candidates = run.value.candidates.map((item) => (item.id === updated.id ? updated : item))
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '处理候选项失败'
    if (caught instanceof Error && 'status' in caught && caught.status === 409) await poll()
  } finally {
    savingId.value = ''
  }
}

function locate(messageId: string): void {
  emit('locate', messageId)
  emit('close')
}

function candidateLabel(candidate: AiExtractionCandidate): string {
  return candidate.kind === 'task' ? '候选待办' : '候选决定'
}
</script>

<template>
  <Drawer
    :visible="open"
    position="right"
    class="w-full! sm:w-[34rem]!"
    :dismissable="true"
    @update:visible="!$event && emit('close')"
  >
    <template #header>
      <div class="flex min-w-0 items-center gap-2">
        <Sparkles :size="20" class="shrink-0 text-primary" />
        <strong class="truncate text-base">提取决定与待办</strong>
      </div>
    </template>

    <div class="flex min-h-full flex-col gap-4">
      <form class="space-y-3 border-y border-surface-200 py-4" @submit.prevent="extract">
        <div class="grid gap-3 sm:grid-cols-2">
          <label class="block">
            <span class="mb-1 block text-xs text-muted-color">开始时间</span>
            <InputText v-model="fromAt" class="w-full" type="datetime-local" required />
          </label>
          <label class="block">
            <span class="mb-1 block text-xs text-muted-color">结束时间</span>
            <InputText v-model="toAt" class="w-full" type="datetime-local" required />
          </label>
        </div>
        <label v-if="modelOptions.length > 1" class="block">
          <span class="mb-1 block text-xs text-muted-color">AI 模型</span>
          <Select v-model="modelId" class="w-full" :options="modelOptions" option-label="label" option-value="value" />
        </label>
        <Button
          type="submit"
          class="w-full"
          :loading="loading"
          :disabled="Boolean(run && ['queued', 'running'].includes(run.status))"
        >
          <Sparkles :size="17" />
          <span>开始提取</span>
        </Button>
      </form>

      <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
      <div
        v-if="run && ['queued', 'running'].includes(run.status)"
        class="flex flex-1 items-center justify-center gap-2 py-16 text-sm text-muted-color"
      >
        <Clock3 :size="18" />正在分析所选消息…
      </div>
      <div v-else-if="run?.status === 'completed' && !run.candidates.length" class="py-16 text-center">
        <Sparkles :size="30" class="mx-auto mb-3 text-muted-color" />
        <strong class="text-sm">没有发现明确的决定或待办</strong>
      </div>
      <template v-else-if="run?.status === 'completed'">
        <div class="flex items-center justify-between gap-3 text-xs text-muted-color">
          <span>分析 {{ run.message_count || 0 }} 条消息</span>
          <span>{{ proposedCount }} 项待确认</span>
        </div>
        <ol class="divide-y divide-surface-200 border-y border-surface-200">
          <li v-for="candidate in run.candidates" :key="candidate.id" class="py-4">
            <div class="flex items-start gap-2">
              <div class="min-w-0 flex-1">
                <div class="mb-1 flex flex-wrap items-center gap-2">
                  <Tag
                    :value="candidateLabel(candidate)"
                    :severity="candidate.kind === 'task' ? 'info' : 'secondary'"
                  />
                  <Tag v-if="candidate.inferred" value="模型推断" severity="warn" />
                  <Tag v-if="candidate.status === 'confirmed'" value="已确认" severity="success" />
                  <Tag v-else-if="candidate.status === 'dismissed'" value="已忽略" severity="secondary" />
                </div>
                <strong class="break-words text-sm">{{ candidate.title }}</strong>
                <p v-if="candidate.detail" class="mt-1 whitespace-pre-wrap break-words text-sm text-muted-color">
                  {{ candidate.detail }}
                </p>
              </div>
              <Bookmark v-if="candidate.kind === 'decision'" :size="18" class="shrink-0 text-muted-color" />
              <Check v-else :size="18" class="shrink-0 text-muted-color" />
            </div>
            <div v-if="candidate.sources.length" class="mt-3 space-y-1 border-l-2 border-primary pl-3">
              <button
                v-for="source in candidate.sources"
                :key="source.message_id"
                type="button"
                class="block w-full rounded-sm py-1 text-left text-xs outline-none hover:text-primary focus-visible:ring-2 focus-visible:ring-primary"
                @click="locate(source.message_id)"
              >
                <span class="flex items-center gap-1 font-medium"><LocateFixed :size="13" />{{ source.sender }}</span>
                <span class="mt-0.5 line-clamp-2 break-words text-muted-color">{{ source.excerpt }}</span>
              </button>
            </div>
            <div v-if="candidate.status === 'proposed'" class="mt-3 flex justify-end gap-2">
              <Button
                text
                severity="secondary"
                size="small"
                :disabled="savingId === candidate.id"
                @click="update(candidate, 'dismiss')"
              >
                <X :size="15" /><span>忽略</span>
              </Button>
              <Button size="small" :loading="savingId === candidate.id" @click="update(candidate, 'confirm')">
                <Check :size="15" /><span>{{ candidate.kind === 'task' ? '创建待办' : '存入收藏' }}</span>
              </Button>
            </div>
          </li>
        </ol>
      </template>
    </div>
  </Drawer>
</template>
