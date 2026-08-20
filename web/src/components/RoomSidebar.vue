<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  Compass,
  EllipsisVertical,
  LockKeyhole,
  LogIn,
  LogOut,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Settings,
  SquarePen,
  UserRound,
  UsersRound,
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
import { avatarColor } from '../avatarColor'
import { useSidebarWidth } from '../composables/useSidebarWidth'
import type { ConversationSummary, Room, User } from '../types'
import ConversationRow from './ConversationRow.vue'

const props = defineProps<{
  conversations: ConversationSummary[]
  selectedId?: string
  user: User | null
  loading: boolean
  refreshing: boolean
  visible: boolean
  collapsed: boolean
  incomingRequests: number
}>()
const emit = defineEmits<{
  select: [conversation: ConversationSummary]
  clear: []
  refresh: []
  newChat: []
  create: []
  join: []
  discover: []
  contacts: []
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
const query = ref('')
const visibleConversations = computed(() => {
  const needle = query.value.trim().toLowerCase()
  return props.conversations.filter((item) => !needle || item.title.toLowerCase().includes(needle))
})
const contextMenu = ref()
const contextItems = ref<MenuItem[]>([])
const menu = ref()
const menuItems = computed<MenuItem[]>(() => [
  { label: '联系人', command: () => (props.user ? emit('contacts') : emit('authenticate')) },
  { label: '新建群聊', command: () => emit('create') },
  { label: '发现群聊', command: () => emit('discover') },
  { label: '通过 ID 加入', command: () => emit('join') },
  { label: '刷新会话', command: () => emit('refresh') },
])

function openContextMenu(event: MouseEvent, conversation: ConversationSummary): void {
  const items: MenuItem[] = [{ label: '打开会话', command: () => emit('select', conversation) }]
  const room = conversation.group
  if (room && ['owner', 'admin'].includes(room.membership_role || '')) {
    items.push({ label: '管理群聊', command: () => emit('manage', room) })
  }
  if (room?.membership_role && room.membership_role !== 'owner') {
    items.push({ label: '退出群聊', command: () => emit('leaveRoom', room) })
  }
  contextItems.value = items
  contextMenu.value?.show(event)
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
    >
      <span class="room-sync-progress block h-full w-1/3 bg-primary" />
    </div>
    <header
      class="flex h-[72px] shrink-0 items-center justify-between gap-2 border-b border-surface-200 px-3"
      :class="{ 'md:justify-center md:px-2': collapsed }"
    >
      <div class="flex min-w-0 items-center gap-3" :class="{ 'md:hidden': collapsed }">
        <img src="/brand/echo-gate.svg" alt="" class="size-8" aria-hidden="true" />
        <div class="min-w-0">
          <h1 class="truncate text-[15px] font-semibold text-surface-900">消息</h1>
          <p class="mt-0.5 truncate text-xs text-muted-color">
            {{ user ? `${conversations.length} 个会话` : '登录后开始聊天' }}
          </p>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <Button
          v-if="user"
          text
          rounded
          aria-label="新对话"
          title="新对话"
          data-testid="new-chat-button"
          @click="emit('newChat')"
        >
          <SquarePen :size="18" />
        </Button>
        <Button text rounded severity="secondary" aria-label="更多操作" title="更多操作" @click="menu.toggle($event)">
          <EllipsisVertical :size="18" />
        </Button>
        <Menu ref="menu" :model="menuItems" :popup="true" />
        <Button
          class="hidden md:inline-flex"
          text
          rounded
          severity="secondary"
          :aria-label="collapsed ? '展开侧边栏' : '收起侧边栏'"
          :title="collapsed ? '展开侧边栏' : '收起侧边栏'"
          @click="emit('toggleCollapse')"
        >
          <PanelLeftOpen v-if="collapsed" :size="18" /><PanelLeftClose v-else :size="18" />
        </Button>
      </div>
    </header>

    <div v-if="!collapsed" class="shrink-0 px-3 py-2">
      <IconField
        ><InputIcon><Search :size="14" /></InputIcon
        ><InputText v-model="query" placeholder="搜索会话" size="small" fluid aria-label="搜索会话"
      /></IconField>
    </div>
    <nav
      class="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-2"
      aria-label="会话列表"
      data-testid="conversation-list"
    >
      <div v-if="loading" class="space-y-1 p-1">
        <div v-for="index in 6" :key="index" class="flex h-[68px] items-center gap-3 px-2">
          <Skeleton shape="circle" size="2.75rem" />
          <div class="flex-1 space-y-2" :class="{ 'md:hidden': collapsed }">
            <Skeleton width="58%" height="0.85rem" /><Skeleton width="76%" height="0.65rem" />
          </div>
        </div>
      </div>
      <div
        v-else-if="!visibleConversations.length"
        class="flex h-full flex-col items-center justify-center px-6 text-center text-muted-color"
      >
        <Search v-if="query" :size="22" /><SquarePen v-else :size="22" />
        <strong class="mt-3 text-sm text-color" :class="{ 'md:hidden': collapsed }">{{
          query ? '没有匹配的会话' : '还没有会话'
        }}</strong>
        <span v-if="!query && user" class="mt-1 text-xs" :class="{ 'md:hidden': collapsed }">从好友开始一段对话</span>
        <Button
          v-if="!query && user"
          class="mt-3"
          :class="{ 'md:hidden': collapsed }"
          size="small"
          outlined
          @click="emit('newChat')"
          ><SquarePen :size="15" />新对话</Button
        >
      </div>
      <template v-else>
        <button
          v-for="conversation in visibleConversations"
          :key="conversation.room_id"
          type="button"
          class="mb-0.5 flex h-[68px] w-full shrink-0 items-center gap-3 rounded-md px-2.5 text-left transition-colors active:bg-surface-100"
          :class="[
            conversation.room_id === selectedId ? 'bg-primary-50 text-primary-900' : 'hover:bg-surface-50',
            collapsed ? 'md:justify-center md:px-1' : '',
          ]"
          :aria-current="conversation.room_id === selectedId ? 'true' : undefined"
          :title="conversation.title"
          @click="emit('select', conversation)"
          @contextmenu.prevent="openContextMenu($event, conversation)"
        >
          <ConversationRow
            :conversation="conversation"
            :selected="conversation.room_id === selectedId"
            :collapsed="collapsed"
          />
        </button>
        <button
          type="button"
          class="min-h-10 w-full flex-1 cursor-default rounded-md"
          aria-label="取消选择会话"
          data-testid="conversation-list-blank"
          @click="emit('clear')"
        />
      </template>
    </nav>

    <button
      v-if="user"
      type="button"
      class="relative flex h-12 shrink-0 items-center gap-3 border-t border-surface-200 px-4 text-sm hover:bg-surface-50"
      :class="{ 'md:justify-center md:px-2': collapsed }"
      @click="emit('contacts')"
    >
      <UsersRound :size="18" /><span :class="{ 'md:hidden': collapsed }">联系人</span>
      <Badge
        v-if="incomingRequests"
        :value="incomingRequests > 99 ? '99+' : String(incomingRequests)"
        severity="danger"
        class="ml-auto"
      />
    </button>
    <footer
      class="flex min-h-[68px] shrink-0 items-center gap-1 border-t border-surface-200 bg-surface-50 px-3 py-2"
      :class="{ 'md:flex-col md:px-2': collapsed }"
    >
      <template v-if="user">
        <button
          type="button"
          class="flex min-w-0 flex-1 items-center gap-2 rounded-md text-left hover:text-primary"
          :class="{ 'md:justify-center': collapsed }"
          title="我的资料"
          @click="emit('profile')"
        >
          <Avatar
            :label="user.avatar_emoji || user.username.slice(0, 1).toUpperCase()"
            shape="circle"
            class="shrink-0 text-white!"
            :style="{ backgroundColor: avatarColor(user.id) }"
          />
          <strong class="min-w-0 flex-1 truncate text-sm" :class="{ 'md:hidden': collapsed }">{{
            user.display_name || user.username
          }}</strong>
        </button>
        <Button text rounded severity="secondary" aria-label="锁定界面" title="锁定界面" @click="emit('lock')"
          ><LockKeyhole :size="17"
        /></Button>
        <Button text rounded severity="secondary" aria-label="设置" title="设置" @click="emit('settings')"
          ><Settings :size="17"
        /></Button>
        <Button text rounded severity="secondary" aria-label="退出登录" title="退出登录" @click="emit('logout')"
          ><LogOut :size="17"
        /></Button>
      </template>
      <template v-else>
        <Button
          class="min-w-0 flex-1"
          :class="{ 'md:flex-none': collapsed }"
          title="登录或注册"
          @click="emit('authenticate')"
          ><LogIn :size="17" /><span :class="{ 'md:hidden': collapsed }">登录或注册</span></Button
        >
        <Button text rounded severity="secondary" aria-label="发现群聊" title="发现群聊" @click="emit('discover')"
          ><Compass :size="17"
        /></Button>
        <Button text rounded severity="secondary" aria-label="应用偏好" title="应用偏好" @click="emit('settings')"
          ><UserRound :size="17"
        /></Button>
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
    <ContextMenu ref="contextMenu" :model="contextItems" />
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
