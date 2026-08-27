<script setup lang="ts">
import { Sparkles, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import type { AiSelectedMessage } from '../aiSelectedContext'

defineProps<{ messages: AiSelectedMessage[] }>()
defineEmits<{ clear: [] }>()
</script>

<template>
  <section class="border-b border-surface-200 bg-surface-50 px-3 py-2 sm:px-5" aria-label="AI 消息上下文">
    <div class="flex items-center gap-2">
      <Sparkles :size="16" class="shrink-0 text-primary" />
      <strong class="min-w-0 flex-1 text-xs">已选择 {{ messages.length }} 条聊天记录作为上下文</strong>
      <Button
        text
        rounded
        severity="secondary"
        size="small"
        aria-label="清除所选上下文"
        title="清除上下文"
        @click="$emit('clear')"
      >
        <X :size="16" />
      </Button>
    </div>
    <div class="mt-1.5 flex max-h-20 flex-col gap-1 overflow-y-auto text-xs text-muted-color">
      <p v-for="message in messages" :key="message.messageId" class="truncate">
        <strong class="text-color">{{ message.sender }}</strong
        >：{{ message.preview }}
      </p>
    </div>
  </section>
</template>
