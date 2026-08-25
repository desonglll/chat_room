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
  <aside class="flex min-h-0 flex-col border-r border-surface-200 bg-surface-0">
    <div class="flex h-14 shrink-0 items-center justify-between border-b border-surface-200 px-3">
      <span class="text-sm font-semibold">AI 对话</span>
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
    <nav class="min-h-0 flex-1 overflow-y-auto p-2" aria-label="AI 对话列表">
      <p v-if="!threads.length" class="px-2 py-5 text-center text-xs text-muted-color">暂无对话</p>
      <ul v-else class="space-y-1">
        <li v-for="thread in threads" :key="thread.id" class="group relative">
          <button
            type="button"
            class="flex min-h-10 w-full items-center gap-2 rounded-md px-2.5 pr-9 text-left text-sm transition-colors"
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
            class="absolute right-1 top-1 size-8! p-0! opacity-0 group-hover:opacity-100 focus:opacity-100"
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
