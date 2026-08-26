<script setup lang="ts">
import { Bookmark } from 'lucide-vue-next'
import type { BroadcastMessage } from '../types'
import MarkdownContent from './MarkdownContent.vue'

interface ContentSegment {
  text: string
  mention: boolean
}

defineProps<{
  message: BroadcastMessage
  groupEnd: boolean
  segments: ContentSegment[]
  currentUserId: string
}>()
</script>

<template>
  <article
    v-if="message.favorite_id"
    class="mt-1 min-w-[min(18rem,72vw)] max-w-[min(34rem,78vw)] overflow-hidden rounded-md border border-primary-200 bg-surface-0 shadow-sm"
  >
    <header
      class="flex items-center gap-2 border-b border-primary-100 bg-primary-50 px-3 py-2 text-xs text-primary-700"
    >
      <Bookmark :size="15" fill="currentColor" aria-hidden="true" />
      <strong>收藏文档</strong>
      <span v-if="message.forwarded_from" class="ml-auto truncate text-muted-color">
        来自 {{ message.forwarded_from.sender }} · {{ message.forwarded_from.room_name }}
      </span>
    </header>
    <MarkdownContent :content="message.content" class="px-3 py-3 text-[15px] leading-6 text-color" />
  </article>
  <p
    v-else
    class="cr-message-bubble mt-1 whitespace-pre-wrap break-words px-3 py-2.5 text-[15px] leading-6"
    :class="
      message.sender_id
        ? [
            message.sender_id === currentUserId
              ? 'cr-bubble-outgoing cr-message-bubble--outgoing'
              : 'cr-bubble-incoming cr-message-bubble--incoming',
            groupEnd
              ? message.sender_id === currentUserId
                ? 'cr-message-bubble--end rounded-br-sm'
                : 'cr-message-bubble--end rounded-bl-sm'
              : '',
          ]
        : ['cr-bubble-incoming cr-message-bubble--incoming', groupEnd ? 'cr-message-bubble--end rounded-bl-sm' : '']
    "
  >
    <template v-for="(segment, index) in segments" :key="index">
      <strong
        v-if="segment.mention"
        class="font-semibold text-primary"
        :class="{ 'text-inherit! underline': message.sender_id === currentUserId }"
      >
        {{ segment.text }}
      </strong>
      <template v-else>{{ segment.text }}</template>
    </template>
  </p>
</template>
