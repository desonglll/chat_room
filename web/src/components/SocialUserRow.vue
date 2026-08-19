<script setup lang="ts">
import Avatar from 'primevue/avatar'
import { avatarColor } from '../avatarColor'
import type { UserSummary } from '../types'

defineProps<{ user: UserSummary; subtitle?: string }>()
</script>

<template>
  <div class="flex min-h-16 items-center gap-3 border-b border-surface-100 px-1 py-2 last:border-b-0">
    <Avatar
      :label="user.avatar_emoji || user.username.slice(0, 1).toUpperCase()"
      shape="circle"
      class="size-11! shrink-0 text-white!"
      :style="{ backgroundColor: avatarColor(user.id) }"
    />
    <div class="min-w-0 flex-1">
      <strong class="block truncate text-sm">{{ user.display_name || user.username }}</strong>
      <span class="mt-0.5 block truncate text-xs text-muted-color">{{ subtitle || `@${user.username}` }}</span>
    </div>
    <div class="flex shrink-0 items-center gap-1"><slot /></div>
  </div>
</template>
