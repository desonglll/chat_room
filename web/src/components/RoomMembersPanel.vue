<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Check, UserMinus, UserPlus, X } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Select from 'primevue/select'
import Tag from 'primevue/tag'
import { inviteRoomMember, listRoomMembers, updateRoomMember } from '../api'
import type { Room, RoomMembership } from '../types'

const props = defineProps<{
  room: Room
  token: string
}>()
const emit = defineEmits<{ changed: [] }>()

const members = ref<RoomMembership[]>([])
const inviteUsername = ref('')
const error = ref('')
const busy = ref('')
const pending = computed(() => members.value.filter((member) => member.status === 'pending'))
const invited = computed(() => members.value.filter((member) => member.status === 'invited'))
const active = computed(() => members.value.filter((member) => member.status === 'active'))
const roles = [
  { label: '管理员', value: 'admin' },
  { label: '成员', value: 'member' },
]

async function refresh(): Promise<void> {
  error.value = ''
  try {
    members.value = await listRoomMembers(props.room.id, props.token)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '读取成员失败'
  }
}

async function invite(): Promise<void> {
  const username = inviteUsername.value.trim()
  if (!username) return
  busy.value = 'invite'
  error.value = ''
  try {
    await inviteRoomMember(props.room.id, props.token, username)
    inviteUsername.value = ''
    await refresh()
    emit('changed')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '邀请失败'
  } finally {
    busy.value = ''
  }
}

async function update(member: RoomMembership, action: 'approve' | 'reject' | 'remove'): Promise<void> {
  busy.value = `${action}:${member.user_id}`
  error.value = ''
  try {
    await updateRoomMember(props.room.id, member.user_id, props.token, action)
    await refresh()
    emit('changed')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '成员操作失败'
  } finally {
    busy.value = ''
  }
}

async function changeRole(member: RoomMembership, role: 'admin' | 'member'): Promise<void> {
  busy.value = `role:${member.user_id}`
  error.value = ''
  try {
    await updateRoomMember(props.room.id, member.user_id, props.token, 'set_role', role)
    await refresh()
    emit('changed')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '角色更新失败'
  } finally {
    busy.value = ''
  }
}

watch(() => props.room.id, refresh)
onMounted(refresh)
</script>

<template>
  <div class="space-y-5">
    <form class="flex gap-2" @submit.prevent="invite">
      <InputText v-model="inviteUsername" class="min-w-0 flex-1" maxlength="48" placeholder="输入用户名邀请" />
      <Button type="submit" :loading="busy === 'invite'" :disabled="!inviteUsername.trim()">
        <UserPlus :size="17" />
        <span>邀请</span>
      </Button>
    </form>

    <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>

    <section v-if="pending.length">
      <div class="mb-2 flex items-center justify-between">
        <strong class="text-sm">待审核</strong>
        <Tag :value="String(pending.length)" severity="warn" />
      </div>
      <div class="divide-y divide-surface-100 border-y border-surface-200">
        <div v-for="member in pending" :key="member.user_id" class="flex min-h-14 items-center gap-3 py-2">
          <Avatar :label="member.avatar_emoji || member.username.slice(0, 1).toUpperCase()" shape="circle" class="bg-surface-200! text-surface-700!" />
          <strong class="min-w-0 flex-1 truncate text-sm">{{ member.username }}</strong>
          <Button text rounded severity="success" aria-label="批准加入" title="批准" :loading="busy === `approve:${member.user_id}`" @click="update(member, 'approve')"><Check :size="17" /></Button>
          <Button text rounded severity="danger" aria-label="拒绝加入" title="拒绝" :loading="busy === `reject:${member.user_id}`" @click="update(member, 'reject')"><X :size="17" /></Button>
        </div>
      </div>
    </section>

    <section v-if="invited.length">
      <div class="mb-2 flex items-center justify-between">
        <strong class="text-sm">已邀请</strong>
        <Tag :value="String(invited.length)" severity="info" />
      </div>
      <div class="divide-y divide-surface-100 border-y border-surface-200">
        <div v-for="member in invited" :key="member.user_id" class="flex min-h-14 items-center gap-3 py-2">
          <Avatar :label="member.avatar_emoji || member.username.slice(0, 1).toUpperCase()" shape="circle" class="bg-surface-200! text-surface-700!" />
          <strong class="min-w-0 flex-1 truncate text-sm">{{ member.username }}</strong>
          <Button text rounded severity="danger" aria-label="取消邀请" title="取消邀请" :loading="busy === `reject:${member.user_id}`" @click="update(member, 'reject')"><X :size="17" /></Button>
        </div>
      </div>
    </section>

    <section>
      <div class="mb-2 flex items-center justify-between">
        <strong class="text-sm">聊天室成员</strong>
        <span class="text-xs text-muted-color">{{ active.length }}</span>
      </div>
      <div class="max-h-64 divide-y divide-surface-100 overflow-y-auto border-y border-surface-200">
        <div v-for="member in active" :key="member.user_id" class="flex min-h-14 items-center gap-3 py-2">
          <Avatar :label="member.avatar_emoji || member.username.slice(0, 1).toUpperCase()" shape="circle" class="bg-surface-200! text-surface-700!" />
          <strong class="min-w-0 flex-1 truncate text-sm">{{ member.username }}</strong>
          <Tag v-if="member.role === 'owner'" value="创建者" severity="contrast" />
          <Select
            v-else-if="room.membership_role === 'owner'"
            :model-value="member.role"
            :options="roles"
            option-label="label"
            option-value="value"
            size="small"
            :disabled="busy === `role:${member.user_id}`"
            @update:model-value="changeRole(member, $event)"
          />
          <Tag v-else :value="member.role === 'admin' ? '管理员' : '成员'" severity="secondary" />
          <Button v-if="member.role !== 'owner'" text rounded severity="danger" aria-label="移除成员" title="移除成员" :loading="busy === `remove:${member.user_id}`" @click="update(member, 'remove')"><UserMinus :size="17" /></Button>
        </div>
      </div>
    </section>
  </div>
</template>
