<script setup lang="ts">
import { ArrowLeft, Bot } from 'lucide-vue-next'
import Button from 'primevue/button'

defineProps<{ title: string; ready: boolean }>()
const emit = defineEmits<{ back: [] }>()
</script>

<template>
  <header class="cr-page-header flex shrink-0 items-center gap-2 px-3 sm:gap-3 sm:px-7">
    <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')">
      <ArrowLeft :size="19" />
    </Button>
    <span class="hidden size-9 shrink-0 place-items-center rounded-md bg-primary-50 text-primary sm:grid"
      ><Bot :size="20"
    /></span>
    <div class="min-w-0 flex-1">
      <h1 class="text-base font-semibold">AI 助手</h1>
      <p class="mt-0.5 truncate text-xs text-muted-color">{{ title }}</p>
    </div>
    <span
      class="shrink-0 rounded-sm px-2 py-1 text-[11px] font-medium"
      :class="ready ? 'bg-green-50 text-green-700' : 'bg-surface-100 text-muted-color'"
    >
      {{ ready ? '可用' : '不可用' }}
    </span>
  </header>
</template>
