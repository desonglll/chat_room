<script setup lang="ts">
import { File as FileIcon, X } from 'lucide-vue-next'
import Checkbox from 'primevue/checkbox'

interface PendingFilePreview {
  id: number
  file: File
  previewUrl: string
  previewKind: 'image' | 'video' | 'file'
}

defineProps<{ files: PendingFilePreview[] }>()
const sensitive = defineModel<boolean>('sensitive', { required: true })
const emit = defineEmits<{ remove: [id: number] }>()
</script>

<template>
  <TransitionGroup
    v-if="files.length"
    tag="div"
    class="cr-composer-width flex gap-2 overflow-x-auto px-3 pb-2 sm:px-1"
    aria-label="待发送附件"
    enter-active-class="transition-[opacity,transform] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
    enter-from-class="translate-y-1 opacity-0"
    leave-active-class="transition-[opacity,transform] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
    leave-to-class="scale-95 opacity-0"
  >
    <div
      v-for="item in files"
      :key="item.id"
      class="relative grid size-[72px] shrink-0 place-items-center overflow-hidden rounded-xl bg-surface-100 text-muted-color shadow-sm"
    >
      <img
        v-if="item.previewKind === 'image'"
        class="size-full object-cover"
        :src="item.previewUrl"
        :alt="item.file.name"
        width="72"
        height="72"
      />
      <video
        v-else-if="item.previewKind === 'video'"
        class="size-full object-cover"
        :src="item.previewUrl"
        muted
        playsinline
        preload="metadata"
      />
      <FileIcon v-else :size="24" />
      <span class="absolute inset-x-1 bottom-1 truncate rounded-sm bg-surface-900/75 px-1 py-0.5 text-[9px] text-white">
        {{ item.file.name }}
      </span>
      <button
        type="button"
        class="absolute right-1 top-1 grid size-6 place-items-center rounded bg-surface-0/90 text-surface-600 shadow-sm hover:bg-surface-0 hover:text-surface-900"
        aria-label="移除附件"
        title="移除附件"
        @click="emit('remove', item.id)"
      >
        <X :size="15" />
      </button>
    </div>
  </TransitionGroup>

  <label
    v-if="files.length"
    class="cr-composer-width flex items-center gap-2 px-3 pb-2 text-xs text-muted-color sm:px-1"
  >
    <Checkbox v-model="sensitive" binary input-id="sensitiveContent" />
    <span>包含敏感内容，接收方需点击确认才能查看</span>
  </label>
</template>
