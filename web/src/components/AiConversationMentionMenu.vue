<script setup lang="ts">
import { Hash } from 'lucide-vue-next'
import type { ConversationMentionRange, MentionableConversation } from '../assistantMentions'

defineProps<{
  range: ConversationMentionRange | null
  candidates: MentionableConversation[]
  activeIndex: number
}>()

const emit = defineEmits<{ choose: [room: MentionableConversation] }>()
</script>

<template>
  <div
    v-if="range"
    class="cr-composer-popover absolute bottom-[calc(100%+0.5rem)] left-0 z-20 w-[min(24rem,100%)] overflow-hidden rounded-md border border-surface-200 bg-surface-0 shadow-lg"
  >
    <p class="border-b border-surface-200 px-3 py-2 text-xs font-medium text-muted-color">选择会话</p>
    <ul v-if="candidates.length" role="listbox" class="max-h-64 overflow-y-auto p-1">
      <li v-for="(room, index) in candidates" :key="room.roomId" role="option">
        <button
          type="button"
          class="flex min-h-10 w-full items-center gap-2 rounded-sm px-2 text-left text-sm"
          :class="index === activeIndex ? 'bg-primary-50 text-primary' : 'hover:bg-surface-100'"
          :aria-selected="index === activeIndex"
          @mousedown.prevent="emit('choose', room)"
        >
          <Hash :size="15" class="shrink-0" />
          <span class="truncate">{{ room.title }}</span>
        </button>
      </li>
    </ul>
    <p v-else class="px-3 py-5 text-center text-sm text-muted-color">没有匹配的会话</p>
  </div>
</template>
