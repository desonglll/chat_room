<script setup lang="ts">
import { computed, ref } from 'vue'
import { RotateCcw } from 'lucide-vue-next'
import Button from 'primevue/button'
import { DEFAULT_PRIVACY_LOCK_SHORTCUT, formatPrivacyLockShortcut, privacyLockShortcutFromEvent } from '../privacyLock'
import type { PrivacyLockShortcut } from '../types'

const props = defineProps<{ modelValue: PrivacyLockShortcut }>()
const emit = defineEmits<{ 'update:modelValue': [shortcut: PrivacyLockShortcut] }>()
const recording = ref(false)
const error = ref('')
const label = computed(() => (recording.value ? '请按下新的组合键' : formatPrivacyLockShortcut(props.modelValue)))

function capture(event: KeyboardEvent): void {
  if (!recording.value) return
  event.preventDefault()
  event.stopPropagation()
  if (event.code === 'Escape') {
    recording.value = false
    error.value = ''
    return
  }
  const shortcut = privacyLockShortcutFromEvent(event)
  if (!shortcut) {
    if (!['Alt', 'Control', 'Meta', 'Shift'].includes(event.key)) {
      error.value = '快捷键需要 Ctrl/⌘ 或 Alt/⌥'
    }
    return
  }
  emit('update:modelValue', shortcut)
  recording.value = false
  error.value = ''
}

function reset(): void {
  emit('update:modelValue', { ...DEFAULT_PRIVACY_LOCK_SHORTCUT })
  recording.value = false
  error.value = ''
}
</script>

<template>
  <div>
    <div class="flex min-h-10 items-stretch gap-2">
      <button
        type="button"
        class="min-w-0 flex-1 rounded-md border px-3 text-sm font-medium transition-[background-color,border-color,box-shadow] duration-[var(--cr-motion-normal)] [transition-timing-function:ease] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary motion-reduce:transition-none"
        :class="
          recording
            ? 'border-primary bg-primary-50 text-primary'
            : 'border-surface-300 bg-surface-0 hover:border-primary'
        "
        :aria-pressed="recording"
        @click="recording = !recording"
        @keydown="capture"
        @blur="recording = false"
      >
        {{ label }}
      </button>
      <Button
        type="button"
        text
        rounded
        severity="secondary"
        aria-label="恢复默认锁屏快捷键"
        title="恢复默认锁屏快捷键"
        @click="reset"
      >
        <RotateCcw :size="16" />
      </Button>
    </div>
    <small v-if="error" class="mt-1.5 block text-danger" role="alert">{{ error }}</small>
  </div>
</template>
