<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import Viewer from 'viewerjs'
import 'viewerjs/dist/viewer.css'
import type { Attachment } from '../types'

const props = defineProps<{
  images: Attachment[]
  activeId: string
}>()
const emit = defineEmits<{ close: [] }>()

const gallery = ref<HTMLElement | null>(null)
let viewer: Viewer | null = null

function createViewer(): void {
  if (!gallery.value) return
  viewer?.destroy()
  viewer = new Viewer(gallery.value, {
    backdrop: true,
    button: true,
    focus: true,
    fullscreen: true,
    keyboard: true,
    loop: true,
    movable: true,
    navbar: true,
    rotatable: true,
    scalable: true,
    title: true,
    toolbar: {
      zoomIn: 1,
      zoomOut: 1,
      oneToOne: 1,
      reset: 1,
      prev: 1,
      play: 1,
      next: 1,
      rotateLeft: 1,
      rotateRight: 1,
      flipHorizontal: 1,
      flipVertical: 1,
    },
    hidden: () => emit('close'),
  })
}

async function showActive(): Promise<void> {
  if (!props.activeId) {
    viewer?.hide()
    return
  }
  await nextTick()
  createViewer()
  const index = props.images.findIndex((image) => image.id === props.activeId)
  if (index >= 0) {
    viewer?.view(index)
    viewer?.show()
  }
}

watch(() => props.activeId, showActive)
watch(() => props.images.map((image) => image.id).join(','), () => {
  if (props.activeId) void showActive()
})
onMounted(() => { if (props.activeId) void showActive() })
onBeforeUnmount(() => viewer?.destroy())
</script>

<template>
  <div ref="gallery" class="hidden" aria-hidden="true">
    <img v-for="image in images" :key="image.id" :src="image.download_url" :alt="image.file_name">
  </div>
</template>
