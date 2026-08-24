<script setup lang="ts">
import { Bookmark, Download, Forward, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import ProgressBar from 'primevue/progressbar'
import type { DownloadProgress } from '../attachmentDownloads'

defineProps<{
  selectedCount: number
  attachmentCount: number
  downloading: boolean
  downloadProgress: DownloadProgress | null
}>()
const emit = defineEmits<{
  close: []
  forward: []
  favorite: []
  download: []
  cancelDownload: []
}>()
</script>

<template>
  <div class="cr-selection-bar flex min-h-16 shrink-0 items-center gap-1.5 px-2 sm:gap-3 sm:px-5">
    <Button text rounded severity="secondary" aria-label="退出多选" title="退出多选" @click="emit('close')">
      <X :size="19" />
    </Button>
    <div class="min-w-0 flex-1">
      <span class="truncate text-sm text-muted-color">已选 {{ selectedCount }} 条</span>
      <div v-if="downloadProgress" class="mt-2 flex items-center gap-2">
        <ProgressBar :value="downloadProgress.percent" :show-value="false" class="h-1.5 min-w-16 flex-1" />
        <span class="shrink-0 text-xs text-muted-color">
          {{ downloadProgress.completedFiles }}/{{ downloadProgress.totalFiles }}
        </span>
        <Button size="small" text severity="danger" aria-label="取消下载" @click="emit('cancelDownload')">取消</Button>
      </div>
    </div>
    <Button
      :disabled="!selectedCount"
      severity="secondary"
      outlined
      class="size-10! p-0! sm:w-auto! sm:px-3!"
      aria-label="转发所选消息"
      title="转发"
      @click="emit('forward')"
    >
      <Forward :size="17" /><span class="hidden sm:inline">转发</span>
    </Button>
    <Button
      :disabled="!selectedCount"
      severity="secondary"
      outlined
      class="size-10! p-0! sm:w-auto! sm:px-3!"
      aria-label="收藏所选消息"
      title="收藏"
      @click="emit('favorite')"
    >
      <Bookmark :size="17" /><span class="hidden sm:inline">收藏</span>
    </Button>
    <Button
      :disabled="!attachmentCount"
      :loading="downloading"
      class="size-10! p-0! sm:w-auto! sm:px-3!"
      aria-label="保存所选附件"
      title="保存附件"
      @click="emit('download')"
    >
      <Download :size="17" /><span class="hidden sm:inline">保存</span>
    </Button>
  </div>
</template>
