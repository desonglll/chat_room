<script setup lang="ts">
import { computed, ref } from 'vue'
import { Download, File, FileImage, FileVideo } from 'lucide-vue-next'
import type { Attachment } from '../types'
import VideoPlayer from './VideoPlayer.vue'

const props = defineProps<{ attachment: Attachment }>()
const emit = defineEmits<{ previewImage: [attachment: Attachment] }>()
const previewFailed = ref(false)

const imageTypes = new Set(['image/avif', 'image/gif', 'image/jpeg', 'image/png', 'image/webp'])
const videoTypes = new Set(['video/mp4', 'video/ogg', 'video/quicktime', 'video/webm'])
const kind = computed(() => {
  if (imageTypes.has(props.attachment.mime_type)) return 'image'
  if (videoTypes.has(props.attachment.mime_type)) return 'video'
  return 'file'
})

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`
}
</script>

<template>
  <div class="w-[min(78vw,440px)] overflow-hidden rounded-lg border border-surface-200 bg-surface-0 sm:w-[min(72vw,440px)]">
    <button
      v-if="kind === 'image' && !previewFailed"
      type="button"
      class="block w-full bg-surface-900 leading-none"
      :title="attachment.file_name"
      @click="emit('previewImage', attachment)"
    >
      <img class="block max-h-[min(52vh,420px)] w-full object-contain" :src="attachment.download_url" :alt="attachment.file_name" loading="lazy" @error="previewFailed = true">
    </button>
    <VideoPlayer
      v-else-if="kind === 'video' && !previewFailed"
      :src="attachment.download_url"
      :mime-type="attachment.mime_type"
      @error="previewFailed = true"
    />
    <a class="flex min-h-14 items-center gap-2.5 px-3 py-2 text-surface-700 no-underline hover:bg-surface-50" :href="attachment.download_url" :download="attachment.file_name">
      <FileImage v-if="kind === 'image'" :size="20" />
      <FileVideo v-else-if="kind === 'video'" :size="20" />
      <File v-else :size="20" />
      <span class="min-w-0 flex-1">
        <strong class="block truncate text-[13px]">{{ attachment.file_name }}</strong>
        <small class="mt-0.5 block text-[11px] text-muted-color">{{ formatSize(attachment.size_bytes) }}</small>
      </span>
      <Download class="ml-auto shrink-0 text-muted-color" :size="18" />
    </a>
  </div>
</template>
