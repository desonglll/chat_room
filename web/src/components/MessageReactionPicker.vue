<script setup lang="ts">
import { ref } from 'vue'
import { SmilePlus } from 'lucide-vue-next'
import Popover from 'primevue/popover'
import { QUICK_REACTIONS } from '../messageReactions'

const emit = defineEmits<{ select: [emoji: string] }>()
const popover = ref()

function select(emoji: string): void {
  emit('select', emoji)
  popover.value?.hide()
}
</script>

<template>
  <button
    type="button"
    class="grid size-6 place-items-center rounded text-muted-color opacity-100 transition hover:bg-surface-200 hover:text-primary active:scale-90 sm:opacity-0 sm:group-hover:opacity-100"
    aria-label="添加表情回应"
    title="添加回应"
    @click="popover.toggle($event)"
  >
    <SmilePlus :size="14" />
  </button>
  <Popover ref="popover">
    <div class="flex items-center gap-1" role="toolbar" aria-label="选择表情回应">
      <button
        v-for="emoji in QUICK_REACTIONS"
        :key="emoji"
        type="button"
        class="grid size-9 place-items-center rounded-md text-xl transition hover:bg-surface-100 active:scale-90"
        :aria-label="`回应 ${emoji}`"
        @click="select(emoji)"
      >
        {{ emoji }}
      </button>
    </div>
  </Popover>
</template>
