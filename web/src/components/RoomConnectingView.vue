<script setup lang="ts">
import { computed } from 'vue'
import { LoaderCircle, RefreshCw } from 'lucide-vue-next'
import { useDelayedVisibility } from '../composables/useDelayedVisibility'

const props = defineProps<{ error: string }>()
const emit = defineEmits<{ retry: [] }>()
const waiting = computed(() => !props.error)
const visible = useDelayedVisibility(waiting)
</script>

<template>
  <section class="flex min-h-0 flex-1 items-center justify-center bg-surface-50" aria-live="polite">
    <span v-if="visible" class="flex items-center gap-2 text-xs text-muted-color">
      <LoaderCircle :size="15" class="animate-spin motion-reduce:animate-none" />
      正在恢复连接
    </span>
    <div v-else-if="error" class="flex max-w-sm flex-col items-center gap-3 px-5 text-center">
      <p class="text-sm text-color">{{ error }}</p>
      <button
        type="button"
        class="inline-flex h-9 items-center gap-2 rounded-md border border-surface-300 bg-surface-0 px-3 text-sm font-medium text-color transition-colors hover:border-primary hover:text-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
        @click="emit('retry')"
      >
        <RefreshCw :size="15" />
        重新连接
      </button>
    </div>
  </section>
</template>
