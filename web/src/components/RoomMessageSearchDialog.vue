<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { LoaderCircle, LocateFixed, Search } from 'lucide-vue-next'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import { searchRoomMessages } from '../api'
import type { StoredMessage } from '../types'

const props = defineProps<{
  open: boolean
  roomId: string
  token: string
  password: string
}>()
const emit = defineEmits<{ close: []; locate: [messageId: string] }>()

const query = ref('')
const results = ref<StoredMessage[]>([])
const loading = ref(false)
const error = ref('')
let timer: ReturnType<typeof setTimeout> | null = null
let version = 0

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer)
  version += 1
})

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      query.value = ''
      results.value = []
      error.value = ''
      return
    }
    await nextTick()
    document.querySelector<HTMLInputElement>('[data-room-message-search]')?.focus()
  },
)

watch(query, (value) => {
  if (timer) clearTimeout(timer)
  const text = value.trim()
  version += 1
  const requestVersion = version
  error.value = ''
  if (!text) {
    results.value = []
    loading.value = false
    return
  }
  loading.value = true
  timer = setTimeout(async () => {
    try {
      const found = await searchRoomMessages(props.roomId, text, props.token, props.password, '', 50)
      if (requestVersion === version) results.value = found
    } catch (caught) {
      if (requestVersion === version) {
        results.value = []
        error.value = caught instanceof Error ? caught.message : '搜索失败'
      }
    } finally {
      if (requestVersion === version) loading.value = false
    }
  }, 250)
})

function formatTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'short', timeStyle: 'short' }).format(date)
}
</script>

<template>
  <Dialog
    :visible="open"
    modal
    header="搜索聊天记录"
    class="w-[min(94vw,640px)]"
    :draggable="false"
    @update:visible="!$event && emit('close')"
  >
    <div class="relative">
      <Search :size="17" class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-color" />
      <InputText
        v-model="query"
        data-room-message-search
        class="w-full pl-10! pr-10!"
        maxlength="200"
        placeholder="输入消息内容"
        autocomplete="off"
      />
      <LoaderCircle
        v-if="loading"
        :size="17"
        class="absolute right-3 top-1/2 -translate-y-1/2 animate-spin text-primary motion-reduce:animate-none"
      />
    </div>

    <p v-if="error" class="mt-3 text-sm text-red-600">{{ error }}</p>
    <p v-else-if="query.trim() && !loading && !results.length" class="py-10 text-center text-sm text-muted-color">
      没有找到匹配消息
    </p>
    <ol v-else-if="results.length" class="mt-3 max-h-[min(60vh,520px)] divide-y divide-surface-200 overflow-y-auto">
      <li v-for="message in results" :key="message.id">
        <button
          type="button"
          class="group flex min-h-16 w-full items-center gap-3 px-2 py-3 text-left hover:bg-surface-50 focus-visible:outline-2 focus-visible:outline-primary"
          @click="emit('locate', message.id)"
        >
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-x-2 text-xs">
              <strong class="text-surface-800">{{ message.sender }}</strong>
              <time class="text-muted-color">{{ formatTime(message.created_at) }}</time>
            </div>
            <p class="mt-1 line-clamp-2 break-words text-sm leading-5 text-surface-700">{{ message.content }}</p>
          </div>
          <LocateFixed :size="16" class="shrink-0 text-muted-color group-hover:text-primary" />
        </button>
      </li>
    </ol>
  </Dialog>
</template>
