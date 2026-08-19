<script setup lang="ts">
import { ref } from 'vue'
import { FileUp, RotateCcw, Trash2 } from 'lucide-vue-next'
import Button from 'primevue/button'
import ProgressBar from 'primevue/progressbar'
import type { AttachmentUploadSession } from '../types'

const props = defineProps<{
  pending: AttachmentUploadSession[]
}>()

const emit = defineEmits<{
  resume: [session: AttachmentUploadSession, file: File]
  cancel: [session: AttachmentUploadSession]
}>()

const fileInput = ref<HTMLInputElement | null>(null)
const selectedSession = ref<AttachmentUploadSession | null>(null)
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
  <div v-if="pending.length" class="shrink-0 border-t border-surface-200 bg-surface-0 px-3 py-2 sm:px-7">
    <input ref="fileInput" class="hidden" type="file" @change="handleFile" />
    <div
      v-for="session in pending"
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
        text
        rounded
        size="small"
        severity="secondary"
        aria-label="选择原文件继续上传"
        title="选择原文件继续上传"
        @click="chooseFile(session)"
        ><RotateCcw :size="16"
      /></Button>
      <Button
        text
        rounded
        size="small"
        severity="danger"
        aria-label="取消上传"
        title="取消上传"
        @click="emit('cancel', session)"
        ><Trash2 :size="16"
      /></Button>
    </div>
  </div>
</template>
