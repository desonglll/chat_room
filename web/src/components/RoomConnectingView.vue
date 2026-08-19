<script setup lang="ts">
import { computed } from 'vue'
import { LoaderCircle } from 'lucide-vue-next'
import { useDelayedVisibility } from '../composables/useDelayedVisibility'

const waiting = computed(() => true)
const visible = useDelayedVisibility(waiting)
</script>

<template>
  <section class="flex min-h-0 flex-1 items-center justify-center bg-surface-50" aria-live="polite">
    <span v-if="visible" class="flex items-center gap-2 text-xs text-muted-color">
      <LoaderCircle :size="15" class="animate-spin motion-reduce:animate-none" />
      正在恢复连接
    </span>
  </section>
</template>
