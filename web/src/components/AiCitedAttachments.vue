<script setup lang="ts">
import { computed, defineAsyncComponent, ref } from 'vue'
import { Paperclip } from 'lucide-vue-next'
import { trailingAiAttachments } from '../aiUi'
import type { AiCitationSource, Attachment } from '../types'
import MessageAttachment from './MessageAttachment.vue'

const ImageViewerGallery = defineAsyncComponent(() => import('./ImageViewerGallery.vue'))
const props = defineProps<{ content: string; sources: AiCitationSource[] }>()
const previewImageId = ref('')

const referenced = computed(() => trailingAiAttachments(props.content, props.sources))
const images = computed(() =>
  referenced.value
    .map((source) => source.attachment)
    .filter((attachment): attachment is Attachment => Boolean(attachment?.mime_type.startsWith('image/'))),
)
</script>

<template>
  <section v-if="referenced.length" class="mt-3 border-t border-surface-200 pt-3" aria-label="回答补充附件">
    <div class="mb-2 flex items-center gap-2 text-xs font-medium text-surface-700">
      <Paperclip :size="14" class="text-primary" aria-hidden="true" />
      <span>补充附件</span>
      <span class="font-normal text-muted-color">{{ referenced.length }} 个</span>
    </div>
    <div class="grid gap-3 sm:grid-cols-2">
      <div v-for="source in referenced" :key="source.label" class="min-w-0">
        <div class="mb-1 flex items-center gap-1.5 text-[10px] text-muted-color">
          <span class="font-mono font-semibold text-primary">[{{ source.label }}]</span>
          <span class="truncate">{{ source.sender }}</span>
        </div>
        <MessageAttachment
          v-if="source.attachment"
          :attachment="source.attachment"
          class="w-full! max-w-full!"
          @preview-image="previewImageId = $event.id"
        />
      </div>
    </div>
    <ImageViewerGallery
      v-if="images.length"
      :images="images"
      :active-id="previewImageId"
      @close="previewImageId = ''"
    />
  </section>
</template>
