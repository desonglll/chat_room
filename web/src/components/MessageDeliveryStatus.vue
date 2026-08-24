<script setup lang="ts">
import { Clock3, RotateCcw } from 'lucide-vue-next'
import type { DeliveryState } from '../types'

defineProps<{
  state: DeliveryState
}>()

const emit = defineEmits<{
  retry: []
}>()
</script>

<template>
  <span v-if="state === 'sending'" class="cr-delivery-state flex justify-end text-muted-color" title="发送中">
    <Clock3 :size="12" aria-label="发送中" />
  </span>
  <button
    v-else-if="state === 'failed'"
    type="button"
    class="cr-delivery-state ml-auto grid size-7 touch-manipulation place-items-center rounded-full text-danger outline-none hover:bg-danger/10 focus-visible:ring-2 focus-visible:ring-danger"
    aria-label="重新发送"
    title="发送失败，点击重试"
    @click="emit('retry')"
  >
    <RotateCcw :size="13" aria-hidden="true" />
  </button>
</template>
