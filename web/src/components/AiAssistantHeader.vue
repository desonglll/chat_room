<script setup lang="ts">
import { ArrowLeft, Bot } from 'lucide-vue-next'
import Button from 'primevue/button'

defineProps<{ title: string; ready: boolean }>()
const emit = defineEmits<{ back: [] }>()
</script>

<template>
  <header class="cr-page-header flex shrink-0 items-center gap-3 px-4 sm:px-7">
    <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')">
      <ArrowLeft :size="19" />
    </Button>
    <span class="grid size-9 shrink-0 place-items-center rounded-md bg-primary-50 text-primary"
      ><Bot :size="20"
    /></span>
    <div class="min-w-0 flex-1">
      <h1 class="text-base font-semibold">AI 助手</h1>
      <p class="mt-0.5 truncate text-xs text-muted-color">{{ title }}</p>
    </div>
    <span
      class="rounded-sm px-2 py-1 text-[11px] font-medium"
      :class="ready ? 'bg-green-50 text-green-700' : 'bg-surface-100 text-muted-color'"
    >
      {{ ready ? '可用' : '不可用' }}
    </span>
  </header>
</template>
