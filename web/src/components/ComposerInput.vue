<script setup lang="ts">
import { defineAsyncComponent, nextTick, ref } from 'vue'
import { Send, Smile } from 'lucide-vue-next'
import Button from 'primevue/button'
import Popover from 'primevue/popover'
import Textarea from 'primevue/textarea'

const EmojiPicker = defineAsyncComponent(() => import('./EmojiPicker.vue'))

const props = withDefaults(
  defineProps<{
    modelValue: string
    disabled: boolean
    canSend: boolean
    loading?: boolean
    placeholder?: string
    ariaLabel?: string
    ariaExpanded?: boolean
    maxLength?: number
    formId?: string
  }>(),
  {
    loading: false,
    placeholder: '输入消息…',
    ariaLabel: '消息',
    ariaExpanded: undefined,
    maxLength: 4096,
    formId: undefined,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  submit: []
  caret: [value: string, caret: number]
  keydown: [event: KeyboardEvent]
  paste: [event: ClipboardEvent]
  composition: [active: boolean]
}>()

const input = ref<{ $el?: HTMLTextAreaElement } | null>(null)
const emojiPopover = ref()

function element(): HTMLTextAreaElement | null {
  return input.value?.$el || null
}

function focus(): void {
  element()?.focus()
}

async function focusAt(caret: number): Promise<void> {
  await nextTick()
  const textarea = element()
  textarea?.focus()
  textarea?.setSelectionRange(caret, caret)
}

function emitCaret(event: Event): void {
  const textarea = event.target as HTMLTextAreaElement
  emit('caret', textarea.value, textarea.selectionStart)
}

function handleInput(event: Event): void {
  const textarea = event.target as HTMLTextAreaElement
  emit('update:modelValue', textarea.value)
  emitCaret(event)
}

function insertEmoji(emoji: string): void {
  const textarea = element()
  const start = textarea?.selectionStart ?? props.modelValue.length
  const end = textarea?.selectionEnd ?? start
  const value = `${props.modelValue.slice(0, start)}${emoji}${props.modelValue.slice(end)}`
  const caret = start + emoji.length
  emit('update:modelValue', value)
  emit('caret', value, caret)
  emojiPopover.value?.hide()
  void focusAt(caret)
}

defineExpose({ element, focus, focusAt })
</script>

<template>
  <form
    :id="formId"
    class="cr-composer shrink-0 pb-[env(safe-area-inset-bottom)] md:pb-0"
    data-testid="chat-form"
    @submit.prevent="emit('submit')"
  >
    <slot name="context" />
    <div class="cr-composer-inner flex items-end gap-1">
      <slot name="leading-tools" />
      <Button
        type="button"
        text
        rounded
        severity="secondary"
        class="cr-composer-tool !size-10 shrink-0"
        :disabled="disabled"
        aria-label="插入表情"
        title="表情"
        @click="emojiPopover.toggle($event)"
      >
        <Smile :size="19" />
      </Button>
      <Popover ref="emojiPopover" class="cr-popover-bottom-left">
        <EmojiPicker @select="insertEmoji" />
      </Popover>
      <slot name="trailing-tools" />
      <label class="sr-only" :for="`${formId || 'message'}-input`">{{ ariaLabel }}</label>
      <div class="relative min-w-0 flex-1">
        <Textarea
          :id="`${formId || 'message'}-input`"
          ref="input"
          :model-value="modelValue"
          name="message"
          autocomplete="off"
          rows="1"
          :maxlength="maxLength"
          auto-resize
          :disabled="disabled"
          :placeholder="placeholder"
          :aria-label="ariaLabel"
          :aria-autocomplete="ariaExpanded === undefined ? undefined : 'list'"
          :aria-expanded="ariaExpanded"
          class="cr-composer-input max-h-32 min-h-10 w-full overflow-y-auto! [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
          @paste="emit('paste', $event)"
          @compositionstart="emit('composition', true)"
          @compositionend="emit('composition', false)"
          @keydown="emit('keydown', $event)"
          @input="handleInput"
          @click="emitCaret"
          @keyup="emitCaret"
        />
        <slot name="popover" />
      </div>
      <Button
        type="submit"
        rounded
        class="cr-composer-send !size-10 shrink-0"
        :loading="loading"
        :disabled="disabled || !canSend"
        :aria-label="ariaLabel === '消息' ? '发送消息' : `发送${ariaLabel}`"
        title="发送"
      >
        <Send v-if="!loading" :size="18" />
      </Button>
    </div>
    <slot name="footer" />
  </form>
</template>
