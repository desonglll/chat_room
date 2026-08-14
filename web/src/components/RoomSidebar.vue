<script setup lang="ts">
import { ChevronRight, Hash, LockKeyhole, LogIn, LogOut, MessagesSquare, Plus, RefreshCw, UserRound } from 'lucide-vue-next'
import type { Room, User } from '../types'

defineProps<{
  rooms: Room[]
  selectedId?: string
  user: User | null
  loading: boolean
  visible: boolean
}>()

const emit = defineEmits<{
  select: [room: Room]
  refresh: []
  create: []
  authenticate: []
  logout: []
}>()

function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', { month: '2-digit', day: '2-digit' }).format(date)
}

</script>

<template>
  <aside class="sidebar" :class="{ 'mobile-hidden': !visible }">
    <header class="sidebar-header">
      <div class="brand-lockup">
        <span class="brand-mark"><MessagesSquare :size="20" /></span>
        <div class="min-width-zero">
          <h1>Chat Room</h1>
          <p aria-live="polite">{{ loading ? '正在读取房间' : `${rooms.length} 个聊天室` }}</p>
        </div>
      </div>
      <div class="toolbar">
        <button class="icon-button" type="button" aria-label="刷新房间" title="刷新房间" :disabled="loading" @click="emit('refresh')">
          <RefreshCw :size="17" :class="{ spinning: loading }" />
        </button>
        <button class="primary-button compact" type="button" data-testid="create-room-button" @click="emit('create')">
          <Plus :size="17" />
          新建
        </button>
      </div>
    </header>

    <div class="room-list" role="list" aria-label="聊天室列表" data-testid="room-list">
      <div v-if="!loading && rooms.length === 0" class="room-list-empty">
        <Hash :size="24" />
        <strong>还没有聊天室</strong>
        <span>创建第一个房间</span>
      </div>
      <button
        v-for="room in rooms"
        :key="room.id"
        class="room-row"
        :class="{ active: room.id === selectedId }"
        type="button"
        role="listitem"
        :aria-current="room.id === selectedId ? 'true' : undefined"
        @click="emit('select', room)"
      >
        <span class="room-access" :class="room.has_password ? 'private' : 'public'">
          <LockKeyhole v-if="room.has_password" :size="17" />
          <Hash v-else :size="17" />
        </span>
        <span class="room-copy">
          <strong>{{ room.name }}</strong>
          <small>{{ room.has_password ? '私密' : '公开' }} · {{ formatDate(room.created_at) }}</small>
        </span>
        <ChevronRight class="room-chevron" :size="17" />
      </button>
    </div>

    <footer class="account-panel">
      <template v-if="user">
        <span class="account-avatar"><UserRound :size="18" /></span>
        <div class="account-copy">
          <small>当前用户</small>
          <strong>{{ user.username }}</strong>
        </div>
        <button class="icon-button" type="button" aria-label="退出登录" title="退出登录" @click="emit('logout')">
          <LogOut :size="17" />
        </button>
      </template>
      <button v-else class="primary-button wide account-login" type="button" @click="emit('authenticate')">
        <LogIn :size="17" />
        登录或注册
      </button>
    </footer>
  </aside>
</template>
