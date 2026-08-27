<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import {
  ArrowLeft,
  AtSign,
  Bell,
  Bot,
  Check,
  CheckCheck,
  LoaderCircle,
  MessageCircleReply,
  UserPlus,
  UsersRound,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Select from 'primevue/select'
import Skeleton from 'primevue/skeleton'
import { useNotifications } from '../composables/useNotifications'
import type { NotificationItem, NotificationKind } from '../notificationsApi'
import AppAvatar from './AppAvatar.vue'

const props = defineProps<{ token: string }>()
const emit = defineEmits<{ back: [] }>()
const router = useRouter()
const {
  items,
  kind,
  nextCursor,
  unreadCount,
  loading,
  loadingMore,
  mutating,
  error,
  refresh,
  selectKind,
  loadMore,
  markRead,
  markAllRead,
} = useNotifications(() => props.token)

const kindOptions: Array<{ label: string; value: NotificationKind | '' }> = [
  { label: '全部类型', value: '' },
  { label: '好友请求', value: 'friend_request' },
  { label: '入群申请', value: 'room_join_request' },
  { label: '@提及', value: 'mention' },
  { label: '回复', value: 'reply' },
  { label: 'AI 运行', value: 'ai_run_completed' },
]
const unreadLabel = computed(() => (unreadCount.value > 99 ? '99+' : String(unreadCount.value)))

function kindLabel(value: NotificationKind): string {
  return kindOptions.find((option) => option.value === value)?.label || '通知'
}

function kindIcon(value: NotificationKind) {
  if (value === 'friend_request') return UserPlus
  if (value === 'room_join_request') return UsersRound
  if (value === 'mention') return AtSign
  if (value === 'reply') return MessageCircleReply
  return Bot
}

function formatTime(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime())
    ? ''
    : new Intl.DateTimeFormat('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }).format(
        date,
      )
}

async function openSource(item: NotificationItem): Promise<void> {
  if (!item.source_available) return
  await markRead(item)
  if (item.kind === 'friend_request') {
    void router.push({ name: 'contacts' })
    return
  }
  if (item.kind === 'ai_run_completed') {
    void router.push({ name: 'assistant' })
    return
  }
  if (!item.room_id) return
  if (item.message_id) {
    void router.push({
      name: 'room',
      params: { id: item.room_id },
      query: { message: item.message_id },
      hash: `#message-${item.message_id}`,
    })
  } else void router.push({ name: 'room', params: { id: item.room_id } })
}
</script>

<template>
  <main id="workspace-main" class="h-full min-h-0 overflow-y-auto bg-surface-0 dark:bg-surface-950">
    <div class="mx-auto flex min-h-full w-full max-w-5xl flex-col">
      <header
        class="sticky top-0 z-10 border-b border-surface-200 bg-surface-0/95 px-4 py-3 backdrop-blur dark:border-surface-800 dark:bg-surface-950/95 sm:px-6"
      >
        <div class="flex min-h-10 items-center gap-3">
          <Button
            text
            rounded
            severity="secondary"
            class="size-9! shrink-0 p-0!"
            aria-label="返回消息"
            @click="emit('back')"
          >
            <ArrowLeft :size="19" aria-hidden="true" />
          </Button>
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <h1 class="text-lg font-semibold text-color">通知</h1>
              <span
                v-if="unreadCount"
                class="inline-flex min-w-5 items-center justify-center rounded-full bg-red-500 px-1.5 text-[11px] font-semibold leading-5 text-white"
                :aria-label="`${unreadCount} 条未读通知`"
                >{{ unreadLabel }}</span
              >
            </div>
          </div>
          <Button
            size="small"
            severity="secondary"
            outlined
            :disabled="!unreadCount || mutating"
            aria-label="全部标记已读"
            @click="markAllRead"
          >
            <CheckCheck :size="16" aria-hidden="true" /><span class="hidden sm:inline">全部已读</span>
          </Button>
        </div>
        <div class="mt-3 flex items-center gap-2">
          <Select
            :model-value="kind"
            :options="kindOptions"
            option-label="label"
            option-value="value"
            size="small"
            class="w-44"
            aria-label="筛选通知类型"
            @update:model-value="selectKind($event as NotificationKind | '')"
          />
          <Button
            text
            rounded
            severity="secondary"
            class="size-9! p-0!"
            aria-label="刷新通知"
            title="刷新"
            :disabled="loading"
            @click="refresh"
          >
            <LoaderCircle v-if="loading" :size="17" class="animate-spin" aria-hidden="true" />
            <Bell v-else :size="17" aria-hidden="true" />
          </Button>
        </div>
      </header>

      <Message v-if="error" severity="error" :closable="false" class="mx-4 mt-4 sm:mx-6">
        <div class="flex items-center justify-between gap-3">
          <span>{{ error }}</span
          ><Button size="small" text label="重试" @click="refresh" />
        </div>
      </Message>

      <section aria-live="polite" :aria-busy="loading" class="flex-1 px-4 pb-8 sm:px-6">
        <div v-if="loading && !items.length" class="divide-y divide-surface-200 dark:divide-surface-800">
          <div v-for="index in 6" :key="index" class="flex min-h-24 items-center gap-3 py-4">
            <Skeleton shape="circle" size="2.5rem" />
            <div class="flex-1 space-y-2"><Skeleton width="32%" height="0.8rem" /><Skeleton width="72%" /></div>
          </div>
        </div>
        <div v-else-if="!items.length" class="grid min-h-80 place-items-center text-center text-muted-color">
          <div>
            <Bell :size="34" class="mx-auto mb-3 opacity-35" aria-hidden="true" />
            <p class="font-medium">暂无通知</p>
          </div>
        </div>
        <ol v-else class="divide-y divide-surface-200 dark:divide-surface-800">
          <li v-for="item in items" :key="item.id" class="group relative flex min-h-24 gap-3 py-4 sm:gap-4">
            <span
              v-if="!item.read_at"
              class="absolute left-0 top-1/2 size-2 -translate-x-3 -translate-y-1/2 rounded-full bg-primary"
            />
            <AppAvatar
              v-if="item.actor"
              :avatar="item.actor.avatar_emoji"
              :fallback="item.actor.display_name || item.actor.username"
              :color-key="item.actor.id"
              class="size-10! shrink-0 text-white!"
            />
            <span
              v-else
              class="grid size-10 shrink-0 place-items-center rounded-full bg-surface-100 text-muted-color dark:bg-surface-800"
            >
              <component :is="kindIcon(item.kind)" :size="18" aria-hidden="true" />
            </span>
            <button
              type="button"
              class="min-w-0 flex-1 text-left outline-none disabled:cursor-default"
              :disabled="!item.source_available"
              @click="openSource(item)"
            >
              <span class="flex flex-wrap items-center gap-x-2 gap-y-1">
                <strong class="text-sm font-semibold text-color">{{ kindLabel(item.kind) }}</strong>
                <span v-if="item.room_name" class="truncate text-xs text-muted-color">{{ item.room_name }}</span>
              </span>
              <span
                class="mt-1 block text-sm leading-6"
                :class="item.source_available ? 'text-color' : 'text-muted-color'"
              >
                {{ item.summary }}
              </span>
              <time class="mt-1 block text-xs text-muted-color" :datetime="item.created_at">{{
                formatTime(item.created_at)
              }}</time>
            </button>
            <Button
              v-if="!item.read_at"
              text
              rounded
              severity="secondary"
              class="size-9! shrink-0 self-center p-0! opacity-70 sm:opacity-0 sm:group-hover:opacity-100 sm:focus-visible:opacity-100"
              aria-label="标记已读"
              title="标记已读"
              :disabled="mutating"
              @click="markRead(item)"
              ><Check :size="17" aria-hidden="true"
            /></Button>
          </li>
        </ol>
        <div v-if="nextCursor" class="flex justify-center pt-5">
          <Button severity="secondary" outlined :loading="loadingMore" label="加载更多" @click="loadMore" />
        </div>
      </section>
    </div>
  </main>
</template>
