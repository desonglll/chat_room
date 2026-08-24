<script setup lang="ts">
import { Check, Clock3, Ellipsis, MessageCircle, RotateCcw, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import type { ContactEntry } from '../contactDirectory'
import AppAvatar from './AppAvatar.vue'

defineProps<{ entry: ContactEntry; selected: boolean; busy: boolean }>()
const emit = defineEmits<{
  select: []
  message: []
  accept: []
  decline: []
  cancel: []
  unblock: []
  menu: [event: Event]
}>()
</script>

<template>
  <li
    class="contact-row cr-contact-row group relative flex min-h-[72px] min-w-0 items-center rounded-md"
    :class="selected ? 'cr-contact-row--active' : 'cr-contact-row--idle'"
  >
    <span
      class="absolute inset-y-3 left-0 w-0.5 origin-center rounded-full bg-primary transition-transform duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
      :class="selected ? 'scale-y-100' : 'scale-y-0'"
      aria-hidden="true"
    />
    <button
      type="button"
      class="flex min-w-0 flex-1 touch-manipulation items-center gap-3 self-stretch rounded-md px-3 text-left focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset sm:px-4"
      :aria-current="selected ? 'true' : undefined"
      @click="emit('select')"
    >
      <AppAvatar
        :avatar="entry.user.avatar_emoji"
        :fallback="entry.user.username"
        :color-key="entry.user.id"
        class="size-11! shrink-0 text-white!"
      />
      <span class="min-w-0 flex-1">
        <span class="flex min-w-0 items-center gap-2">
          <strong class="truncate text-sm font-semibold text-surface-900">
            {{ entry.displayName }}
          </strong>
          <span
            v-if="entry.kind === 'incoming'"
            class="shrink-0 rounded-sm bg-primary-50 px-1.5 py-0.5 text-[10px] font-semibold text-primary-800"
          >
            待处理
          </span>
        </span>
        <span class="mt-1 block truncate text-xs text-muted-color">
          {{ entry.subtitle || `@${entry.user.username}` }}
        </span>
      </span>
    </button>

    <div
      class="contact-actions mr-2 flex shrink-0 items-center gap-0.5 transition-[opacity,transform] duration-[var(--cr-motion-normal)] [transition-timing-function:var(--cr-ease-out)] motion-reduce:transition-none"
      aria-label="联系人快捷操作"
    >
      <template v-if="entry.kind === 'friend'">
        <Button
          text
          rounded
          severity="secondary"
          class="size-11! touch-manipulation sm:size-10!"
          aria-label="发消息"
          title="发消息"
          :loading="busy"
          @click="emit('message')"
        >
          <MessageCircle :size="18" aria-hidden="true" />
        </Button>
        <Button
          text
          rounded
          severity="secondary"
          class="size-11! touch-manipulation sm:size-10!"
          aria-label="更多好友操作"
          title="更多"
          @click="emit('menu', $event)"
        >
          <Ellipsis :size="18" aria-hidden="true" />
        </Button>
      </template>

      <template v-else-if="entry.kind === 'incoming'">
        <Button
          text
          rounded
          class="size-11! touch-manipulation sm:size-10!"
          aria-label="接受好友申请"
          title="接受"
          :loading="busy"
          @click="emit('accept')"
        >
          <Check :size="19" aria-hidden="true" />
        </Button>
        <Button
          text
          rounded
          severity="secondary"
          class="size-11! touch-manipulation sm:size-10!"
          aria-label="拒绝好友申请"
          title="拒绝"
          :disabled="busy"
          @click="emit('decline')"
        >
          <X :size="19" aria-hidden="true" />
        </Button>
      </template>

      <Button
        v-else-if="entry.kind === 'outgoing'"
        text
        rounded
        severity="secondary"
        class="size-11! touch-manipulation sm:size-10!"
        aria-label="撤回好友申请"
        title="撤回申请"
        :loading="busy"
        @click="emit('cancel')"
      >
        <Clock3 :size="18" aria-hidden="true" />
      </Button>

      <Button
        v-else
        text
        rounded
        class="size-11! touch-manipulation sm:size-10!"
        aria-label="解除拉黑"
        title="解除拉黑"
        :loading="busy"
        @click="emit('unblock')"
      >
        <RotateCcw :size="18" aria-hidden="true" />
      </Button>
    </div>
  </li>
</template>

<style scoped>
.contact-row {
  content-visibility: auto;
  contain-intrinsic-size: auto 72px;
}

@media (hover: hover) and (pointer: fine) {
  .contact-actions {
    opacity: 0;
    transform: translateX(4px);
  }

  .contact-row:hover .contact-actions,
  .contact-row:focus-within .contact-actions {
    opacity: 1;
    transform: translateX(0);
  }
}
</style>
