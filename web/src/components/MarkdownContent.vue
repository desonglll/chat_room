<script setup lang="ts">
import { computed } from 'vue'
import { renderMarkdown } from '../markdown'

const props = defineProps<{ content: string }>()
const html = computed(() => renderMarkdown(props.content))
</script>

<template>
  <div class="cr-markdown break-words" v-html="html" />
</template>

<style scoped>
.cr-markdown :deep(p),
.cr-markdown :deep(ul),
.cr-markdown :deep(ol),
.cr-markdown :deep(pre),
.cr-markdown :deep(blockquote) {
  margin: 0 0 0.65rem;
}

.cr-markdown :deep(*:last-child) {
  margin-bottom: 0;
}

.cr-markdown :deep(ul),
.cr-markdown :deep(ol) {
  padding-left: 1.35rem;
}

.cr-markdown :deep(ul) {
  list-style: disc;
}

.cr-markdown :deep(ol) {
  list-style: decimal;
}

.cr-markdown :deep(h1),
.cr-markdown :deep(h2),
.cr-markdown :deep(h3) {
  margin: 0.8rem 0 0.35rem;
  font-size: 1em;
  font-weight: 650;
}

.cr-markdown :deep(code) {
  border-radius: 0.2rem;
  background: var(--p-surface-100);
  padding: 0.1rem 0.3rem;
  font-size: 0.88em;
}

.cr-markdown :deep(pre) {
  overflow-x: auto;
  border: 1px solid var(--p-surface-200);
  border-radius: 0.3rem;
  background: var(--p-surface-50);
  padding: 0.75rem;
}

.cr-markdown :deep(pre code) {
  background: transparent;
  padding: 0;
}

.cr-markdown :deep(blockquote) {
  border-left: 3px solid var(--p-primary-color);
  color: var(--p-text-muted-color);
  padding-left: 0.75rem;
}

.cr-markdown :deep(a) {
  color: var(--p-primary-color);
  text-decoration: underline;
}
</style>
