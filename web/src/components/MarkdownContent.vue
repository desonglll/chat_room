<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, ref, useId, watch } from 'vue'
import { useRouter } from 'vue-router'
import { aiSourceRoute, inlineAiAttachments } from '../aiUi'
import { renderMarkdown } from '../markdown'
import type { AiCitationSource, Attachment } from '../types'
import MessageAttachment from './MessageAttachment.vue'

const ImageViewerGallery = defineAsyncComponent(() => import('./ImageViewerGallery.vue'))
const props = withDefaults(defineProps<{ content: string; sources?: AiCitationSource[] }>(), { sources: () => [] })
const html = computed(() => renderMarkdown(props.content))
const root = ref<HTMLElement | null>(null)
const router = useRouter()
const targetPrefix = `ai-attachment-${useId().replaceAll(':', '')}`
const placements = ref<Array<{ targetId: string; source: AiCitationSource; attachment: Attachment }>>([])
const previewImageId = ref('')
const images = computed(() =>
  placements.value
    .map((placement) => placement.attachment)
    .filter((attachment) => attachment.mime_type.startsWith('image/')),
)

function decorateCitations(): void {
  placements.value = []
  if (!root.value || !props.sources.length) return
  const byLabel = new Map(props.sources.map((source) => [source.label.toUpperCase(), source]))
  const inlineSources = new Set(inlineAiAttachments(props.content, props.sources).map((source) => source.label))
  const placedAttachments = new Set<string>()
  const attachmentGroups = new Map<HTMLElement, HTMLElement>()
  const nextPlacements: typeof placements.value = []
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
    const attachmentLinks: Array<{ link: HTMLAnchorElement; source: AiCitationSource }> = []
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
      if (source.attachment && inlineSources.has(source.label)) attachmentLinks.push({ link, source })
    }
    node.replaceWith(fragment)
    for (const { link, source } of attachmentLinks) {
      const attachment = source.attachment
      if (!attachment || placedAttachments.has(attachment.id)) continue
      const block = link.closest<HTMLElement>('p, li, blockquote, td, dd') || link.parentElement
      if (!block) continue
      let group = attachmentGroups.get(block)
      if (!group) {
        group = document.createElement('div')
        group.className = 'ai-inline-attachment-group'
        if (block === root.value || block.matches('li, td, dd')) block.append(group)
        else block.after(group)
        attachmentGroups.set(block, group)
      }
      const target = document.createElement('div')
      const targetId = `${targetPrefix}-${nextPlacements.length}`
      target.id = targetId
      group.append(target)
      placedAttachments.add(attachment.id)
      nextPlacements.push({ targetId, source, attachment })
    }
  }
  placements.value = nextPlacements
}

function handleClick(event: MouseEvent): void {
  const link = (event.target as HTMLElement | null)?.closest<HTMLAnchorElement>('a[data-ai-source]')
  const source = props.sources.find((item) => item.message_id === link?.dataset.aiSource)
  if (!link || !source) return
  event.preventDefault()
  void router.push(aiSourceRoute(source)).catch(() => {})
}

watch([html, () => props.sources], async () => {
  placements.value = []
  await nextTick()
  decorateCitations()
})
onMounted(decorateCitations)
</script>

<template>
  <div ref="root" class="cr-markdown min-w-0 max-w-full break-words" @click="handleClick" v-html="html" />
  <Teleport v-for="placement in placements" :key="placement.targetId" :to="`#${placement.targetId}`">
    <figure class="my-2 min-w-0 max-w-[30rem]">
      <figcaption class="mb-1.5 flex items-center gap-1.5 text-[11px] text-muted-color">
        <span class="font-mono font-semibold text-primary">[{{ placement.source.label }}]</span>
        <span class="truncate">{{ placement.source.sender }}</span>
      </figcaption>
      <MessageAttachment
        :attachment="placement.attachment"
        class="w-full! max-w-full!"
        @preview-image="previewImageId = $event.id"
      />
    </figure>
  </Teleport>
  <ImageViewerGallery v-if="images.length" :images="images" :active-id="previewImageId" @close="previewImageId = ''" />
</template>

<style scoped>
.cr-markdown :deep(p),
.cr-markdown :deep(ul),
.cr-markdown :deep(ol),
.cr-markdown :deep(pre),
.cr-markdown :deep(blockquote) {
  margin: 0 0 0.65rem;
}

.cr-markdown :deep(.ai-inline-attachment-group) {
  display: grid;
  max-width: 30rem;
  gap: 0.25rem;
  margin: 0.25rem 0 0.8rem;
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
