<script setup lang="ts">
import { avatarColor } from '../avatarColor'
import { conversationAttentionCount, conversationDisplayTitle, conversationPreview } from '../conversationState'
import type { ConversationSummary } from '../types'
import IconSprite from './IconSprite.vue'

defineProps<{
  conversation: ConversationSummary
  selected: boolean
  collapsed: boolean
  revealPreview: boolean
}>()

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
    class="cr-conversation-avatar relative grid size-10 shrink-0 place-items-center rounded-full text-sm font-semibold text-white"
    :style="{
      backgroundColor: avatarColor(
        conversation.kind === 'direct' ? conversation.peer?.id || conversation.room_id : conversation.room_id,
      ),
    }"
  >
    <template v-if="conversation.avatar_emoji">{{ conversation.avatar_emoji }}</template>
    <IconSprite v-else-if="conversation.kind === 'group'" name="rooms" :size="18" />
    <template v-else>{{ conversationDisplayTitle(conversation).slice(0, 1).toUpperCase() }}</template>
    <span
      v-if="collapsed && conversationAttentionCount(conversation) > 0"
      class="absolute -right-1 -top-1 grid min-w-5 place-items-center rounded-full border-2 border-surface-0 bg-primary px-1 text-[10px] font-semibold leading-4 text-white"
    >
      {{ conversationAttentionCount(conversation) > 99 ? '99+' : conversationAttentionCount(conversation) }}
    </span>
  </span>
  <span class="min-w-0 flex-1" :class="{ 'md:hidden': collapsed }">
    <span class="flex items-baseline gap-2">
      <strong class="cr-conversation-title min-w-0 flex-1 truncate text-sm font-semibold">{{
        conversationDisplayTitle(conversation)
      }}</strong>
      <small
        class="cr-conversation-time shrink-0 text-[11px]"
        :class="conversation.unread_count ? 'font-semibold' : ''"
      >
        {{ formatActivity(conversation.last_activity_at) }}
      </small>
    </span>
    <span class="mt-1 flex min-w-0 items-center gap-2">
      <small
        v-if="revealPreview || conversation.pending_join_requests > 0"
        class="cr-conversation-preview min-w-0 flex-1 truncate text-xs"
      >
        <template v-if="revealPreview && conversation.last_message?.sender && conversation.kind === 'group'">
          {{ conversation.last_message.sender }}:
        </template>
        {{ conversationPreview(conversation, revealPreview) }}
      </small>
      <small
        v-else
        class="flex min-w-0 flex-1 items-center gap-1.5 truncate text-xs text-muted-color"
        aria-hidden="true"
      >
        <IconSprite :name="conversation.kind === 'direct' ? 'message' : 'rooms'" :size="13" />
        {{ conversation.kind === 'direct' ? '私聊' : '群聊' }}
      </small>
      <span
        v-if="conversationAttentionCount(conversation) > 0"
        class="cr-unread-badge grid min-w-5 shrink-0 place-items-center rounded-full px-1.5 text-[10px] font-semibold leading-5"
      >
        {{ conversationAttentionCount(conversation) > 99 ? '99+' : conversationAttentionCount(conversation) }}
      </span>
    </span>
  </span>
</template>
