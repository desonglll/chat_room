<script setup lang="ts">
import { RefreshCw, WifiOff } from 'lucide-vue-next'
import Button from 'primevue/button'
import { usePwa } from '../pwa'

const { online, updateAvailable, applyUpdate } = usePwa()
</script>

<template>
  <Transition name="pwa-status">
    <aside v-if="!online || updateAvailable" class="cr-pwa-status" role="status" aria-live="polite">
      <div v-if="!online" class="flex min-w-0 items-center gap-2">
        <WifiOff :size="16" aria-hidden="true" />
        <span>离线 · 显示已加载内容</span>
      </div>
      <Button v-if="updateAvailable" size="small" severity="secondary" @click="applyUpdate">
        <RefreshCw :size="15" />更新
      </Button>
    </aside>
  </Transition>
</template>

<style scoped>
.cr-pwa-status {
  position: fixed;
  top: max(0.75rem, env(safe-area-inset-top));
  right: 0.75rem;
  z-index: 49;
  display: flex;
  min-height: 2.5rem;
  max-width: min(24rem, calc(100vw - 1.5rem));
  align-items: center;
  gap: 0.75rem;
  border: 1px solid var(--cr-border);
  border-radius: var(--cr-radius-md);
  background: color-mix(in srgb, var(--cr-surface) 96%, transparent);
  padding: 0.4rem 0.5rem 0.4rem 0.75rem;
  color: var(--cr-text);
  box-shadow: var(--cr-shadow-md);
  font-size: 0.8125rem;
  font-weight: 600;
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
}

.pwa-status-enter-active,
.pwa-status-leave-active {
  transition:
    opacity var(--cr-motion-normal) ease,
    transform var(--cr-motion-normal) var(--cr-ease-out);
}

.pwa-status-enter-from,
.pwa-status-leave-to {
  opacity: 0;
  transform: translateY(-0.5rem);
}

@media (max-width: 767px) {
  .cr-pwa-status {
    left: 0.75rem;
    right: auto;
  }
}

@media (prefers-reduced-motion: reduce) {
  .pwa-status-enter-active,
  .pwa-status-leave-active {
    transition: opacity var(--cr-motion-fast) ease;
  }

  .pwa-status-enter-from,
  .pwa-status-leave-to {
    transform: none;
  }
}
</style>
