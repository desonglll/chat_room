<script setup lang="ts">
import type { MessageReaction, RoomMember } from '../types'

const props = defineProps<{
  reactions: MessageReaction[]
  participants: RoomMember[]
  currentUserId: string
  own: boolean
}>()
const emit = defineEmits<{ toggle: [emoji: string, active: boolean] }>()

function reacted(reaction: MessageReaction): boolean {
  return reaction.user_ids.includes(props.currentUserId)
}

function reactionTitle(reaction: MessageReaction): string {
  const names = reaction.user_ids.map(
    (userId) => props.participants.find((member) => member.user_id === userId)?.username || '成员',
  )
  return `${names.join('、')} 回应了 ${reaction.emoji}`
}
</script>

<template>
  <div v-if="reactions.length" class="mt-1 flex flex-wrap gap-1" :class="{ 'justify-end': own }">
    <button
      v-for="reaction in reactions"
      :key="reaction.emoji"
      type="button"
      class="flex min-h-8 min-w-11 touch-manipulation items-center justify-center gap-1 rounded-full border px-2 text-xs outline-none transition-[background-color,border-color,color,transform] focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1 active:scale-95 motion-reduce:transform-none motion-reduce:transition-none"
      :class="
        reacted(reaction)
          ? 'border-primary-300 bg-primary-50 text-primary-800 hover:bg-primary-100'
          : 'border-surface-200 bg-surface-0 text-surface-700 hover:border-primary-200 hover:bg-surface-50'
      "
      :title="reactionTitle(reaction)"
      :aria-label="`${reacted(reaction) ? '取消' : '添加'} ${reaction.emoji} 回应，当前 ${reaction.user_ids.length} 人`"
      @click="emit('toggle', reaction.emoji, !reacted(reaction))"
    >
      <span aria-hidden="true">{{ reaction.emoji }}</span>
      <span class="tabular-nums">{{ reaction.user_ids.length }}</span>
    </button>
  </div>
</template>
