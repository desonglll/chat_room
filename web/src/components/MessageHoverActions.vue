<script setup lang="ts">
import { Bookmark, CornerUpLeft, Forward } from 'lucide-vue-next'
import MessageReactionPicker from './MessageReactionPicker.vue'

defineProps<{ enabled: boolean }>()
const emit = defineEmits<{
  reaction: [emoji: string]
  reply: []
  forward: []
  favorite: []
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
    <button type="button" class="cr-message-inline-action" aria-label="收藏消息" title="收藏" @click="emit('favorite')">
      <Bookmark :size="14" aria-hidden="true" />
    </button>
  </div>
</template>
