<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { FileUp, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Textarea from 'primevue/textarea'
import type { FavoriteItem } from '../types'

const props = defineProps<{
  visible: boolean
  maxUploadBytes: number
  create: (title: string, content: string) => Promise<FavoriteItem>
  createAttachment: (file: File, title: string, content: string, maxUploadBytes: number) => Promise<FavoriteItem>
}>()
const emit = defineEmits<{
  'update:visible': [visible: boolean]
  success: [message: string]
  error: [message: string]
}>()
const title = ref('')
const content = ref('')
const file = ref<File | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)
const busy = ref(false)
const canSubmit = computed(() => Boolean(file.value || title.value.trim() || content.value.trim()))

function reset(): void {
  title.value = ''
  content.value = ''
  file.value = null
  if (fileInput.value) fileInput.value.value = ''
}

function selectFile(event: Event): void {
  const selected = (event.target as HTMLInputElement).files?.[0] || null
  if (selected && selected.size > props.maxUploadBytes) {
    emit('error', `文件不能超过 ${Math.ceil(props.maxUploadBytes / 1024 / 1024)} MiB`)
    if (fileInput.value) fileInput.value.value = ''
    return
  }
  file.value = selected
}

async function submit(): Promise<void> {
  if (!canSubmit.value) return
  busy.value = true
  try {
    if (file.value) await props.createAttachment(file.value, title.value, content.value, props.maxUploadBytes)
    else await props.create(title.value, content.value)
    emit('update:visible', false)
    emit('success', file.value ? '文件已添加到收藏' : '收藏已创建')
    reset()
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '创建收藏失败')
  } finally {
    busy.value = false
  }
}

watch(
  () => props.visible,
  (visible) => {
    if (!visible && !busy.value) reset()
  },
)
</script>

<template>
  <Dialog
    :visible="visible"
    modal
    header="新建收藏"
    class="w-[min(92vw,520px)]"
    :draggable="false"
    @update:visible="emit('update:visible', $event)"
  >
    <form class="space-y-4" @submit.prevent="submit">
      <div>
        <label for="favorite-title" class="mb-2 block text-sm font-medium">标题</label>
        <InputText id="favorite-title" v-model="title" maxlength="120" fluid />
      </div>
      <div>
        <label for="favorite-content" class="mb-2 block text-sm font-medium">内容</label>
        <Textarea id="favorite-content" v-model="content" maxlength="8000" rows="7" auto-resize fluid />
      </div>
      <div>
        <span class="mb-2 block text-sm font-medium">附件</span>
        <input ref="fileInput" type="file" class="sr-only" @change="selectFile" />
        <div v-if="file" class="flex min-h-11 items-center gap-2 rounded-md border border-surface-200 px-3">
          <FileUp :size="17" class="shrink-0 text-primary" />
          <span class="min-w-0 flex-1 truncate text-sm">{{ file.name }}</span>
          <Button
            type="button"
            text
            rounded
            severity="secondary"
            aria-label="移除附件"
            title="移除附件"
            @click="file = null"
          >
            <X :size="16" />
          </Button>
        </div>
        <Button v-else type="button" severity="secondary" outlined fluid @click="fileInput?.click()">
          <FileUp :size="17" /><span>添加文件或图片</span>
        </Button>
      </div>
      <div class="flex justify-end gap-2">
        <Button type="button" label="取消" severity="secondary" text @click="emit('update:visible', false)" />
        <Button type="submit" label="创建" :loading="busy" :disabled="!canSubmit" />
      </div>
    </form>
  </Dialog>
</template>
