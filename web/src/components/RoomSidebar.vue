<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { EllipsisVertical, Search, SquarePen } from 'lucide-vue-next'
import Button from 'primevue/button'
import ContextMenu from 'primevue/contextmenu'
import IconField from 'primevue/iconfield'
import InputIcon from 'primevue/inputicon'
import InputText from 'primevue/inputtext'
import Menu from 'primevue/menu'
import Skeleton from 'primevue/skeleton'
import type { MenuItem } from 'primevue/menuitem'
import { useSidebarWidth } from '../composables/useSidebarWidth'
import { conversationPreferenceMenuItems } from '../conversationPreferenceMenu'
import type { ConversationPreferencesPatch } from '../conversationPreferencesApi'
import { conversationDisplayTitle, shouldRevealConversationPreview } from '../conversationState'
import type { ConversationSummary, Room, User } from '../types'
import ConversationAliasDialog from './ConversationAliasDialog.vue'
import ConversationRow from './ConversationRow.vue'
import WorkspaceRail from './WorkspaceRail.vue'
const props = defineProps<{
  conversations: ConversationSummary[]
  selectedId?: string
  user: User | null
  loading: boolean
  refreshing: boolean
  visible: boolean
  collapsed: boolean
  incomingRequests: number
  notificationUnreadCount: number
  activeSection: string
  setAlias: (roomId: string, alias: string) => Promise<ConversationSummary>
  updatePreferences: (roomId: string, patch: ConversationPreferencesPatch) => Promise<unknown>
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
  favorites: []
  notifications: []
  search: []
  assistant: []
  chat: []
  authenticate: []
  logout: []
  lock: []
  settings: []
  profile: []
  toggleCollapse: []
  resize: [width: number]
  manage: [room: Room]
  leaveRoom: [room: Room]
  success: [message: string]
  error: [message: string]
}>()
const { width: sidebarWidth, resizing, startResize, resizeBy } = useSidebarWidth()
watch(sidebarWidth, (width) => emit('resize', width), { immediate: true })
const query = ref('')
const visibleConversations = computed(() => {
  const needle = query.value.trim().toLowerCase()
  return props.conversations.filter((item) => !needle || `${item.alias} ${item.title}`.toLowerCase().includes(needle))
})
const conversationSections = computed(() => [
  {
    key: 'pinned',
    label: '置顶',
    items: visibleConversations.value.filter((item) => item.preferences.is_pinned && !item.preferences.is_archived),
  },
  {
    key: 'recent',
    label: '最近',
    items: visibleConversations.value.filter((item) => !item.preferences.is_pinned && !item.preferences.is_archived),
  },
  {
    key: 'archived',
    label: '已归档',
    items: visibleConversations.value.filter((item) => item.preferences.is_archived),
  },
])
const revealPreview = computed(() => shouldRevealConversationPreview(props.activeSection, props.selectedId))
const contextMenu = ref()
const contextItems = ref<MenuItem[]>([])
const rowMenu = ref()
const updatingRoomId = ref('')
const aliasConversation = ref<ConversationSummary | null>(null)
const aliasOpen = ref(false)
const menu = ref()
const menuItems = computed<MenuItem[]>(() => [
  { label: '联系人', command: () => (props.user ? emit('contacts') : emit('authenticate')) },
  { label: '新建群聊', command: () => emit('create') },
  { label: '通过 ID 加入', command: () => emit('join') },
  { label: '刷新会话', command: () => emit('refresh') },
])
function openContextMenu(event: MouseEvent, conversation: ConversationSummary): void {
  contextItems.value = conversationMenuItems(conversation)
  contextMenu.value?.show(event)
}
function openRowMenu(event: MouseEvent, conversation: ConversationSummary): void {
  contextItems.value = conversationMenuItems(conversation)
  rowMenu.value?.toggle(event)
}

function conversationMenuItems(conversation: ConversationSummary): MenuItem[] {
  const items: MenuItem[] = [
    { label: '打开会话', command: () => emit('select', conversation) },
    { label: conversation.alias ? '修改备注' : '设置备注', command: () => openAlias(conversation) },
    { separator: true },
    ...conversationPreferenceMenuItems(
      conversation,
      updatingRoomId.value === conversation.room_id,
      (patch, success) => void savePreference(conversation, patch, success),
    ),
  ]
  const room = conversation.group
  if (room && ['owner', 'admin'].includes(room.membership_role || '')) {
    items.push({ label: '管理群聊', command: () => emit('manage', room) })
  }
  if (room?.membership_role) items.push({ label: '退出群聊', command: () => emit('leaveRoom', room) })
  return items
}

async function savePreference(
  conversation: ConversationSummary,
  patch: ConversationPreferencesPatch,
  success: string,
): Promise<void> {
  if (updatingRoomId.value) return
  updatingRoomId.value = conversation.room_id
  try {
    await props.updatePreferences(conversation.room_id, patch)
    emit('success', success)
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '无法保存会话设置')
  } finally {
    updatingRoomId.value = ''
  }
}

function openAlias(conversation: ConversationSummary): void {
  aliasConversation.value = conversation
  aliasOpen.value = true
}

function handleResizeKeydown(event: KeyboardEvent): void {
  if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) return
  event.preventDefault()
  resizeBy((event.key === 'ArrowLeft' ? -1 : 1) * (event.shiftKey ? 32 : 8))
}
</script>

<template>
  <aside
    class="cr-sidebar absolute inset-0 z-10 grid min-h-0 min-w-0 md:relative md:inset-auto"
    :class="[{ 'cr-sidebar--nav-only': !visible }, { 'cr-sidebar--collapsed': collapsed }]"
  >
    <WorkspaceRail
      :active-section="activeSection"
      :user="user"
      :incoming-requests="incomingRequests"
      :notification-unread-count="notificationUnreadCount"
      :collapsed="collapsed"
      @chat="emit('chat')"
      @contacts="emit('contacts')"
      @favorites="emit('favorites')"
      @notifications="emit('notifications')"
      @search="emit('search')"
      @assistant="emit('assistant')"
      @discover="emit('discover')"
      @create="emit('create')"
      @authenticate="emit('authenticate')"
      @profile="emit('profile')"
      @lock="emit('lock')"
      @settings="emit('settings')"
      @logout="emit('logout')"
      @toggle-collapse="emit('toggleCollapse')"
    />

    <section class="cr-inbox-pane">
      <div v-if="refreshing && !loading" class="cr-inbox-sync" aria-label="正在刷新">
        <span class="room-sync-progress" />
      </div>
      <header class="cr-inbox-header">
        <div class="min-w-0">
          <h1>消息</h1>
          <p>{{ user ? `${conversations.length} 个会话` : '登录后开始聊天' }}</p>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <Button
            v-if="user"
            class="size-9! shrink-0 p-0!"
            rounded
            aria-label="新对话"
            title="新对话"
            data-testid="new-chat-button"
            @click="emit('newChat')"
          >
            <SquarePen :size="17" aria-hidden="true" />
          </Button>
          <Button
            class="size-9! shrink-0 p-0!"
            text
            rounded
            severity="secondary"
            aria-label="更多操作"
            title="更多操作"
            @click="menu.toggle($event)"
          >
            <EllipsisVertical :size="18" aria-hidden="true" />
          </Button>
          <Menu ref="menu" :model="menuItems" :popup="true" class="cr-menu-top-right" />
        </div>
      </header>

      <div class="cr-inbox-search">
        <IconField class="w-full">
          <InputIcon><Search :size="15" aria-hidden="true" /></InputIcon>
          <InputText
            v-model="query"
            name="conversation-search"
            autocomplete="off"
            placeholder="搜索会话…"
            variant="filled"
            fluid
            aria-label="搜索会话"
            class="h-10 pl-9! text-sm"
          />
        </IconField>
      </div>

      <nav id="conversation-list" class="cr-conversation-list" aria-label="会话列表" data-testid="conversation-list">
        <div v-if="loading" class="space-y-1 p-1">
          <div v-for="index in 6" :key="index" class="flex h-[72px] items-center gap-3 px-2">
            <Skeleton shape="circle" size="2.75rem" />
            <div class="flex-1 space-y-2">
              <Skeleton width="58%" height="0.85rem" /><Skeleton width="76%" height="0.65rem" />
            </div>
          </div>
        </div>
        <div v-else-if="!visibleConversations.length" class="cr-inbox-empty">
          <span
            ><Search v-if="query" :size="20" aria-hidden="true" /><SquarePen v-else :size="20" aria-hidden="true"
          /></span>
          <strong>{{ query ? '没有匹配的会话' : '还没有会话' }}</strong>
          <small v-if="!query && user">从好友开始一段对话</small>
          <Button v-if="!query && user" size="small" outlined @click="emit('newChat')">
            <SquarePen :size="15" />新对话
          </Button>
        </div>
        <template v-else>
          <div v-for="section in conversationSections" :key="section.key">
            <template v-if="section.items.length">
              <h2 class="px-2 pb-1 pt-3 text-[11px] font-semibold text-muted-color">{{ section.label }}</h2>
              <div v-for="conversation in section.items" :key="conversation.room_id" class="group relative">
                <button
                  type="button"
                  class="cr-conversation-row pr-11!"
                  :class="
                    conversation.room_id === selectedId ? 'cr-conversation-row--active' : 'cr-conversation-row--idle'
                  "
                  :aria-current="conversation.room_id === selectedId ? 'true' : undefined"
                  :title="
                    conversation.alias
                      ? `${conversationDisplayTitle(conversation)}（原名：${conversation.title}）`
                      : conversation.title
                  "
                  @click="emit('select', conversation)"
                  @contextmenu.prevent="openContextMenu($event, conversation)"
                >
                  <ConversationRow
                    :conversation="conversation"
                    :selected="conversation.room_id === selectedId"
                    :collapsed="false"
                    :reveal-preview="revealPreview"
                  />
                </button>
                <button
                  type="button"
                  class="absolute right-2 top-1/2 grid size-8 -translate-y-1/2 place-items-center rounded-md text-muted-color opacity-0 outline-none transition-[opacity,color,background-color,transform] duration-[var(--cr-motion-fast)] hover:bg-surface-100 hover:text-primary focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-primary group-hover:opacity-100 max-md:opacity-100 motion-reduce:transition-none"
                  :class="{ 'opacity-100': conversation.room_id === selectedId }"
                  :aria-label="`${conversationDisplayTitle(conversation)}的更多操作`"
                  title="更多操作"
                  :disabled="updatingRoomId === conversation.room_id"
                  @click.stop="openRowMenu($event, conversation)"
                >
                  <EllipsisVertical :size="16" aria-hidden="true" />
                </button>
              </div>
            </template>
          </div>
          <button
            type="button"
            class="min-h-10 w-full flex-1 cursor-default outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
            aria-label="取消选择会话"
            data-testid="conversation-list-blank"
            @click="emit('clear')"
          />
        </template>
      </nav>
    </section>

    <div
      v-if="!collapsed"
      class="cr-sidebar-resizer group"
      role="separator"
      aria-orientation="vertical"
      aria-label="调整会话栏宽度"
      aria-valuemin="340"
      aria-valuemax="460"
      :aria-valuenow="sidebarWidth"
      tabindex="0"
      @pointerdown.prevent="startResize"
      @keydown="handleResizeKeydown"
    >
      <span :class="{ 'opacity-100': resizing }" />
    </div>
    <ContextMenu ref="contextMenu" :model="contextItems" />
    <Menu ref="rowMenu" :model="contextItems" :popup="true" />
    <ConversationAliasDialog
      :open="aliasOpen"
      :conversation="aliasConversation"
      :set-alias="setAlias"
      @close="aliasOpen = false"
    />
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
