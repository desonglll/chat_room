<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Download, File, FileVideo } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import SelectButton from 'primevue/selectbutton'
import type { Attachment, DisplayMessage } from '../types'
import AttachmentPreviewDialog from './AttachmentPreviewDialog.vue'
import ImageViewerGallery from './ImageViewerGallery.vue'

const FILTERS = [
  { label: '全部', value: 'all' },
  { label: '图片', value: 'image' },
  { label: '视频', value: 'video' },
  { label: '文件', value: 'file' },
]

const props = defineProps<{
  open: boolean
  messages: DisplayMessage[]
  downloading: boolean
}>()
const emit = defineEmits<{
  close: []
  download: [attachments: Attachment[]]
}>()

const filter = ref('all')
const selected = ref<string[]>([])
const previewing = ref<Attachment | null>(null)
const previewImageId = ref('')
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => { if (!value) emit('close') },
})
const files = computed(() => props.messages.flatMap((message) =>
  message.type === 'broadcast' && message.attachment && !message.recalled_at
    ? [{ messageId: message.message_id, sender: message.sender, timestamp: message.timestamp, attachment: message.attachment }]
    : [],
))
const filtered = computed(() => files.value.filter(({ attachment }) => {
  if (filter.value === 'image') return attachment.mime_type.startsWith('image/')
  if (filter.value === 'video') return attachment.mime_type.startsWith('video/')
  if (filter.value === 'file') return !attachment.mime_type.startsWith('image/') && !attachment.mime_type.startsWith('video/')
  return true
}))
const selectedAttachments = computed(() => files.value
  .filter((file) => selected.value.includes(file.messageId))
  .map((file) => file.attachment))

watch(() => props.open, (open) => {
  previewing.value = null
  previewImageId.value = ''
  if (!open) return
  filter.value = 'all'
  selected.value = []
})

function toggleAll(): void {
  const ids = filtered.value.map((file) => file.messageId)
  const allSelected = ids.length > 0 && ids.every((id) => selected.value.includes(id))
  selected.value = allSelected
    ? selected.value.filter((id) => !ids.includes(id))
    : [...new Set([...selected.value, ...ids])]
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(0.1, bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`
}

function preview(attachment: Attachment): void {
  if (attachment.mime_type.startsWith('image/')) previewImageId.value = attachment.id
  else previewing.value = attachment
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="聊天文件" class="w-[min(96vw,720px)]" :draggable="false">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-surface-200 pb-4">
      <SelectButton v-model="filter" :options="FILTERS" option-label="label" option-value="value" :allow-empty="false" />
      <Button size="small" severity="secondary" outlined :disabled="!filtered.length" @click="toggleAll">
        {{ filtered.length && filtered.every((file) => selected.includes(file.messageId)) ? '取消全选' : '全选当前' }}
      </Button>
    </div>

    <div v-if="filtered.length" class="max-h-[58vh] overflow-y-auto py-2">
      <div v-for="file in filtered" :key="file.messageId" class="flex min-h-16 items-center gap-3 border-b border-surface-100 px-1 py-2 transition hover:bg-surface-50">
        <Checkbox v-model="selected" :value="file.messageId" />
        <button type="button" class="flex min-w-0 flex-1 items-center gap-3 text-left" @click="preview(file.attachment)">
          <span class="grid size-10 shrink-0 place-items-center overflow-hidden rounded-md bg-surface-100 text-muted-color">
            <img v-if="file.attachment.mime_type.startsWith('image/')" :src="file.attachment.download_url" :alt="file.attachment.file_name" class="size-full object-cover">
            <FileVideo v-else-if="file.attachment.mime_type.startsWith('video/')" :size="20" />
            <File v-else :size="20" />
          </span>
          <span class="min-w-0 flex-1">
            <strong class="block truncate text-sm">{{ file.attachment.file_name }}</strong>
            <small class="mt-1 block truncate text-xs text-muted-color">{{ file.sender }} · {{ formatSize(file.attachment.size_bytes) }}</small>
          </span>
        </button>
      </div>
    </div>
    <div v-else class="grid min-h-48 place-items-center text-sm text-muted-color">暂无聊天文件</div>

    <div class="flex items-center justify-between gap-3 border-t border-surface-200 pt-4">
      <span class="text-xs text-muted-color">已选择 {{ selectedAttachments.length }} 个文件</span>
      <Button :disabled="!selectedAttachments.length" :loading="downloading" @click="emit('download', selectedAttachments)">
        <Download :size="17" />
        <span>批量保存</span>
      </Button>
    </div>
  </Dialog>
  <AttachmentPreviewDialog :attachment="previewing" @close="previewing = null" />
  <ImageViewerGallery
    :images="files.filter((file) => file.attachment.mime_type.startsWith('image/')).map((file) => file.attachment)"
    :active-id="previewImageId"
    @close="previewImageId = ''"
  />
</template>
