<script setup lang="ts">
import { Bookmark, CornerUpLeft, Forward, Pin } from 'lucide-vue-next'
import MessageReactionPicker from './MessageReactionPicker.vue'

defineProps<{ enabled: boolean; favorited: boolean; pinnable: boolean; pinned: boolean }>()
const emit = defineEmits<{
  reaction: [emoji: string]
  reply: []
  forward: []
  favorite: []
  pin: []
}>()
</script>

<template>
  <div v-if="enabled" class="cr-message-actions" role="toolbar" aria-label="消息操作">
    <MessageReactionPicker @select="emit('reaction', $event)" />
    <button type="button" class="cr-message-inline-action" aria-label="回复消息" title="回复" @click="emit('reply')">
      <CornerUpLeft :size="14" aria-hidden="true" />
    </button>
    <button type="button" class="cr-message-inline-action" aria-label="转发消息" title="转发" @click="emit('forward')">
      <Forward :size="14" aria-hidden="true" />
    </button>
    <button
      type="button"
      class="cr-message-inline-action"
      :class="{ 'cr-message-inline-action--active': favorited }"
      :aria-label="favorited ? '取消收藏' : '收藏消息'"
      :title="favorited ? '取消收藏' : '收藏'"
      :aria-pressed="favorited"
      @click="emit('favorite')"
    >
      <Bookmark :size="14" :fill="favorited ? 'currentColor' : 'none'" aria-hidden="true" />
    </button>
    <button
      v-if="pinnable"
      type="button"
      class="cr-message-inline-action"
      :class="{ 'cr-message-inline-action--active': pinned }"
      :aria-label="pinned ? '取消置顶' : '置顶消息'"
      :title="pinned ? '取消置顶' : '置顶'"
      :aria-pressed="pinned"
      @click="emit('pin')"
    >
      <Pin :size="14" :fill="pinned ? 'currentColor' : 'none'" aria-hidden="true" />
    </button>
  </div>
</template>
