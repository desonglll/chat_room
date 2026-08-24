<script setup lang="ts">
import {
  ArrowLeft,
  AtSign,
  Check,
  Clock3,
  Ellipsis,
  MessageCircle,
  Quote,
  RotateCcw,
  ShieldOff,
  UsersRound,
  X,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import Divider from 'primevue/divider'
import Tag from 'primevue/tag'
import type { ContactEntry } from '../contactDirectory'
import AppAvatar from './AppAvatar.vue'

defineProps<{ entry: ContactEntry | null; busy: boolean }>()
const emit = defineEmits<{
  close: []
  message: [userId: string]
  accept: [userId: string]
  decline: [userId: string]
  cancel: [userId: string]
  unblock: [userId: string]
  menu: [event: Event, entry: ContactEntry]
}>()

function displayName(entry: ContactEntry): string {
  return entry.displayName
}

function relationshipLabel(entry: ContactEntry): string {
  if (entry.kind === 'friend') return '好友'
  if (entry.kind === 'incoming') return '等待你处理'
  if (entry.kind === 'outgoing') return '等待对方接受'
  return '已拉黑'
}

function relationshipSeverity(entry: ContactEntry): 'success' | 'warn' | 'info' | 'danger' {
  if (entry.kind === 'friend') return 'success'
  if (entry.kind === 'incoming') return 'warn'
  if (entry.kind === 'outgoing') return 'info'
  return 'danger'
}
</script>

<template>
  <aside class="cr-contact-inspector flex h-full min-h-0 min-w-0 flex-col">
    <header class="cr-inspector-header flex h-14 shrink-0 items-center gap-2 px-3">
      <Button
        text
        rounded
        severity="secondary"
        class="size-11! touch-manipulation xl:hidden"
        aria-label="返回联系人列表"
        title="返回"
        @click="emit('close')"
      >
        <ArrowLeft :size="20" aria-hidden="true" />
      </Button>
      <h2 class="min-w-0 flex-1 truncate text-sm font-semibold text-surface-900">详细资料</h2>
      <Button
        v-if="entry?.kind === 'friend'"
        text
        rounded
        severity="secondary"
        class="size-11! touch-manipulation xl:size-10!"
        aria-label="更多好友操作"
        title="更多"
        @click="emit('menu', $event, entry)"
      >
        <Ellipsis :size="18" aria-hidden="true" />
      </Button>
    </header>

    <Transition name="contact-inspector" mode="out-in">
      <div
        v-if="entry"
        :key="entry.key"
        class="cr-inspector-body min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-5"
      >
        <div class="flex min-w-0 items-center gap-4">
          <AppAvatar
            :avatar="entry.user.avatar_emoji"
            :fallback="entry.user.username"
            :color-key="entry.user.id"
            class="size-16! shrink-0 text-xl! text-white!"
          />
          <div class="min-w-0 flex-1">
            <h3 class="break-words text-lg font-semibold leading-tight text-surface-900">{{ displayName(entry) }}</h3>
            <p class="mt-1 truncate text-sm text-muted-color" translate="no">@{{ entry.user.username }}</p>
            <Tag class="mt-2" :value="relationshipLabel(entry)" :severity="relationshipSeverity(entry)" />
          </div>
        </div>

        <div class="mt-6 flex gap-2" aria-label="联系人操作">
          <Button
            v-if="entry.kind === 'friend'"
            class="flex-1"
            size="small"
            label="发消息"
            :loading="busy"
            @click="emit('message', entry.user.id)"
          >
            <template #icon><MessageCircle :size="17" aria-hidden="true" /></template>
          </Button>

          <template v-else-if="entry.kind === 'incoming'">
            <Button class="flex-1" size="small" label="接受" :loading="busy" @click="emit('accept', entry.user.id)">
              <template #icon><Check :size="17" aria-hidden="true" /></template>
            </Button>
            <Button
              class="flex-1"
              size="small"
              label="拒绝"
              severity="secondary"
              outlined
              :disabled="busy"
              @click="emit('decline', entry.user.id)"
            >
              <template #icon><X :size="17" aria-hidden="true" /></template>
            </Button>
          </template>

          <Button
            v-else-if="entry.kind === 'outgoing'"
            class="flex-1"
            size="small"
            label="撤回申请"
            severity="secondary"
            outlined
            :loading="busy"
            @click="emit('cancel', entry.user.id)"
          >
            <template #icon><Clock3 :size="17" aria-hidden="true" /></template>
          </Button>

          <Button
            v-else
            class="flex-1"
            size="small"
            label="解除拉黑"
            :loading="busy"
            @click="emit('unblock', entry.user.id)"
          >
            <template #icon><RotateCcw :size="17" aria-hidden="true" /></template>
          </Button>
        </div>

        <Divider />

        <dl class="space-y-5">
          <div
            v-if="entry.kind === 'friend' && displayName(entry) !== (entry.user.display_name || entry.user.username)"
            class="flex min-w-0 gap-3"
          >
            <Quote :size="18" class="mt-0.5 shrink-0 text-muted-color" aria-hidden="true" />
            <div class="min-w-0">
              <dt class="text-xs font-medium text-muted-color">原名称</dt>
              <dd class="mt-1 break-words text-sm text-surface-900">
                {{ entry.user.display_name || entry.user.username }}
              </dd>
            </div>
          </div>
          <div class="flex min-w-0 gap-3">
            <AtSign :size="18" class="mt-0.5 shrink-0 text-muted-color" aria-hidden="true" />
            <div class="min-w-0">
              <dt class="text-xs font-medium text-muted-color">用户名</dt>
              <dd class="mt-1 break-all text-sm font-medium text-surface-900" translate="no">
                @{{ entry.user.username }}
              </dd>
            </div>
          </div>

          <div v-if="entry.subtitle && entry.kind === 'friend'" class="flex min-w-0 gap-3">
            <Quote :size="18" class="mt-0.5 shrink-0 text-muted-color" aria-hidden="true" />
            <div class="min-w-0">
              <dt class="text-xs font-medium text-muted-color">个性签名</dt>
              <dd class="mt-1 break-words text-sm leading-6 text-surface-800">{{ entry.subtitle }}</dd>
            </div>
          </div>

          <div class="flex min-w-0 gap-3">
            <ShieldOff
              v-if="entry.kind === 'blocked'"
              :size="18"
              class="mt-0.5 shrink-0 text-danger"
              aria-hidden="true"
            />
            <UsersRound v-else :size="18" class="mt-0.5 shrink-0 text-muted-color" aria-hidden="true" />
            <div class="min-w-0">
              <dt class="text-xs font-medium text-muted-color">关系状态</dt>
              <dd class="mt-1 text-sm text-surface-800">{{ relationshipLabel(entry) }}</dd>
            </div>
          </div>
        </dl>
      </div>

      <div v-else key="empty" class="grid min-h-0 flex-1 place-items-center">
        <img src="/brand/echo-gate.svg" alt="" width="44" height="44" class="opacity-20" aria-hidden="true" />
      </div>
    </Transition>
  </aside>
</template>

<style scoped>
.contact-inspector-enter-active,
.contact-inspector-leave-active {
  transition:
    opacity var(--cr-motion-normal) var(--cr-ease-out),
    transform var(--cr-motion-normal) var(--cr-ease-out);
}
.contact-inspector-enter-from {
  opacity: 0;
  transform: translateX(8px);
}
.contact-inspector-leave-to {
  opacity: 0;
  transform: translateX(-4px);
}
@media (prefers-reduced-motion: reduce) {
  .contact-inspector-enter-active,
  .contact-inspector-leave-active {
    transition: none;
  }
}
</style>
