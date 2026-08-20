<script setup lang="ts">
import {
  ArrowLeft,
  AtSign,
  Ban,
  Check,
  Clock3,
  EllipsisVertical,
  MessageCircle,
  Quote,
  RotateCcw,
  X,
} from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import { avatarColor } from '../avatarColor'
import type { ContactEntry } from '../contactDirectory'

const props = defineProps<{ entry: ContactEntry | null; busy: boolean }>()
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
  return entry.user.display_name || entry.user.username
}

function relationshipLabel(entry: ContactEntry): string {
  if (entry.kind === 'friend') return '好友'
  if (entry.kind === 'incoming') return '收到的好友申请'
  if (entry.kind === 'outgoing') return '好友申请已发送'
  return '已加入黑名单'
}
</script>

<template>
  <section class="flex h-full min-h-0 min-w-0 flex-col bg-surface-50">
    <Transition name="contact-profile" mode="out-in">
      <div v-if="entry" :key="entry.key" class="flex min-h-0 flex-1 flex-col">
        <header class="flex h-[72px] shrink-0 items-center gap-3 border-b border-surface-200 bg-surface-0 px-3 sm:px-5">
          <Button
            class="lg:hidden"
            text
            rounded
            severity="secondary"
            aria-label="返回联系人列表"
            title="返回"
            @click="emit('close')"
          >
            <ArrowLeft :size="20" aria-hidden="true" />
          </Button>
          <Avatar
            :label="entry.user.avatar_emoji || entry.user.username.slice(0, 1).toUpperCase()"
            shape="circle"
            class="size-10! shrink-0 text-white!"
            :style="{ backgroundColor: avatarColor(entry.user.id) }"
          />
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-sm font-semibold text-surface-900">{{ displayName(entry) }}</h2>
            <p class="mt-0.5 truncate text-xs text-muted-color">{{ relationshipLabel(entry) }}</p>
          </div>
          <Button
            v-if="entry.kind === 'friend'"
            text
            rounded
            severity="secondary"
            aria-label="更多好友操作"
            title="更多"
            @click="emit('menu', $event, entry)"
          >
            <EllipsisVertical :size="19" aria-hidden="true" />
          </Button>
        </header>

        <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
          <div class="relative h-28 bg-primary-50" aria-hidden="true" />
          <div class="relative mx-auto w-full max-w-xl px-5 pb-10 text-center">
            <Avatar
              :label="entry.user.avatar_emoji || entry.user.username.slice(0, 1).toUpperCase()"
              shape="circle"
              class="-mt-12 size-24! border-4 border-surface-50 text-3xl! text-white! shadow-md"
              :style="{ backgroundColor: avatarColor(entry.user.id) }"
            />
            <h3 class="mt-4 text-xl font-semibold text-surface-900 text-balance">{{ displayName(entry) }}</h3>
            <p class="mt-1 text-sm text-muted-color">@{{ entry.user.username }}</p>

            <div class="mt-6 flex min-h-[72px] items-start justify-center gap-7" aria-label="联系人操作">
              <button
                v-if="entry.kind === 'friend'"
                type="button"
                class="group/action flex w-16 flex-col items-center gap-2 text-xs font-medium text-primary focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
                :disabled="busy"
                @click="emit('message', entry.user.id)"
              >
                <span
                  class="grid size-11 place-items-center rounded-full bg-primary text-primary-contrast shadow-sm transition-transform duration-150 group-hover/action:-translate-y-0.5 group-active/action:scale-95 motion-reduce:transition-none"
                >
                  <MessageCircle :size="20" aria-hidden="true" />
                </span>
                <span>{{ busy ? '打开中…' : '发消息' }}</span>
              </button>

              <template v-else-if="entry.kind === 'incoming'">
                <button
                  type="button"
                  class="group/action flex w-16 flex-col items-center gap-2 text-xs font-medium text-primary focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
                  :disabled="busy"
                  @click="emit('accept', entry.user.id)"
                >
                  <span
                    class="grid size-11 place-items-center rounded-full bg-primary text-primary-contrast shadow-sm transition-transform duration-150 group-hover/action:-translate-y-0.5 group-active/action:scale-95 motion-reduce:transition-none"
                  >
                    <Check :size="21" aria-hidden="true" />
                  </span>
                  <span>{{ busy ? '处理中…' : '接受' }}</span>
                </button>
                <button
                  type="button"
                  class="group/action flex w-16 flex-col items-center gap-2 text-xs font-medium text-muted-color focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
                  :disabled="busy"
                  @click="emit('decline', entry.user.id)"
                >
                  <span
                    class="grid size-11 place-items-center rounded-full border border-surface-200 bg-surface-0 shadow-xs transition-transform duration-150 group-hover/action:-translate-y-0.5 group-active/action:scale-95 motion-reduce:transition-none"
                  >
                    <X :size="20" aria-hidden="true" />
                  </span>
                  <span>拒绝</span>
                </button>
              </template>

              <button
                v-else-if="entry.kind === 'outgoing'"
                type="button"
                class="group/action flex w-20 flex-col items-center gap-2 text-xs font-medium text-muted-color focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
                :disabled="busy"
                @click="emit('cancel', entry.user.id)"
              >
                <span
                  class="grid size-11 place-items-center rounded-full border border-surface-200 bg-surface-0 shadow-xs transition-transform duration-150 group-hover/action:-translate-y-0.5 group-active/action:scale-95 motion-reduce:transition-none"
                >
                  <Clock3 :size="20" aria-hidden="true" />
                </span>
                <span>{{ busy ? '处理中…' : '撤回申请' }}</span>
              </button>

              <button
                v-else
                type="button"
                class="group/action flex w-20 flex-col items-center gap-2 text-xs font-medium text-primary focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
                :disabled="busy"
                @click="emit('unblock', entry.user.id)"
              >
                <span
                  class="grid size-11 place-items-center rounded-full bg-primary text-primary-contrast shadow-sm transition-transform duration-150 group-hover/action:-translate-y-0.5 group-active/action:scale-95 motion-reduce:transition-none"
                >
                  <RotateCcw :size="19" aria-hidden="true" />
                </span>
                <span>{{ busy ? '处理中…' : '取消拉黑' }}</span>
              </button>
            </div>

            <dl class="mt-7 border-y border-surface-200 bg-surface-0 text-left">
              <div class="flex min-h-16 items-center gap-4 px-4">
                <AtSign :size="19" class="shrink-0 text-primary" aria-hidden="true" />
                <div class="min-w-0">
                  <dt class="text-xs text-muted-color">用户名</dt>
                  <dd class="mt-1 truncate text-sm font-medium">@{{ entry.user.username }}</dd>
                </div>
              </div>
              <div
                v-if="entry.subtitle && entry.kind !== 'incoming' && entry.kind !== 'outgoing'"
                class="flex min-h-16 items-center gap-4 border-t border-surface-100 px-4"
              >
                <Quote :size="19" class="shrink-0 text-primary" aria-hidden="true" />
                <div class="min-w-0">
                  <dt class="text-xs text-muted-color">个性签名</dt>
                  <dd class="mt-1 break-words text-sm font-medium">{{ entry.subtitle }}</dd>
                </div>
              </div>
              <div class="flex min-h-16 items-center gap-4 border-t border-surface-100 px-4">
                <Ban v-if="entry.kind === 'blocked'" :size="19" class="shrink-0 text-danger" aria-hidden="true" />
                <MessageCircle v-else :size="19" class="shrink-0 text-primary" aria-hidden="true" />
                <div class="min-w-0">
                  <dt class="text-xs text-muted-color">关系</dt>
                  <dd class="mt-1 text-sm font-medium">{{ relationshipLabel(entry) }}</dd>
                </div>
              </div>
            </dl>
          </div>
        </div>
      </div>

      <div v-else key="empty" class="grid min-h-0 flex-1 place-items-center bg-surface-50">
        <img src="/brand/echo-gate.svg" alt="" width="56" height="56" class="opacity-25" aria-hidden="true" />
      </div>
    </Transition>
  </section>
</template>

<style scoped>
.contact-profile-enter-active,
.contact-profile-leave-active {
  transition:
    opacity var(--cr-motion-normal) var(--cr-ease-out),
    transform var(--cr-motion-normal) var(--cr-ease-out);
}
.contact-profile-enter-from {
  opacity: 0;
  transform: translateX(10px);
}
.contact-profile-leave-to {
  opacity: 0;
  transform: translateX(-6px);
}
@media (prefers-reduced-motion: reduce) {
  .contact-profile-enter-active,
  .contact-profile-leave-active {
    transition: none;
  }
}
</style>
