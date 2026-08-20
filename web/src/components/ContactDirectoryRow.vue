<script setup lang="ts">
import { Ban, Clock3, UserRoundPlus } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import { avatarColor } from '../avatarColor'
import type { ContactEntry } from '../contactDirectory'

defineProps<{ entry: ContactEntry; selected: boolean }>()
const emit = defineEmits<{ select: [] }>()
</script>

<template>
  <button
    type="button"
    class="contact-row group flex h-[72px] w-full min-w-0 items-center gap-3 px-3 text-left transition-[background-color,transform] duration-150 ease-out focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset active:scale-[0.985] motion-reduce:transition-none"
    :class="selected ? 'bg-primary-50' : 'hover:bg-surface-50'"
    :aria-current="selected ? 'true' : undefined"
    @click="emit('select')"
  >
    <Avatar
      :label="entry.user.avatar_emoji || entry.user.username.slice(0, 1).toUpperCase()"
      shape="circle"
      class="size-12! shrink-0 text-white! shadow-xs"
      :style="{ backgroundColor: avatarColor(entry.user.id) }"
    />
    <span class="min-w-0 flex-1">
      <strong class="block truncate text-sm font-semibold text-surface-900">
        {{ entry.user.display_name || entry.user.username }}
      </strong>
      <span class="mt-1 block truncate text-xs" :class="selected ? 'text-primary-700' : 'text-muted-color'">
        {{ entry.subtitle || `@${entry.user.username}` }}
      </span>
    </span>
    <span
      v-if="entry.kind === 'incoming'"
      class="grid size-7 shrink-0 place-items-center rounded-full bg-primary text-primary-contrast"
      title="待处理申请"
      aria-label="待处理申请"
    >
      <UserRoundPlus :size="15" aria-hidden="true" />
    </span>
    <Clock3 v-else-if="entry.kind === 'outgoing'" :size="16" class="shrink-0 text-muted-color" aria-hidden="true" />
    <Ban v-else-if="entry.kind === 'blocked'" :size="16" class="shrink-0 text-danger" aria-hidden="true" />
  </button>
</template>

<style scoped>
.contact-row {
  content-visibility: auto;
  contain-intrinsic-size: auto 72px;
}
</style>
