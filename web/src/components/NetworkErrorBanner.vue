<script setup lang="ts">
import Button from 'primevue/button'
import Message from 'primevue/message'

defineProps<{ message: string }>()
defineEmits<{ retry: [] }>()
</script>

<template>
  <Transition name="network-banner">
    <div v-if="message" class="cr-network-banner" role="alert">
      <Message severity="error" :closable="false">
        <div class="flex items-center gap-3">
          <span class="min-w-0 flex-1">{{ message }}</span>
          <Button label="重试" size="small" severity="danger" outlined @click="$emit('retry')" />
        </div>
      </Message>
    </div>
  </Transition>
</template>

<style scoped>
.cr-network-banner {
  position: fixed;
  top: max(0.75rem, env(safe-area-inset-top));
  left: 50%;
  z-index: 50;
  width: min(calc(100vw - 1.5rem), 28rem);
  transform: translateX(-50%);
}

.cr-network-banner :deep(.p-message) {
  margin: 0;
  border-color: color-mix(in srgb, var(--cr-danger) 28%, var(--cr-border));
  background: color-mix(in srgb, var(--cr-surface) 94%, transparent);
  box-shadow: var(--cr-shadow-lg);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
}

.network-banner-enter-active {
  transition:
    opacity var(--cr-motion-slow) ease,
    transform var(--cr-motion-slow) ease;
}

.network-banner-leave-active {
  transition:
    opacity var(--cr-motion-normal) var(--cr-ease-out),
    transform var(--cr-motion-normal) var(--cr-ease-out);
}

.network-banner-enter-from,
.network-banner-leave-to {
  opacity: 0;
  transform: translate(-50%, -100%) scale(0.98);
}

@media (max-width: 767px) {
  .cr-network-banner {
    top: auto;
    bottom: calc(4.5rem + env(safe-area-inset-bottom));
  }

  .network-banner-enter-from,
  .network-banner-leave-to {
    transform: translate(-50%, 100%) scale(0.98);
  }
}

@media (prefers-reduced-motion: reduce) {
  .network-banner-enter-active,
  .network-banner-leave-active {
    transition: opacity var(--cr-motion-fast) ease;
  }

  .network-banner-enter-from,
  .network-banner-leave-to {
    transform: translateX(-50%);
  }
}
</style>
