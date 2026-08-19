<script setup lang="ts">
import Badge from 'primevue/badge'
import { avatarColor } from '../avatarColor'
import { conversationAttentionCount, conversationPreview } from '../conversationState'
import type { ConversationSummary } from '../types'
import IconSprite from './IconSprite.vue'

defineProps<{ conversation: ConversationSummary; selected: boolean; collapsed: boolean }>()

function formatActivity(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(date)
  }
  return new Intl.DateTimeFormat('zh-CN', { month: '2-digit', day: '2-digit' }).format(date)
}
</script>

<template>
  <span
    class="relative grid size-11 shrink-0 place-items-center rounded-full text-base font-semibold text-white"
    :style="{
      backgroundColor: avatarColor(
        conversation.kind === 'direct' ? conversation.peer?.id || conversation.room_id : conversation.room_id,
      ),
    }"
  >
    <template v-if="conversation.avatar_emoji">{{ conversation.avatar_emoji }}</template>
    <IconSprite v-else-if="conversation.kind === 'group'" name="rooms" :size="18" />
    <template v-else>{{ conversation.title.slice(0, 1).toUpperCase() }}</template>
    <Badge
      v-if="collapsed && conversationAttentionCount(conversation) > 0"
      :value="conversationAttentionCount(conversation) > 99 ? '99+' : String(conversationAttentionCount(conversation))"
      severity="danger"
      class="absolute -right-1 -top-1"
    />
  </span>
  <span class="min-w-0 flex-1" :class="{ 'md:hidden': collapsed }">
    <span class="flex items-baseline gap-2">
      <strong class="min-w-0 flex-1 truncate text-sm font-semibold">{{ conversation.title }}</strong>
      <small class="shrink-0 text-[11px]" :class="conversation.unread_count ? 'text-primary' : 'text-muted-color'">
        {{ formatActivity(conversation.last_activity_at) }}
      </small>
    </span>
    <span class="mt-1 flex min-w-0 items-center gap-2">
      <small class="min-w-0 flex-1 truncate text-xs text-muted-color">
        <template v-if="conversation.last_message?.sender && conversation.kind === 'group'">
          {{ conversation.last_message.sender }}:
        </template>
        {{ conversationPreview(conversation) }}
      </small>
      <Badge
        v-if="conversationAttentionCount(conversation) > 0"
        :value="
          conversationAttentionCount(conversation) > 99 ? '99+' : String(conversationAttentionCount(conversation))
        "
        severity="danger"
        class="shrink-0"
      />
    </span>
  </span>
</template>
