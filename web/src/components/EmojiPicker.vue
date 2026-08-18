<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import 'emoji-picker-element'
import zhCN from 'emoji-picker-element/i18n/zh_CN'
import type Picker from 'emoji-picker-element/picker'

interface EmojiClickDetail {
  unicode?: string
  emoji: { unicode?: string }
}

const emit = defineEmits<{ select: [emoji: string] }>()

const pickerEl = ref<Picker | null>(null)
// emoji-picker-element only reads its own light/dark palette from a
// `.light`/`.dark` class on the host element (or a `prefers-color-scheme`
// media query as a last resort) — it has no way to see our `[data-theme]`
// attribute on its own, so this observer is what actually keeps it in sync
// with the rest of the app instead of being permanently stuck light.
const isDark = ref(document.documentElement.getAttribute('data-theme') === 'dark')
let themeObserver: MutationObserver | undefined

function onEmojiClick(event: Event): void {
  const detail = (event as CustomEvent<EmojiClickDetail>).detail
  const emoji = detail?.unicode || detail?.emoji?.unicode
  if (emoji) emit('select', emoji)
}

onMounted(() => {
  if (pickerEl.value) pickerEl.value.i18n = zhCN
  pickerEl.value?.addEventListener('emoji-click', onEmojiClick)
  themeObserver = new MutationObserver(() => {
    isDark.value = document.documentElement.getAttribute('data-theme') === 'dark'
  })
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

onBeforeUnmount(() => themeObserver?.disconnect())
</script>

<template>
  <emoji-picker
    ref="pickerEl"
    :class="isDark ? 'dark' : 'light'"
    locale="zh"
    data-source="/emoji-data-zh.json"
  />
</template>

<style scoped>
emoji-picker {
  width: 20rem;
  max-width: 84vw;
  height: 22rem;
  /* Route the picker's own theming variables through our semantic tokens
     (already light/dark-aware via [data-theme]) instead of fixed hex values,
     so it matches the app's actual palette in both themes, not just "some
     light theme" / "some dark theme". */
  --background: var(--cr-surface);
  --border-color: var(--cr-border);
  --indicator-color: var(--cr-primary);
  --outline-color: var(--cr-primary);
  --input-border-color: var(--cr-border);
  --input-font-color: var(--cr-text);
  --input-placeholder-color: var(--cr-text-muted);
  --category-font-color: var(--cr-text-muted);
  --button-hover-background: var(--cr-surface-subtle);
  --button-active-background: var(--cr-primary-soft);
  --category-emoji-size: 1.25rem;
}
</style>
