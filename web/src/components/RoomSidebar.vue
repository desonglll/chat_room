<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  Compass,
  EllipsisVertical,
  LogIn,
  LogOut,
  LockKeyhole,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Search,
  Settings,
  UserRound,
} from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Badge from 'primevue/badge'
import Button from 'primevue/button'
import ContextMenu from 'primevue/contextmenu'
import IconField from 'primevue/iconfield'
import InputIcon from 'primevue/inputicon'
import InputText from 'primevue/inputtext'
import Menu from 'primevue/menu'
import Skeleton from 'primevue/skeleton'
import type { MenuItem } from 'primevue/menuitem'
import IconSprite from './IconSprite.vue'
import { avatarColor } from '../avatarColor'
import { useSidebarWidth } from '../composables/useSidebarWidth'
import type { Room, User } from '../types'

const props = defineProps<{
  rooms: Room[]
  selectedId?: string
  user: User | null
  loading: boolean
  refreshing: boolean
  visible: boolean
  collapsed: boolean
}>()

const emit = defineEmits<{
  select: [room: Room]
  refresh: []
  create: []
  join: []
  discover: []
  authenticate: []
  logout: []
  lock: []
  settings: []
  profile: []
  toggleCollapse: []
  resize: [width: number]
  manage: [room: Room]
  leaveRoom: [room: Room]
}>()

const { width: sidebarWidth, resizing, startResize } = useSidebarWidth()
watch(sidebarWidth, (width) => emit('resize', width), { immediate: true })

// The sidebar is "my rooms" only — public rooms the user hasn't joined live
// on the Discover page instead, reachable via the compass button below.
const searchQuery = ref('')
const joinedRooms = computed(() => {
  const needle = searchQuery.value.trim().toLowerCase()
  return props.rooms
    .filter((room) => room.membership_status)
    .filter((room) => !needle || room.name.toLowerCase().includes(needle))
})

const roomContextMenu = ref()
const roomContextMenuItems = ref<MenuItem[]>([])

const sidebarMenu = ref()
const sidebarMenuItems = computed<MenuItem[]>(() => [
  { label: '刷新房间', command: () => emit('refresh') },
  { label: '发现聊天室', command: () => emit('discover') },
  { label: '通过 ID 加入', command: () => emit('join') },
])

function openRoomContextMenu(event: MouseEvent, room: Room): void {
  const items: MenuItem[] = [{ label: '打开聊天室', command: () => emit('select', room) }]
  if (['owner', 'admin'].includes(room.membership_role || '')) {
    items.push({ label: '管理聊天室', command: () => emit('manage', room) })
  }
  if (room.membership_role && room.membership_role !== 'owner') {
    items.push({
      label: '退出聊天室',
      command: () => {
        if (window.confirm(`确定退出聊天室"${room.name}"吗？`)) emit('leaveRoom', room)
      },
    })
  }
  roomContextMenuItems.value = items
  roomContextMenu.value?.show(event)
}

function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', { month: '2-digit', day: '2-digit' }).format(date)
}
</script>

<template>
  <aside
    class="absolute inset-0 z-10 flex min-h-0 min-w-0 flex-col border-r border-surface-200 bg-surface-0 shadow-sm transition-[transform,opacity,visibility] duration-200 ease-out motion-reduce:transition-none md:relative md:inset-auto md:visible md:translate-x-0 md:opacity-100"
    :class="[
      visible
        ? 'visible translate-x-0 opacity-100'
        : 'invisible pointer-events-none -translate-x-4 opacity-0 md:pointer-events-auto',
      resizing ? '' : 'md:transition-[width] md:duration-200 md:ease-out',
    ]"
  >
    <div
      v-if="refreshing && !loading"
      class="pointer-events-none absolute inset-x-0 top-0 z-20 h-0.5 overflow-hidden bg-primary-100"
      aria-hidden="true"
    >
      <span class="room-sync-progress block h-full w-1/3 bg-primary" />
    </div>
    <header
      class="flex h-[72px] shrink-0 items-center justify-between gap-2 border-b border-surface-200 px-4"
      :class="{ 'md:justify-center md:px-2': collapsed }"
    >
      <div class="flex min-w-0 items-center gap-3">
        <span
          class="grid size-10 shrink-0 place-items-center rounded-lg bg-surface-100 shadow-sm"
          :class="{ 'md:hidden': collapsed }"
        >
          <img src="/brand/echo-gate.svg" alt="" class="size-7" aria-hidden="true" />
        </span>
        <div class="min-w-0" :class="{ 'md:hidden': collapsed }">
          <h1 class="truncate text-[15px] font-semibold text-surface-900">Chat Room</h1>
          <p class="mt-0.5 truncate text-xs text-muted-color" aria-live="polite">
            {{ loading ? '正在读取房间' : refreshing ? '正在同步' : `${joinedRooms.length} 个聊天室` }}
          </p>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <Button
          :class="{ 'md:hidden': collapsed }"
          text
          rounded
          aria-label="新建聊天室"
          title="新建聊天室"
          data-testid="create-room-button"
          @click="emit('create')"
        >
          <Plus :size="17" />
        </Button>
        <Button
          :class="{ 'md:hidden': collapsed }"
          text
          rounded
          severity="secondary"
          aria-label="更多操作"
          title="更多操作"
          @click="sidebarMenu.toggle($event)"
        >
          <EllipsisVertical :size="17" />
        </Button>
        <Menu ref="sidebarMenu" :model="sidebarMenuItems" :popup="true" />
        <Button
          class="hidden md:inline-flex"
          text
          rounded
          severity="secondary"
          :aria-label="collapsed ? '展开侧边栏' : '收起侧边栏'"
          :title="collapsed ? '展开侧边栏' : '收起侧边栏'"
          @click="emit('toggleCollapse')"
        >
          <PanelLeftOpen v-if="collapsed" :size="18" />
          <PanelLeftClose v-else :size="18" />
        </Button>
      </div>
    </header>

    <div v-if="!collapsed" class="shrink-0 border-b border-surface-200 px-3 py-2">
      <IconField>
        <InputIcon><Search :size="14" /></InputIcon>
        <InputText v-model="searchQuery" placeholder="搜索聊天室" size="small" fluid aria-label="搜索聊天室" />
      </IconField>
    </div>

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

      <div
        v-else-if="joinedRooms.length === 0 && searchQuery.trim()"
        class="flex h-full flex-col items-center justify-center text-center text-muted-color"
      >
        <span
          class="grid size-14 place-items-center rounded-xl bg-gradient-to-br from-primary-50 to-surface-0 shadow-sm"
          ><Search :size="20"
        /></span>
        <strong class="mt-3 text-sm text-color" :class="{ 'md:hidden': collapsed }">没有匹配的聊天室</strong>
      </div>

      <div
        v-else-if="joinedRooms.length === 0"
        class="flex h-full flex-col items-center justify-center text-center text-muted-color"
      >
        <span
          class="grid size-14 place-items-center rounded-xl bg-gradient-to-br from-primary-50 to-surface-0 shadow-sm"
          ><IconSprite name="rooms" :size="23"
        /></span>
        <strong class="mt-3 text-sm text-color" :class="{ 'md:hidden': collapsed }">还没有聊天室</strong>
        <span class="mt-1 text-xs" :class="{ 'md:hidden': collapsed }">创建一个，或去发现公开聊天室</span>
        <Button class="mt-3" :class="{ 'md:hidden': collapsed }" size="small" outlined @click="emit('discover')">
          <Compass :size="15" /><span>发现聊天室</span>
        </Button>
      </div>

      <button
        v-for="room in joinedRooms"
        v-else
        :key="room.id"
        class="relative mb-1.5 flex min-h-16 w-full items-center gap-3 rounded-xl px-3 text-left transition-[background-color,box-shadow,transform] duration-200 ease-spring active:scale-[0.97]"
        :class="[
          room.id === selectedId
            ? 'bg-primary-50 text-primary-900 shadow-sm ring-1 ring-primary-200'
            : 'bg-surface-0 text-surface-800 shadow-xs hover:bg-surface-50 hover:shadow-sm',
          collapsed ? 'md:justify-center md:px-1' : '',
        ]"
        type="button"
        role="listitem"
        :title="room.name"
        :aria-current="room.id === selectedId ? 'true' : undefined"
        @click="emit('select', room)"
        @contextmenu.prevent="openRoomContextMenu($event, room)"
      >
        <span
          class="grid size-9 shrink-0 place-items-center rounded-full text-base text-white"
          :style="{ backgroundColor: avatarColor(room.id) }"
        >
          <template v-if="room.avatar_emoji">{{ room.avatar_emoji }}</template>
          <IconSprite v-else-if="room.has_password" name="lock" :size="17" />
          <template v-else>{{ room.name.slice(0, 1).toUpperCase() }}</template>
        </span>
        <span class="min-w-0 flex-1" :class="{ 'md:hidden': collapsed }">
          <span class="flex items-baseline gap-2">
            <strong class="min-w-0 flex-1 truncate text-sm font-semibold">{{ room.name }}</strong>
            <small class="shrink-0 text-[11px] text-muted-color">{{ formatDate(room.created_at) }}</small>
          </span>
          <span class="mt-0.5 flex items-center gap-2">
            <small class="min-w-0 flex-1 truncate text-xs text-muted-color">
              {{
                room.membership_status === 'pending'
                  ? '待审核'
                  : room.membership_status === 'invited'
                    ? '已邀请'
                    : room.has_password
                      ? '私密'
                      : '公开'
              }}
            </small>
            <Badge
              v-if="room.unread_count > 0"
              :value="room.unread_count > 99 ? '99+' : String(room.unread_count)"
              severity="danger"
              class="shrink-0"
            />
          </span>
        </span>
        <Badge
          v-if="room.unread_count > 0 && collapsed"
          :value="room.unread_count > 99 ? '99+' : String(room.unread_count)"
          severity="danger"
          class="md:absolute md:right-0 md:top-1"
        />
      </button>
    </div>

    <footer
      class="flex min-h-[76px] shrink-0 items-center gap-2 border-t border-surface-200 bg-surface-50 px-4 py-3"
      :class="{ 'md:flex-col md:px-2': collapsed }"
    >
      <template v-if="user">
        <button
          type="button"
          class="flex min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-md text-left transition hover:text-primary"
          :class="{ 'md:justify-center': collapsed }"
          title="我的"
          @click="emit('profile')"
        >
          <Avatar
            :label="user.avatar_emoji || user.username.slice(0, 1).toUpperCase()"
            shape="circle"
            class="shrink-0 text-white!"
            :style="{ backgroundColor: avatarColor(user.id) }"
          />
          <span class="min-w-0 flex-1" :class="{ 'md:hidden': collapsed }">
            <small class="block text-[11px] text-muted-color">当前用户</small>
            <strong class="mt-0.5 block truncate text-sm">{{ user.display_name || user.username }}</strong>
          </span>
        </button>
        <Button text rounded severity="secondary" aria-label="锁定界面" title="锁定界面" @click="emit('lock')">
          <LockKeyhole :size="17" />
        </Button>
        <Button text rounded severity="secondary" aria-label="设置" title="设置" @click="emit('settings')">
          <Settings :size="17" />
        </Button>
        <Button text rounded severity="secondary" aria-label="退出登录" title="退出登录" @click="emit('logout')">
          <LogOut :size="17" />
        </Button>
      </template>
      <template v-else>
        <Button
          class="min-w-0 flex-1"
          :class="{ 'md:flex-none': collapsed }"
          :aria-label="collapsed ? '登录或注册' : undefined"
          title="登录或注册"
          @click="emit('authenticate')"
        >
          <LogIn :size="17" />
          <span :class="{ 'md:hidden': collapsed }">登录或注册</span>
        </Button>
        <Button text rounded severity="secondary" aria-label="应用偏好" title="应用偏好" @click="emit('settings')">
          <UserRound :size="17" />
        </Button>
      </template>
    </footer>

    <div
      v-if="!collapsed"
      class="absolute inset-y-0 right-0 z-10 hidden w-1.5 -translate-x-1/2 cursor-col-resize touch-none select-none hover:bg-primary-200 md:block"
      :class="{ 'bg-primary-300': resizing }"
      role="separator"
      aria-orientation="vertical"
      aria-label="调整侧边栏宽度"
      @pointerdown.prevent="startResize"
    />
    <ContextMenu ref="roomContextMenu" :model="roomContextMenuItems" />
  </aside>
</template>

<style scoped>
@keyframes room-sync {
  from {
    transform: translateX(-120%);
  }
  to {
    transform: translateX(400%);
  }
}

.room-sync-progress {
  animation: room-sync 1s var(--cr-ease-out) infinite;
}

@media (prefers-reduced-motion: reduce) {
  .room-sync-progress {
    animation: none;
    width: 100%;
  }
}
</style>
