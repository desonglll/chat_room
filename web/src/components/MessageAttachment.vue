<script setup lang="ts">
import { computed, ref } from 'vue'
import { Download, File, FileImage, FileVideo } from 'lucide-vue-next'
import type { Attachment } from '../types'

const props = defineProps<{ attachment: Attachment }>()
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
  <div class="message-attachment">
    <a
      v-if="kind === 'image' && !previewFailed"
      class="media-preview"
      :href="attachment.download_url"
      target="_blank"
      rel="noopener"
      :title="attachment.file_name"
    >
      <img :src="attachment.download_url" :alt="attachment.file_name" loading="lazy" @error="previewFailed = true">
    </a>
    <video
      v-else-if="kind === 'video' && !previewFailed"
      class="media-preview"
      :src="attachment.download_url"
      controls
      preload="metadata"
      @error="previewFailed = true"
    />
    <a class="attachment-file" :href="attachment.download_url" :download="attachment.file_name">
      <FileImage v-if="kind === 'image'" :size="20" />
      <FileVideo v-else-if="kind === 'video'" :size="20" />
      <File v-else :size="20" />
      <span>
        <strong>{{ attachment.file_name }}</strong>
        <small>{{ formatSize(attachment.size_bytes) }}</small>
      </span>
      <Download :size="18" />
    </a>
  </div>
</template>

<style scoped>
.message-attachment {
  width: min(440px, 72vw);
  overflow: hidden;
  border: 1px solid #d4d4d8;
  border-radius: 6px;
  background: #fff;
}

.media-preview {
  display: block;
  width: 100%;
  max-height: min(52vh, 420px);
  background: #18181b;
  object-fit: contain;
}

a.media-preview {
  line-height: 0;
}

.media-preview img {
  display: block;
  width: 100%;
  max-height: min(52vh, 420px);
  object-fit: contain;
}

.attachment-file {
  display: flex;
  min-height: 54px;
  align-items: center;
  gap: 10px;
  padding: 8px 11px;
  color: #3f3f46;
  text-decoration: none;
}

.attachment-file:hover {
  background: #f4f4f5;
}

.attachment-file > svg {
  flex: 0 0 auto;
  color: #0f766e;
}

.attachment-file > svg:last-child {
  margin-left: auto;
  color: #71717a;
}

.attachment-file span {
  min-width: 0;
  flex: 1 1 auto;
}

.attachment-file strong,
.attachment-file small {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-file strong {
  font-size: 13px;
}

.attachment-file small {
  margin-top: 2px;
  color: #71717a;
  font-size: 11px;
}

@media (max-width: 767px) {
  .message-attachment {
    width: min(78vw, 440px);
  }
}
</style>
