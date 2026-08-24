<script setup lang="ts">
import { ref } from 'vue'
import { Eye, File, FileVideo, RotateCcw, ShieldAlert, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import ProgressBar from 'primevue/progressbar'
import { uploadPercent } from '../attachmentUploadProgress'
import type { UploadMessage } from '../types'

const props = defineProps<{ message: UploadMessage }>()
const emit = defineEmits<{ cancel: [key: string]; retry: [key: string] }>()
const revealed = ref(false)

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value))
}

function statusLabel(message: UploadMessage): string {
  if (message.status === 'failed') return '上传失败'
  if (message.phase === 'queued') return '等待上传'
  if (message.phase === 'hashing') return '正在计算文件哈希'
  if (message.phase === 'deduplicating') return '已找到相同文件，正在复用'
  if (message.phase === 'finalizing') return '服务端正在确认文件'
  return '正在上传'
}
</script>

<template>
  <div class="motion-outgoing mb-4 flex flex-row-reverse items-start gap-2" :data-upload-id="message.key">
    <div class="w-[min(86%,440px)]">
      <div class="mb-1 flex items-center justify-end gap-2 text-xs text-muted-color">
        <strong>你</strong>
        <time>{{ formatTime(message.timestamp) }}</time>
      </div>
      <div class="overflow-hidden rounded-2xl rounded-br-md border border-surface-200 bg-surface-0 shadow-sm">
        <div
          v-if="message.preview_url || (message.is_sensitive && !revealed)"
          class="relative overflow-hidden"
          :class="{ 'min-h-28': message.is_sensitive && !revealed }"
        >
          <img
            v-if="message.mime_type.startsWith('image/')"
            :src="message.preview_url"
            :alt="message.file_name"
            class="max-h-64 w-full object-contain"
            :class="{ 'scale-110 blur-xl': message.is_sensitive && !revealed }"
          />
          <video
            v-else-if="message.mime_type.startsWith('video/')"
            :src="message.preview_url"
            class="max-h-64 w-full bg-black object-contain"
            :class="{ 'scale-110 blur-xl': message.is_sensitive && !revealed }"
            muted
            playsinline
            preload="metadata"
          />
          <div
            v-if="message.is_sensitive && !revealed"
            class="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-surface-900/55 text-white"
          >
            <ShieldAlert :size="26" />
            <strong class="text-sm">已标记为敏感内容</strong>
            <button
              type="button"
              class="mt-1 flex items-center gap-1.5 rounded-full bg-white/95 px-3 py-1.5 text-xs font-semibold text-surface-900 hover:bg-white"
              @click="revealed = true"
            >
              <Eye :size="14" />
              <span>确认查看</span>
            </button>
          </div>
        </div>
        <div class="flex items-center gap-3 px-3 py-2.5">
          <span class="grid size-10 shrink-0 place-items-center rounded-full bg-primary-50 text-primary">
            <FileVideo v-if="message.mime_type.startsWith('video/')" :size="20" />
            <File v-else :size="20" />
          </span>
          <div class="min-w-0 flex-1">
            <strong class="block truncate text-sm">{{ message.file_name }}</strong>
            <span class="text-xs text-muted-color">{{ formatSize(message.size_bytes) }}</span>
          </div>
          <Button
            v-if="message.status === 'failed'"
            text
            rounded
            size="small"
            aria-label="重试上传"
            title="重试上传"
            @click="emit('retry', message.key)"
          >
            <RotateCcw :size="17" />
          </Button>
          <Button
            text
            rounded
            size="small"
            severity="danger"
            aria-label="取消上传"
            title="取消上传"
            @click="emit('cancel', message.key)"
          >
            <X :size="17" />
          </Button>
        </div>
        <div class="px-3 pb-3">
          <div class="mb-1 flex justify-between gap-3 text-xs">
            <span :class="message.status === 'failed' ? 'text-danger' : 'text-muted-color'">
              {{ statusLabel(message) }}
            </span>
            <span class="shrink-0 font-medium"
              >{{ uploadPercent(message.phase, message.processed_bytes, message.total_bytes) }}%</span
            >
          </div>
          <ProgressBar
            :value="uploadPercent(message.phase, message.processed_bytes, message.total_bytes)"
            :show-value="false"
            class="h-1.5"
          />
          <small v-if="message.error" class="mt-1 block truncate text-danger" :title="message.error">{{
            message.error
          }}</small>
          <p v-if="message.content" class="mt-2 whitespace-pre-wrap break-words text-sm leading-5">
            {{ message.content }}
          </p>
        </div>
      </div>
    </div>
  </div>
</template>
