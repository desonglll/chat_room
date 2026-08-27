<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { ArrowDown, Bot } from 'lucide-vue-next'
import type { AiUiMessage } from '../aiUi'
import type { FavoriteItem } from '../types'
import { isViewportNearBottom } from '../messageViewportPolicy'
import AiAssistantMessage from './AiAssistantMessage.vue'

defineProps<{
  messages: AiUiMessage[]
  roomTitle: string
  saveFavorite: (title: string, content: string) => Promise<FavoriteItem>
}>()
const emit = defineEmits<{ sources: [message: AiUiMessage] }>()

const viewport = ref<HTMLElement | null>(null)
const now = ref(Date.now())
const followingLatest = ref(true)
const awayFromLatest = ref(false)
let pendingFrame: number | null = null
let elapsedTimer: ReturnType<typeof setInterval> | null = null
let programmaticScrollTimer: ReturnType<typeof setTimeout> | null = null
let touchStartY: number | null = null
let lastScrollTop = 0

onMounted(() => {
  elapsedTimer = setInterval(() => (now.value = Date.now()), 1000)
})

onUnmounted(() => {
  if (elapsedTimer !== null) clearInterval(elapsedTimer)
  if (programmaticScrollTimer !== null) clearTimeout(programmaticScrollTimer)
  if (pendingFrame !== null) cancelAnimationFrame(pendingFrame)
})

function updateViewportPosition(): void {
  const element = viewport.value
  if (!element) return
  const nearLatest = isViewportNearBottom(element)
  const movedUp = element.scrollTop < lastScrollTop - 1
  lastScrollTop = element.scrollTop
  if (programmaticScrollTimer !== null) {
    if (nearLatest) awayFromLatest.value = false
    return
  }
  awayFromLatest.value = !nearLatest
  if (movedUp) followingLatest.value = false
  else if (nearLatest) followingLatest.value = true
}

function pauseFollowing(): void {
  const element = viewport.value
  if (!element || element.scrollHeight <= element.clientHeight + 1) return
  followingLatest.value = false
  awayFromLatest.value = true
  if (pendingFrame !== null) {
    cancelAnimationFrame(pendingFrame)
    pendingFrame = null
  }
}

function handleWheel(event: WheelEvent): void {
  if (event.deltaY < 0) pauseFollowing()
}

function handleTouchStart(event: TouchEvent): void {
  touchStartY = event.touches[0]?.clientY ?? null
}

function handleTouchMove(event: TouchEvent): void {
  const currentY = event.touches[0]?.clientY
  if (touchStartY !== null && currentY !== undefined && currentY > touchStartY + 4) pauseFollowing()
}

async function scrollToLatest(smooth = false): Promise<void> {
  await nextTick()
  if (!viewport.value) return
  followingLatest.value = true
  awayFromLatest.value = false
  viewport.value.scrollTo({
    top: viewport.value.scrollHeight,
    behavior: smooth ? 'smooth' : 'auto',
  })
  if (programmaticScrollTimer !== null) clearTimeout(programmaticScrollTimer)
  programmaticScrollTimer = setTimeout(
    () => {
      programmaticScrollTimer = null
      updateViewportPosition()
    },
    smooth ? 450 : 0,
  )
}

function scrollToLatestSoon(): void {
  if (pendingFrame !== null) return
  pendingFrame = requestAnimationFrame(async () => {
    pendingFrame = null
    await nextTick()
    if (followingLatest.value) await scrollToLatest()
    else awayFromLatest.value = true
  })
}

defineExpose({ scrollToLatest, scrollToLatestSoon })
</script>

<template>
  <div class="relative min-h-0">
    <div
      ref="viewport"
      class="h-full min-h-0 overflow-y-auto px-3 py-4 sm:px-7 sm:py-6"
      aria-live="polite"
      @scroll.passive="updateViewportPosition"
      @wheel.passive="handleWheel"
      @touchstart.passive="handleTouchStart"
      @touchmove.passive="handleTouchMove"
    >
      <div v-if="!messages.length" class="grid min-h-full place-items-center text-center text-muted-color">
        <div>
          <Bot :size="32" class="mx-auto opacity-35" />
          <p class="mt-3 text-sm">可以直接提问，也可以输入 @ 引用一个聊天会话</p>
        </div>
      </div>
      <TransitionGroup v-else tag="ol" name="ai-message" class="mx-auto w-full max-w-3xl space-y-4 sm:space-y-6">
        <li
          v-for="message in messages"
          :key="message.id"
          class="flex"
          :class="message.role === 'user' ? 'justify-end' : 'justify-start'"
        >
          <article
            v-if="message.role === 'user'"
            class="min-w-0 max-w-[88%] rounded-md bg-primary px-3.5 py-2.5 text-sm leading-6 text-primary-contrast sm:max-w-[82%]"
          >
            <p class="whitespace-pre-wrap break-words">{{ message.content }}</p>
          </article>
          <AiAssistantMessage
            v-else
            :message="message"
            :room-title="roomTitle"
            :now="now"
            :save-favorite="saveFavorite"
            @sources="emit('sources', message)"
          />
        </li>
      </TransitionGroup>
    </div>

    <button
      v-if="awayFromLatest"
      type="button"
      title="回到最新消息"
      aria-label="回到最新消息"
      class="absolute bottom-3 left-1/2 z-10 grid size-9 -translate-x-1/2 place-items-center rounded-full border border-surface-200 bg-surface-0 text-surface-700 shadow-md transition-colors hover:bg-surface-100 focus-visible:outline-2 focus-visible:outline-primary"
      @click="scrollToLatest(true)"
    >
      <ArrowDown :size="17" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.ai-message-enter-active {
  transition:
    opacity var(--cr-motion-enter) var(--cr-ease-out),
    transform var(--cr-motion-enter) var(--cr-ease-out);
}

.ai-message-enter-from {
  opacity: 0;
  transform: translateY(4px);
}

@media (prefers-reduced-motion: reduce) {
  .ai-message-enter-active {
    transition: none;
  }
}
</style>
