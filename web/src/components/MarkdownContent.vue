<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { aiSourceRoute } from '../aiUi'
import { renderMarkdown } from '../markdown'
import type { AiCitationSource } from '../types'

const props = withDefaults(defineProps<{ content: string; sources?: AiCitationSource[] }>(), { sources: () => [] })
const html = computed(() => renderMarkdown(props.content))
const root = ref<HTMLElement | null>(null)
const router = useRouter()

function decorateCitations(): void {
  if (!root.value || !props.sources.length) return
  const byLabel = new Map(props.sources.map((source) => [source.label.toUpperCase(), source]))
  const walker = document.createTreeWalker(root.value, NodeFilter.SHOW_TEXT)
  const nodes: Text[] = []
  while (walker.nextNode()) {
    const node = walker.currentNode as Text
    if (node.parentElement?.closest('a, code, pre') || !/\[[A-Z]\d+\]/i.test(node.data)) continue
    nodes.push(node)
  }
  for (const node of nodes) {
    const parts = node.data.split(/(\[[A-Z]\d+\])/gi)
    if (parts.length === 1) continue
    const fragment = document.createDocumentFragment()
    for (const part of parts) {
      const label = /^\[([A-Z]\d+)\]$/i.exec(part)?.[1]?.toUpperCase()
      const source = label ? byLabel.get(label) : undefined
      if (!source) {
        fragment.append(document.createTextNode(part))
        continue
      }
      const link = document.createElement('a')
      link.href = router.resolve(aiSourceRoute(source)).href
      link.dataset.aiSource = source.message_id
      link.textContent = `[${label}]`
      link.title = `定位到 ${label} 原文`
      fragment.append(link)
    }
    node.replaceWith(fragment)
  }
}

function handleClick(event: MouseEvent): void {
  const link = (event.target as HTMLElement | null)?.closest<HTMLAnchorElement>('a[data-ai-source]')
  const source = props.sources.find((item) => item.message_id === link?.dataset.aiSource)
  if (!link || !source) return
  event.preventDefault()
  void router.push(aiSourceRoute(source)).catch(() => {})
}

watch([html, () => props.sources], async () => {
  await nextTick()
  decorateCitations()
})
onMounted(decorateCitations)
</script>

<template>
  <div ref="root" class="cr-markdown min-w-0 max-w-full break-words" @click="handleClick" v-html="html" />
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
  overflow-wrap: anywhere;
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
  overflow-wrap: normal;
  padding: 0;
}

.cr-markdown :deep(table) {
  display: block;
  max-width: 100%;
  overflow-x: auto;
  border-collapse: collapse;
}

.cr-markdown :deep(th),
.cr-markdown :deep(td) {
  padding: 0.4rem 0.55rem;
  border: 1px solid var(--p-surface-200);
  text-align: left;
  white-space: nowrap;
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
