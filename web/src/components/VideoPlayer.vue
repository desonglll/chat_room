<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import Plyr from 'plyr'
import 'plyr/dist/plyr.css'

defineProps<{
  src: string
  mimeType: string
}>()
const emit = defineEmits<{ error: [] }>()

const video = ref<HTMLVideoElement | null>(null)
let player: Plyr | null = null

onMounted(() => {
  if (!video.value) return
  player = new Plyr(video.value, {
    controls: [
      'play-large',
      'play',
      'progress',
      'current-time',
      'mute',
      'volume',
      'settings',
      'pip',
      'fullscreen',
    ],
    ratio: '16:9',
    storage: { enabled: true, key: 'chat-room.player' },
  })
})

onBeforeUnmount(() => {
  player?.destroy()
  player = null
})
</script>

<template>
  <div class="max-h-[min(52vh,420px)] overflow-hidden bg-black [--plyr-color-main:var(--p-primary-color)]">
    <video ref="video" playsinline preload="metadata" @error="emit('error')">
      <source :src="src" :type="mimeType">
    </video>
  </div>
</template>
