<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  ArrowLeft,
  Ban,
  Check,
  Copy,
  EllipsisVertical,
  ListChecks,
  LogOut,
  UserMinus,
  UserRound,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import Menu from 'primevue/menu'
import Popover from 'primevue/popover'
import type { ChatStatus, Room, RoomMember, UserSummary } from '../types'
import AdminRoomLockButton from './AdminRoomLockButton.vue'
import AppAvatar from './AppAvatar.vue'
import IconSprite from './IconSprite.vue'

const props = defineProps<{
  room: Room
  alias: string
  originalTitle: string
  kind: 'group' | 'direct'
  peer: UserSummary | null
  status: ChatStatus
  statusLabel: string
  authenticated: boolean
  members: RoomMember[]
  currentUserId: string
  token: string
}>()

const emit = defineEmits<{
  back: []
  manage: []
  leave: []
  files: []
  viewProfile: [userId: string]
  toggleSelection: []
  removeFriend: []
  blockUser: []
}>()

const memberPopover = ref()
const moreMenu = ref()
const roomIdCopied = ref(false)
const statusColor = computed(
  () =>
    ({
      idle: 'bg-surface-300',
      connecting: 'bg-warning',
      online: 'bg-success',
      offline: 'bg-danger',
      failed: 'bg-danger',
    })[props.status],
)
const canManage = computed(() => ['owner', 'admin'].includes(props.room.membership_role || ''))
const displayTitle = computed(() => props.alias || props.room.name)
const moreMenuItems = computed(() => [
  ...(props.kind === 'direct' && props.peer
    ? [{ label: '查看资料', icon: 'profile', command: () => emit('viewProfile', props.peer!.id) }]
    : []),
  { label: '多选消息', icon: 'select', command: () => emit('toggleSelection') },
  ...(props.kind === 'group' && canManage.value
    ? [{ label: '管理聊天室', icon: 'manage', command: () => emit('manage') }]
    : []),
  ...(props.kind === 'group'
    ? [{ label: '退出聊天室', icon: 'leave', danger: true, command: () => emit('leave') }]
    : []),
  ...(props.kind === 'direct'
    ? [
        { label: '删除好友', icon: 'remove', danger: true, command: confirmRemoveFriend },
        { label: '拉黑', icon: 'block', danger: true, command: confirmBlockUser },
      ]
    : []),
])

function confirmRemoveFriend(): void {
  if (window.confirm(`删除好友“${displayTitle.value}”并关闭私聊？`)) emit('removeFriend')
}

function confirmBlockUser(): void {
  if (window.confirm(`拉黑“${displayTitle.value}”？双方将无法继续私聊。`)) emit('blockUser')
}

async function copyRoomId(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.room.id)
    roomIdCopied.value = true
    window.setTimeout(() => {
      roomIdCopied.value = false
    }, 1600)
  } catch {
    window.prompt('复制聊天室 ID', props.room.id)
  }
}
</script>

<template>
  <header class="cr-chat-header flex shrink-0 items-center justify-between gap-3 px-3 sm:px-4">
    <div class="cr-chat-identity flex min-w-0 items-center gap-2 sm:gap-3">
      <Button
        class="cr-header-back md:hidden"
        text
        rounded
        severity="secondary"
        aria-label="返回房间列表"
        title="返回房间列表"
        @click="emit('back')"
      >
        <ArrowLeft :size="20" />
      </Button>
      <button
        v-if="kind === 'direct' && peer"
        type="button"
        class="cr-profile-trigger cr-chat-avatar shrink-0 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
        aria-label="查看对方资料"
        title="查看对方资料"
        aria-haspopup="dialog"
        @click="emit('viewProfile', peer.id)"
      >
        <AppAvatar
          :avatar="room.avatar_emoji"
          :fallback="displayTitle"
          :color-key="peer?.id || room.id"
          class="size-9! text-white!"
        />
      </button>
      <span v-else class="cr-chat-avatar shrink-0" aria-hidden="true">
        <AppAvatar
          :avatar="room.avatar_emoji"
          :fallback="displayTitle"
          :color-key="room.id"
          class="size-9! text-white!"
        />
      </span>
      <div class="group min-w-0">
        <div class="flex min-w-0 items-center gap-1.5">
          <button
            v-if="kind === 'direct' && peer"
            type="button"
            class="cr-chat-header-title min-w-0 flex-1 truncate rounded-sm text-left text-base font-semibold text-surface-900 outline-none hover:text-primary focus-visible:ring-2 focus-visible:ring-primary"
            :title="alias ? `原名：${originalTitle}` : room.description || undefined"
            @click="emit('viewProfile', peer.id)"
          >
            {{ displayTitle }}
          </button>
          <strong
            v-else
            class="cr-chat-header-title min-w-0 flex-1 truncate text-base font-semibold text-surface-900"
            :title="alias ? `原名：${originalTitle}` : room.description || undefined"
          >
            {{ displayTitle }}
          </strong>
          <Button
            v-if="kind === 'group'"
            class="cr-copy-room-id"
            text
            rounded
            severity="secondary"
            size="small"
            :aria-label="roomIdCopied ? '聊天室 ID 已复制' : '复制聊天室 ID'"
            :title="roomIdCopied ? '已复制' : `复制 ID：${room.id}`"
            @click="copyRoomId"
          >
            <Check v-if="roomIdCopied" :size="14" class="text-success" />
            <Copy v-else :size="14" />
          </Button>
          <span class="sr-only" aria-live="polite">{{ roomIdCopied ? '聊天室 ID 已复制' : '' }}</span>
        </div>
        <div class="cr-chat-status mt-0.5 flex min-w-0 items-center gap-1.5 text-xs" aria-live="polite">
          <span class="size-1.5 shrink-0 rounded-full" :class="statusColor" aria-hidden="true" />
          <span class="shrink-0">{{ statusLabel }}</span>
          <button
            v-if="authenticated && kind === 'group'"
            type="button"
            class="shrink-0 cursor-pointer rounded-sm outline-none underline-offset-2 hover:text-primary hover:underline focus-visible:ring-2 focus-visible:ring-primary"
            @click="memberPopover.toggle($event)"
          >
            · {{ members.length }} 人在线
          </button>
          <span v-if="kind === 'direct' && peer" class="truncate" translate="no">· @{{ peer.username }}</span>
          <span v-else-if="alias" class="hidden truncate sm:inline">· 原名 {{ originalTitle }}</span>
          <span v-else class="hidden shrink-0 sm:inline">· {{ room.has_password ? '私密房间' : '公开房间' }}</span>
        </div>
      </div>
    </div>

    <div class="cr-chat-actions flex shrink-0 items-center gap-0.5">
      <Popover v-if="kind === 'group'" ref="memberPopover" class="cr-popover-top-right">
        <div class="w-60">
          <div class="mb-2 flex items-center justify-between border-b border-surface-200 pb-3">
            <strong class="text-sm">在线成员</strong>
            <span class="text-xs text-muted-color">{{ members.length }}</span>
          </div>
          <ul v-if="members.length" class="max-h-72 space-y-1 overflow-y-auto overscroll-contain p-0">
            <li
              v-for="member in members"
              :key="member.user_id"
              class="group flex min-h-10 items-center gap-2.5 rounded-md px-1.5 py-1 text-sm hover:bg-surface-50"
            >
              <button
                type="button"
                class="min-w-0 touch-manipulation cursor-pointer rounded-full outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2"
                aria-label="查看用户资料"
                @click="emit('viewProfile', member.user_id)"
              >
                <AppAvatar
                  :avatar="member.avatar_emoji"
                  :fallback="member.username"
                  :color-key="member.user_id"
                  size="small"
                  class="shrink-0 text-white!"
                />
              </button>
              <span class="min-w-0 flex-1 truncate">{{
                member.user_id === currentUserId ? `${member.username}（你）` : member.username
              }}</span>
            </li>
          </ul>
          <p v-else class="py-6 text-center text-sm text-muted-color">暂无在线成员</p>
        </div>
      </Popover>
      <AdminRoomLockButton :room-id="room.id" :token="token" />
      <Button
        v-if="kind === 'group'"
        class="cr-header-action cr-header-secondary"
        text
        rounded
        severity="secondary"
        aria-label="查看在线成员"
        title="在线成员"
        @click="memberPopover.toggle($event)"
      >
        <IconSprite name="members" :size="19" />
      </Button>
      <Button
        v-if="authenticated"
        class="cr-header-action cr-header-secondary"
        text
        rounded
        severity="secondary"
        aria-label="聊天文件"
        title="聊天文件"
        @click="emit('files')"
      >
        <IconSprite name="files" :size="19" />
      </Button>
      <Button
        v-if="authenticated"
        class="cr-header-action"
        text
        rounded
        severity="secondary"
        aria-label="更多操作"
        title="更多操作"
        @click="moreMenu.toggle($event)"
      >
        <EllipsisVertical :size="20" />
      </Button>
      <Menu ref="moreMenu" :model="moreMenuItems" :popup="true" class="cr-menu-top-right">
        <template #item="{ item, props: itemProps }">
          <button type="button" v-bind="itemProps.action" :class="{ 'text-danger!': item.danger }">
            <ListChecks v-if="item.icon === 'select'" :size="17" />
            <UserRound v-else-if="item.icon === 'profile'" :size="17" />
            <EllipsisVertical v-else-if="item.icon === 'manage'" :size="17" />
            <LogOut v-else-if="item.icon === 'leave'" :size="17" />
            <UserMinus v-else-if="item.icon === 'remove'" :size="17" />
            <Ban v-else-if="item.icon === 'block'" :size="17" />
            <span>{{ item.label }}</span>
          </button>
        </template>
      </Menu>
    </div>
  </header>
</template>
