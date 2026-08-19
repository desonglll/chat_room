<script setup lang="ts">
import { computed, ref } from 'vue'
import { FileUp, RotateCcw, Trash2 } from 'lucide-vue-next'
import Button from 'primevue/button'
import ProgressBar from 'primevue/progressbar'
import type { ChunkedUploadProgress } from '../composables/useChunkedUpload'
import type { AttachmentUploadSession } from '../types'

const props = defineProps<{
  progress: ChunkedUploadProgress | null
  pending: AttachmentUploadSession[]
  disabled: boolean
}>()

const emit = defineEmits<{
  resume: [session: AttachmentUploadSession, file: File]
  cancel: [session: AttachmentUploadSession]
}>()

const fileInput = ref<HTMLInputElement | null>(null)
const selectedSession = ref<AttachmentUploadSession | null>(null)
const visiblePending = computed(() => props.pending.filter((item) => item.id !== props.progress?.uploadId))
const visible = computed(() => Boolean(props.progress || visiblePending.value.length))
const activePercent = computed(() => props.progress
  ? Math.round((props.progress.processedBytes / props.progress.totalBytes) * 100)
  : 0)
const activeLabel = computed(() => {
  if (!props.progress) return ''
  if (props.progress.phase === 'hashing') return `正在计算 SHA-256 · ${activePercent.value}%`
  if (props.progress.phase === 'deduplicating') return '已找到相同文件，正在复用'
  if (props.progress.phase === 'finalizing') return '文件已上传，正在创建消息'
  return `服务端已确认 ${activePercent.value}%`
})

function chooseFile(session: AttachmentUploadSession): void {
  selectedSession.value = session
  if (fileInput.value) {
    fileInput.value.value = ''
    fileInput.value.click()
  }
}

function handleFile(event: Event): void {
  const file = (event.target as HTMLInputElement).files?.[0]
  if (file && selectedSession.value) emit('resume', selectedSession.value, file)
  selectedSession.value = null
}

function sessionPercent(session: AttachmentUploadSession): number {
  return Math.round((session.received_bytes / session.declared_size_bytes) * 100)
}
</script>

<template>
  <div v-if="visible" class="shrink-0 border-t border-surface-200 bg-surface-0 px-3 py-2 sm:px-7">
    <input ref="fileInput" class="hidden" type="file" @change="handleFile">
    <div v-if="progress" class="flex min-h-8 items-center gap-2 text-xs text-muted-color">
      <FileUp :size="16" class="shrink-0 text-primary" />
      <div class="min-w-0 flex-1">
        <div class="mb-1 flex min-w-0 justify-between gap-2">
          <span class="truncate">{{ progress.fileName }}</span>
          <span class="shrink-0">{{ activeLabel }}</span>
        </div>
        <ProgressBar :value="activePercent" :show-value="false" class="h-1.5" />
      </div>
    </div>
    <div
      v-for="session in visiblePending"
      :key="session.id"
      class="flex min-h-10 items-center gap-2 border-t border-surface-100 py-1.5 first:border-t-0"
    >
      <FileUp :size="16" class="shrink-0 text-muted-color" />
      <div class="min-w-0 flex-1">
        <div class="flex justify-between gap-2 text-xs">
          <span class="truncate">{{ session.file_name }}</span>
          <span class="shrink-0 text-muted-color">等待继续 · {{ sessionPercent(session) }}%</span>
        </div>
        <ProgressBar :value="sessionPercent(session)" :show-value="false" class="mt-1 h-1" />
      </div>
      <Button
        text rounded size="small" severity="secondary" :disabled="disabled"
        aria-label="选择原文件继续上传" title="选择原文件继续上传" @click="chooseFile(session)"
      ><RotateCcw :size="16" /></Button>
      <Button
        text rounded size="small" severity="danger" :disabled="disabled"
        aria-label="取消上传" title="取消上传" @click="emit('cancel', session)"
      ><Trash2 :size="16" /></Button>
    </div>
  </div>
</template>
