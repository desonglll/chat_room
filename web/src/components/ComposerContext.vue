<script setup lang="ts">
import { Pencil, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import type { BroadcastMessage } from '../types'

defineProps<{
  editing: BroadcastMessage | null
  replying: BroadcastMessage | null
}>()

const emit = defineEmits<{
  cancelEdit: []
  cancelReply: []
}>()

function replySummary(message: BroadcastMessage): string {
  if (message.recalled_at) return '消息已撤回'
  return message.content || (message.attachment ? `[附件] ${message.attachment.file_name}` : '[消息]')
}
</script>

<template>
  <div v-if="editing" class="flex items-center gap-3 px-3 pt-3 sm:px-7">
    <Pencil :size="16" class="shrink-0 text-primary" />
    <div class="min-w-0 flex-1">
      <strong class="block truncate text-xs text-primary">编辑已发送消息</strong>
      <span class="mt-0.5 block truncate text-xs text-muted-color">{{ editing.content }}</span>
    </div>
    <Button
      type="button"
      text
      rounded
      severity="secondary"
      aria-label="取消编辑"
      title="取消编辑"
      @click="emit('cancelEdit')"
    >
      <X :size="17" />
    </Button>
  </div>

  <div v-if="replying" class="flex items-center gap-3 px-3 pt-3 sm:px-7">
    <div class="min-w-0 flex-1 border-l-[3px] border-primary pl-2.5">
      <strong class="block truncate text-xs text-primary">回复 {{ replying.sender }}</strong>
      <span class="mt-0.5 block truncate text-xs text-muted-color">{{ replySummary(replying) }}</span>
    </div>
    <Button
      type="button"
      text
      rounded
      severity="secondary"
      aria-label="取消回复"
      title="取消回复"
      @click="emit('cancelReply')"
    >
      <X :size="17" />
    </Button>
  </div>
</template>
