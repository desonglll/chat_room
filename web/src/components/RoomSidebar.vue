<script setup lang="ts">
import {
  ChevronRight,
  Hash,
  LockKeyhole,
  LogIn,
  LogOut,
  MessagesSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  RefreshCw,
  Settings,
} from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Badge from 'primevue/badge'
import Button from 'primevue/button'
import Skeleton from 'primevue/skeleton'
import type { Room, User } from '../types'

defineProps<{
  rooms: Room[]
  selectedId?: string
  user: User | null
  loading: boolean
  visible: boolean
  collapsed: boolean
}>()

const emit = defineEmits<{
  select: [room: Room]
  refresh: []
  create: []
  authenticate: []
  logout: []
  settings: []
  toggleCollapse: []
}>()

function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', { month: '2-digit', day: '2-digit' }).format(date)
}
</script>

<template>
  <aside
    class="min-h-0 min-w-0 flex-col border-r border-surface-200 bg-surface-0 transition-[width] duration-200 ease-out md:flex"
    :class="visible ? 'flex' : 'hidden'"
  >
    <header class="flex h-[72px] shrink-0 items-center justify-between gap-2 border-b border-surface-200 px-4" :class="{ 'md:justify-center md:px-2': collapsed }">
      <div class="flex min-w-0 items-center gap-3">
        <span class="grid size-10 shrink-0 place-items-center rounded-lg bg-primary text-primary-contrast shadow-sm" :class="{ 'md:hidden': collapsed }">
          <MessagesSquare :size="21" />
        </span>
        <div class="min-w-0" :class="{ 'md:hidden': collapsed }">
          <h1 class="truncate text-[15px] font-semibold text-surface-900">Chat Room</h1>
          <p class="mt-0.5 text-xs text-muted-color" aria-live="polite">
            {{ loading ? '正在读取房间' : `${rooms.length} 个聊天室` }}
          </p>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <Button :class="{ 'md:hidden': collapsed }" text rounded severity="secondary" aria-label="刷新房间" title="刷新房间" :disabled="loading" @click="emit('refresh')">
          <RefreshCw :size="17" :class="{ 'animate-spin': loading }" />
        </Button>
        <Button :class="{ 'md:hidden': collapsed }" text rounded aria-label="新建聊天室" title="新建聊天室" data-testid="create-room-button" @click="emit('create')">
          <Plus :size="17" />
        </Button>
        <Button class="hidden md:inline-flex" text rounded severity="secondary" :aria-label="collapsed ? '展开侧边栏' : '收起侧边栏'" :title="collapsed ? '展开侧边栏' : '收起侧边栏'" @click="emit('toggleCollapse')">
          <PanelLeftOpen v-if="collapsed" :size="18" />
          <PanelLeftClose v-else :size="18" />
        </Button>
      </div>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto p-2" role="list" aria-label="聊天室列表" data-testid="room-list">
      <div v-if="loading" class="space-y-2 p-1">
        <div v-for="index in 5" :key="index" class="flex h-16 items-center gap-3 px-2">
          <Skeleton width="2.5rem" height="2.5rem" border-radius="8px" />
          <div class="flex-1 space-y-2" :class="{ 'md:hidden': collapsed }">
            <Skeleton width="58%" height="0.85rem" />
            <Skeleton width="38%" height="0.65rem" />
          </div>
        </div>
      </div>

      <div v-else-if="rooms.length === 0" class="flex h-full flex-col items-center justify-center text-center text-muted-color">
        <span class="grid size-12 place-items-center rounded-lg bg-surface-100"><Hash :size="23" /></span>
        <strong class="mt-3 text-sm text-color" :class="{ 'md:hidden': collapsed }">还没有聊天室</strong>
        <span class="mt-1 text-xs" :class="{ 'md:hidden': collapsed }">创建第一个房间</span>
      </div>

      <button
        v-for="room in rooms"
        v-else
        :key="room.id"
        class="relative mb-1 flex min-h-16 w-full items-center gap-3 rounded-lg border px-3 text-left transition-colors"
        :class="[
          room.id === selectedId
            ? 'border-primary-200 bg-primary-50 text-primary-900'
            : 'border-transparent bg-surface-0 text-surface-800 hover:border-surface-200 hover:bg-surface-50',
          collapsed ? 'md:justify-center md:px-1' : '',
        ]"
        type="button"
        role="listitem"
        :title="room.name"
        :aria-current="room.id === selectedId ? 'true' : undefined"
        @click="emit('select', room)"
      >
        <span
          class="grid size-9 shrink-0 place-items-center rounded-md"
          :class="room.has_password ? 'bg-amber-50 text-amber-700' : 'bg-emerald-50 text-emerald-700'"
        >
          <LockKeyhole v-if="room.has_password" :size="17" />
          <Hash v-else :size="17" />
        </span>
        <span class="min-w-0 flex-1" :class="{ 'md:hidden': collapsed }">
          <strong class="block truncate text-sm font-semibold">{{ room.name }}</strong>
          <small class="mt-1 block truncate text-xs text-muted-color">
            {{ room.membership_status === 'pending' ? '待审核' : room.membership_status === 'invited' ? '已邀请' : (room.has_password ? '私密' : '公开') }} · {{ formatDate(room.created_at) }}
          </small>
        </span>
        <Badge
          v-if="room.unread_count > 0"
          :value="room.unread_count > 99 ? '99+' : String(room.unread_count)"
          severity="danger"
          :class="{ 'md:absolute md:right-0 md:top-1': collapsed }"
        />
        <ChevronRight class="shrink-0 text-surface-400" :class="{ 'md:hidden': collapsed }" :size="17" />
      </button>
    </div>

    <footer class="flex min-h-[76px] shrink-0 items-center gap-2 border-t border-surface-200 bg-surface-50 px-4 py-3" :class="{ 'md:flex-col md:px-2': collapsed }">
      <template v-if="user">
        <Avatar :label="user.avatar_emoji || user.username.slice(0, 1).toUpperCase()" shape="circle" class="shrink-0 bg-surface-200! text-surface-700!" />
        <div class="min-w-0 flex-1" :class="{ 'md:hidden': collapsed }">
          <small class="block text-[11px] text-muted-color">当前用户</small>
          <strong class="mt-0.5 block truncate text-sm">{{ user.username }}</strong>
        </div>
        <Button text rounded severity="secondary" aria-label="偏好设置" title="偏好设置" @click="emit('settings')">
          <Settings :size="17" />
        </Button>
        <Button text rounded severity="secondary" aria-label="退出登录" title="退出登录" @click="emit('logout')">
          <LogOut :size="17" />
        </Button>
      </template>
      <template v-else>
        <Button class="min-w-0 flex-1" :class="{ 'md:flex-none': collapsed }" :aria-label="collapsed ? '登录或注册' : undefined" title="登录或注册" @click="emit('authenticate')">
          <LogIn :size="17" />
          <span :class="{ 'md:hidden': collapsed }">登录或注册</span>
        </Button>
        <Button text rounded severity="secondary" aria-label="偏好设置" title="偏好设置" @click="emit('settings')">
          <Settings :size="17" />
        </Button>
      </template>
    </footer>
  </aside>
</template>
