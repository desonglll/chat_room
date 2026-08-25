<script setup lang="ts">
import { MessageSquare, Plus, Trash2 } from 'lucide-vue-next'
import Button from 'primevue/button'
import type { AiThread } from '../types'

defineProps<{
  threads: AiThread[]
  activeId: string
  busy: boolean
}>()

const emit = defineEmits<{
  create: []
  select: [threadId: string]
  delete: [thread: AiThread]
}>()
</script>

<template>
  <aside
    class="grid min-h-0 grid-cols-[auto_minmax(0,1fr)] border-b border-surface-200 bg-surface-0 md:flex md:flex-col md:border-r md:border-b-0"
  >
    <div
      class="flex h-12 shrink-0 items-center justify-between border-r border-surface-200 px-2 md:h-14 md:border-r-0 md:border-b md:px-3"
    >
      <span class="hidden text-sm font-semibold sm:inline">AI 对话</span>
      <Button
        text
        rounded
        aria-label="新建 AI 对话"
        title="新建对话"
        class="size-8! p-0!"
        :disabled="busy"
        @click="emit('create')"
      >
        <Plus :size="17" />
      </Button>
    </div>
    <nav
      class="min-h-0 min-w-0 flex-1 overflow-x-auto overflow-y-hidden p-1 md:overflow-x-hidden md:overflow-y-auto md:p-2"
      aria-label="AI 对话列表"
    >
      <p v-if="!threads.length" class="px-2 py-3 text-center text-xs text-muted-color md:py-5">暂无对话</p>
      <ul v-else class="flex gap-1 md:block md:space-y-1">
        <li v-for="thread in threads" :key="thread.id" class="group relative w-40 shrink-0 md:w-auto">
          <button
            type="button"
            class="flex h-10 w-full items-center gap-2 rounded-md px-2.5 pr-9 text-left text-sm transition-colors"
            :class="thread.id === activeId ? 'bg-primary-50 text-primary' : 'hover:bg-surface-100'"
            :aria-current="thread.id === activeId ? 'page' : undefined"
            @click="emit('select', thread.id)"
          >
            <MessageSquare :size="15" class="shrink-0 opacity-70" />
            <span class="truncate">{{ thread.title }}</span>
          </button>
          <Button
            text
            rounded
            severity="secondary"
            aria-label="删除 AI 对话"
            title="删除对话"
            class="absolute right-1 top-1 size-8! p-0! opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus:opacity-100"
            :disabled="busy"
            @click.stop="emit('delete', thread)"
          >
            <Trash2 :size="14" />
          </Button>
        </li>
      </ul>
    </nav>
  </aside>
</template>
