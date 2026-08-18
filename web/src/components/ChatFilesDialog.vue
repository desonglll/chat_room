<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Download, File, FileVideo, ShieldAlert } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import Message from 'primevue/message'
import ProgressBar from 'primevue/progressbar'
import SelectButton from 'primevue/selectbutton'
import { listRoomFiles } from '../api'
import type { Attachment, ChatFileItem } from '../types'
import type { DownloadProgress } from '../attachmentDownloads'
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
  roomId: string
  token: string
  password: string
  downloading: boolean
  downloadProgress: DownloadProgress | null
}>()
const emit = defineEmits<{
  close: []
  download: [attachments: Attachment[]]
  cancelDownload: []
}>()

const filter = ref('all')
const selected = ref<string[]>([])
const files = ref<ChatFileItem[]>([])
const nextBefore = ref<string | null>(null)
const loading = ref(false)
const error = ref('')
const previewing = ref<Attachment | null>(null)
const previewImageId = ref('')
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => { if (!value) emit('close') },
})
const filtered = computed(() => files.value)
const selectedAttachments = computed(() => files.value
  .filter((file) => selected.value.includes(file.message_id))
  .map((file) => file.attachment))
let requestVersion = 0

watch(() => props.open, (open) => {
  previewing.value = null
  previewImageId.value = ''
  if (!open) {
    requestVersion += 1
    loading.value = false
    return
  }
  selected.value = []
  if (filter.value === 'all') void loadFiles(true)
  else filter.value = 'all'
})
watch(filter, () => { if (props.open) void loadFiles(true) })

async function loadFiles(reset = false): Promise<void> {
  if ((loading.value && !reset) || !props.roomId || !props.token) return
  const version = ++requestVersion
  loading.value = true
  error.value = ''
  try {
    const page = await listRoomFiles(
      props.roomId,
      props.token,
      props.password,
      filter.value as 'all' | 'image' | 'video' | 'file',
      reset ? '' : nextBefore.value || '',
    )
    if (version !== requestVersion) return
    files.value = reset ? page.items : [...files.value, ...page.items]
    nextBefore.value = page.next_before
    if (reset) selected.value = []
  } catch (caught) {
    if (version === requestVersion) error.value = caught instanceof Error ? caught.message : '读取聊天文件失败'
  } finally {
    if (version === requestVersion) loading.value = false
  }
}

function toggleAll(): void {
  const ids = filtered.value.map((file) => file.message_id)
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
  if (attachment.is_sensitive) return
  if (attachment.mime_type.startsWith('image/')) previewImageId.value = attachment.id
  else previewing.value = attachment
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="聊天文件" class="w-[min(96vw,720px)]" :draggable="false">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-surface-200 pb-4">
      <SelectButton v-model="filter" :options="FILTERS" option-label="label" option-value="value" :allow-empty="false" />
      <Button size="small" severity="secondary" outlined :disabled="!filtered.length" @click="toggleAll">
        {{ filtered.length && filtered.every((file) => selected.includes(file.message_id)) ? '取消全选' : '全选当前' }}
      </Button>
    </div>

    <div v-if="filtered.length" class="max-h-[58vh] overflow-y-auto py-2">
      <div v-for="file in filtered" :key="file.message_id" class="flex min-h-16 items-center gap-3 border-b border-surface-100 px-1 py-2 transition hover:bg-surface-50">
        <Checkbox v-model="selected" :value="file.message_id" />
        <button type="button" class="flex min-w-0 flex-1 items-center gap-3 text-left" @click="preview(file.attachment)">
          <span class="relative grid size-10 shrink-0 place-items-center overflow-hidden rounded-md bg-surface-100 text-muted-color">
            <img
              v-if="file.attachment.mime_type.startsWith('image/')"
              :src="file.attachment.download_url"
              :alt="file.attachment.file_name"
              class="size-full object-cover"
              :class="{ 'blur-md': file.attachment.is_sensitive }"
            >
            <FileVideo v-else-if="file.attachment.mime_type.startsWith('video/')" :size="20" />
            <File v-else :size="20" />
            <ShieldAlert v-if="file.attachment.is_sensitive" :size="14" class="absolute inset-0 m-auto text-white drop-shadow" />
          </span>
          <span class="min-w-0 flex-1">
            <strong class="block truncate text-sm">{{ file.attachment.file_name }}</strong>
            <small class="mt-1 block truncate text-xs text-muted-color">{{ file.sender }} · {{ formatSize(file.attachment.size_bytes) }}</small>
          </span>
        </button>
      </div>
    </div>
    <Message v-if="error" severity="error" class="my-4">{{ error }}</Message>
    <div v-else-if="!filtered.length && !loading" class="grid min-h-48 place-items-center text-sm text-muted-color">暂无聊天文件</div>
    <div v-if="loading" class="grid min-h-20 place-items-center text-sm text-muted-color">正在加载...</div>
    <div v-if="nextBefore && !loading" class="flex justify-center py-3">
      <Button size="small" severity="secondary" outlined @click="loadFiles()">加载更多</Button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-3 border-t border-surface-200 pt-4">
      <div class="min-w-52 flex-1">
        <span class="text-xs text-muted-color">
          <template v-if="downloadProgress">
            {{ downloadProgress.stage === 'packing' ? '正在打包' : '正在下载' }} ·
            第 {{ downloadProgress.batchIndex }}/{{ downloadProgress.batchCount }} 批 ·
            {{ downloadProgress.completedFiles }}/{{ downloadProgress.totalFiles }} 个
          </template>
          <template v-else>已选择 {{ selectedAttachments.length }} 个文件</template>
        </span>
        <ProgressBar v-if="downloadProgress" :value="downloadProgress.percent" :show-value="false" class="mt-2 h-1.5" />
      </div>
      <Button v-if="downloading" severity="danger" outlined @click="emit('cancelDownload')">取消</Button>
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
