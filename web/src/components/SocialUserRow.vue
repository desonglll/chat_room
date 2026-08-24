<script setup lang="ts">
import type { UserSummary } from '../types'
import AppAvatar from './AppAvatar.vue'

defineProps<{ user: UserSummary; subtitle?: string }>()
</script>

<template>
  <div
    class="group flex min-h-[72px] items-center gap-3 px-3 py-2 transition-colors duration-[var(--cr-motion-normal)] [transition-timing-function:ease] hover:bg-surface-50 motion-reduce:transition-none"
  >
    <AppAvatar
      :avatar="user.avatar_emoji"
      :fallback="user.username"
      :color-key="user.id"
      class="size-12! shrink-0 text-white!"
    />
    <div class="min-w-0 flex-1">
      <strong class="block truncate text-sm text-surface-900">{{ user.display_name || user.username }}</strong>
      <p class="mt-1 truncate text-xs text-muted-color">
        <span>@{{ user.username }}</span
        ><span v-if="subtitle"> · {{ subtitle }}</span>
      </p>
    </div>
    <div class="flex shrink-0 flex-wrap items-center justify-end gap-1"><slot /></div>
  </div>
</template>
