<script setup lang="ts">
import { computed } from 'vue'
import { Download, File } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import type { Attachment } from '../types'
import VideoPlayer from './VideoPlayer.vue'

const props = defineProps<{ attachment: Attachment | null }>()
const emit = defineEmits<{ close: [] }>()
const visible = computed({
  get: () => Boolean(props.attachment),
  set: (value: boolean) => { if (!value) emit('close') },
})
const kind = computed(() => {
  const mime = props.attachment?.mime_type || ''
  if (mime.startsWith('image/')) return 'image'
  if (mime.startsWith('video/')) return 'video'
  if (mime.startsWith('audio/')) return 'audio'
  if (mime === 'application/pdf' || mime === 'text/plain') return 'document'
  return 'file'
})
</script>

<template>
  <Dialog v-model:visible="visible" modal :header="attachment?.file_name" class="w-[min(96vw,900px)]" :draggable="false">
    <template v-if="attachment">
      <div class="grid min-h-64 place-items-center overflow-hidden bg-surface-950">
        <img v-if="kind === 'image'" :src="attachment.download_url" :alt="attachment.file_name" class="max-h-[70vh] max-w-full object-contain">
        <VideoPlayer v-else-if="kind === 'video'" class="w-full" :src="attachment.download_url" :mime-type="attachment.mime_type" />
        <audio v-else-if="kind === 'audio'" class="w-[min(90%,560px)]" :src="attachment.download_url" controls preload="metadata" />
        <iframe v-else-if="kind === 'document'" class="h-[70vh] w-full bg-white" :src="attachment.download_url" :title="attachment.file_name" />
        <div v-else class="px-6 text-center text-surface-300">
          <File class="mx-auto" :size="42" />
          <strong class="mt-3 block text-sm">当前格式无法在线预览</strong>
          <span class="mt-1 block text-xs text-surface-400">{{ attachment.mime_type }}</span>
        </div>
      </div>
      <div class="mt-4 flex justify-end border-t border-surface-200 pt-4">
        <Button as="a" :href="attachment.download_url" :download="attachment.file_name">
          <Download :size="17" />
          <span>保存文件</span>
        </Button>
      </div>
    </template>
  </Dialog>
</template>
