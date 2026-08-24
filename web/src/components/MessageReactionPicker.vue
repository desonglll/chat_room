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
    class="grid size-8 touch-manipulation place-items-center rounded-md text-muted-color opacity-100 outline-none transition-[background-color,color,opacity,transform] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] hover:bg-surface-200 hover:text-primary focus-visible:ring-2 focus-visible:ring-primary active:scale-[0.97] motion-reduce:transform-none motion-reduce:transition-none sm:size-7 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100"
    aria-label="添加表情回应"
    title="添加回应"
    @click="popover.toggle($event)"
  >
    <SmilePlus :size="14" aria-hidden="true" />
  </button>
  <Popover ref="popover">
    <div class="flex items-center gap-1" role="toolbar" aria-label="选择表情回应">
      <button
        v-for="emoji in QUICK_REACTIONS"
        :key="emoji"
        type="button"
        class="grid size-10 touch-manipulation place-items-center rounded-md text-xl outline-none transition-[background-color,transform] duration-[var(--cr-motion-fast)] [transition-timing-function:var(--cr-ease-out)] hover:bg-surface-100 focus-visible:ring-2 focus-visible:ring-primary active:scale-[0.97] motion-reduce:transform-none motion-reduce:transition-none"
        :aria-label="`回应 ${emoji}`"
        @click="select(emoji)"
      >
        {{ emoji }}
      </button>
    </div>
  </Popover>
</template>
