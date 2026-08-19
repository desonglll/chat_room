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
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import Menu from 'primevue/menu'
import Popover from 'primevue/popover'
import { avatarColor } from '../avatarColor'
import type { ChatStatus, Room, RoomMember, UserSummary } from '../types'
import IconSprite from './IconSprite.vue'

const props = defineProps<{
  room: Room
  kind: 'group' | 'direct'
  peer: UserSummary | null
  status: ChatStatus
  statusLabel: string
  authenticated: boolean
  members: RoomMember[]
  currentUserId: string
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
const moreMenuItems = computed(() => [
  ...(props.kind === 'direct' && props.peer
    ? [{ label: '查看资料', icon: 'profile', command: () => emit('viewProfile', props.peer!.id) }]
    : []),
  { label: '多选消息', icon: 'select', command: () => emit('toggleSelection') },
  ...(props.kind === 'group' && canManage.value
    ? [{ label: '管理聊天室', icon: 'manage', command: () => emit('manage') }]
    : []),
  ...(props.kind === 'group' && props.room.membership_role !== 'owner'
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
  if (window.confirm(`删除好友“${props.room.name}”并关闭私聊？`)) emit('removeFriend')
}

function confirmBlockUser(): void {
  if (window.confirm(`拉黑“${props.room.name}”？双方将无法继续私聊。`)) emit('blockUser')
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
  <header
    class="flex h-[72px] shrink-0 items-center justify-between gap-3 border-b border-surface-200 bg-surface-0 px-3 sm:px-5"
  >
    <div class="flex min-w-0 items-center gap-2 sm:gap-3">
      <Button
        class="md:hidden"
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
        type="button"
        class="shrink-0 rounded-full"
        :class="{ 'cursor-default': kind === 'group' }"
        :aria-label="kind === 'direct' ? '查看对方资料' : undefined"
        @click="kind === 'direct' && peer && emit('viewProfile', peer.id)"
      >
        <Avatar
          :label="room.avatar_emoji || room.name.slice(0, 1).toUpperCase()"
          shape="circle"
          class="size-10! text-white!"
          :style="{ backgroundColor: avatarColor(peer?.id || room.id) }"
        />
      </button>
      <div class="min-w-0">
        <div class="flex min-w-0 items-center gap-1.5">
          <button
            type="button"
            class="truncate text-left text-[15px] font-semibold text-surface-900"
            :class="{ 'cursor-default': kind === 'group' }"
            :title="room.description || undefined"
            @click="kind === 'direct' && peer && emit('viewProfile', peer.id)"
          >
            {{ room.name }}
          </button>
          <Button
            v-if="kind === 'group'"
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
        </div>
        <div class="mt-1 flex min-w-0 items-center gap-1.5 text-xs text-muted-color">
          <span class="size-2 shrink-0 rounded-full" :class="statusColor" />
          <span class="shrink-0">{{ statusLabel }}</span>
          <button
            v-if="authenticated && kind === 'group'"
            type="button"
            class="shrink-0 cursor-pointer underline-offset-2 hover:text-primary hover:underline"
            @click="memberPopover.toggle($event)"
          >
            · {{ members.length }} 人在线
          </button>
          <span v-if="kind === 'direct' && peer" class="truncate">· @{{ peer.username }}</span>
          <span v-else class="hidden shrink-0 sm:inline">· {{ room.has_password ? '私密房间' : '公开房间' }}</span>
          <code v-if="kind === 'group'" class="hidden truncate font-mono text-[10px] lg:inline">· {{ room.id }}</code>
        </div>
      </div>
    </div>

    <div class="flex shrink-0 items-center gap-1">
      <Popover v-if="kind === 'group'" ref="memberPopover">
        <div class="w-60">
          <div class="mb-2 flex items-center justify-between border-b border-surface-200 pb-3">
            <strong class="text-sm">在线成员</strong>
            <span class="text-xs text-muted-color">{{ members.length }}</span>
          </div>
          <ul class="max-h-72 space-y-1 overflow-y-auto p-0">
            <li
              v-for="member in members"
              :key="member.user_id"
              class="group flex min-h-10 items-center gap-2.5 rounded-md px-1.5 py-1 text-sm hover:bg-surface-50"
            >
              <button
                type="button"
                class="min-w-0 cursor-pointer rounded-full"
                aria-label="查看用户资料"
                @click="emit('viewProfile', member.user_id)"
              >
                <Avatar
                  :label="member.avatar_emoji || member.username.slice(0, 1).toUpperCase()"
                  shape="circle"
                  size="small"
                  class="shrink-0 text-white!"
                  :style="{ backgroundColor: avatarColor(member.user_id) }"
                />
              </button>
              <span class="min-w-0 flex-1 truncate">{{
                member.user_id === currentUserId ? `${member.username}（你）` : member.username
              }}</span>
            </li>
          </ul>
        </div>
      </Popover>
      <Button
        v-if="kind === 'group'"
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
        text
        rounded
        severity="secondary"
        aria-label="更多操作"
        title="更多操作"
        @click="moreMenu.toggle($event)"
      >
        <EllipsisVertical :size="20" />
      </Button>
      <Menu ref="moreMenu" :model="moreMenuItems" :popup="true">
        <template #item="{ item, props: itemProps }">
          <a v-bind="itemProps.action" :class="{ 'text-danger!': item.danger }">
            <ListChecks v-if="item.icon === 'select'" :size="17" />
            <UserRound v-else-if="item.icon === 'profile'" :size="17" />
            <EllipsisVertical v-else-if="item.icon === 'manage'" :size="17" />
            <LogOut v-else-if="item.icon === 'leave'" :size="17" />
            <UserMinus v-else-if="item.icon === 'remove'" :size="17" />
            <Ban v-else-if="item.icon === 'block'" :size="17" />
            <span>{{ item.label }}</span>
          </a>
        </template>
      </Menu>
    </div>
  </header>
</template>
