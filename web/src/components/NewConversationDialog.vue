<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { Check, Clock3, MessageCircle, Search, UserPlus, UsersRound, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import IconField from 'primevue/iconfield'
import InputIcon from 'primevue/inputicon'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import ProgressSpinner from 'primevue/progressspinner'
import { cancelFriendRequest, searchUsers, sendFriendRequest, startDirectChat } from '../socialApi'
import type { ConversationSummary, SocialUser } from '../types'
import SocialUserRow from './SocialUserRow.vue'

const props = defineProps<{ open: boolean; token: string; friends: SocialUser[] }>()
const emit = defineEmits<{
  close: []
  opened: [conversation: ConversationSummary]
  socialChanged: []
  createGroup: []
}>()
const visible = computed({ get: () => props.open, set: (value) => !value && emit('close') })
const query = ref('')
const results = ref<SocialUser[]>([])
const searching = ref(false)
const busyId = ref('')
const error = ref('')
let timer: number | undefined
let searchVersion = 0

onBeforeUnmount(() => window.clearTimeout(timer))

watch(
  () => props.open,
  (open) => {
    if (open) return
    searchVersion += 1
    searching.value = false
    query.value = ''
    results.value = []
    error.value = ''
  },
)
watch(query, (value) => {
  window.clearTimeout(timer)
  const version = ++searchVersion
  const needle = value.trim()
  if (needle.length < 2) {
    results.value = []
    searching.value = false
    return
  }
  timer = window.setTimeout(() => void runSearch(needle, version), 300)
})

async function runSearch(needle: string, version: number): Promise<void> {
  searching.value = true
  error.value = ''
  try {
    const users = await searchUsers(needle, props.token)
    if (version === searchVersion) results.value = users
  } catch (caught) {
    if (version === searchVersion) error.value = caught instanceof Error ? caught.message : '搜索失败'
  } finally {
    if (version === searchVersion) searching.value = false
  }
}

async function openChat(userId: string): Promise<void> {
  busyId.value = userId
  error.value = ''
  try {
    emit('opened', await startDirectChat(userId, props.token))
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '无法开始私聊'
  } finally {
    busyId.value = ''
  }
}

async function requestFriend(user: SocialUser): Promise<void> {
  busyId.value = user.id
  error.value = ''
  try {
    if (user.relationship === 'outgoing') await cancelFriendRequest(user.id, props.token)
    else await sendFriendRequest(user.id, props.token)
    user.relationship =
      user.relationship === 'outgoing' ? 'none' : user.relationship === 'incoming' ? 'friend' : 'outgoing'
    emit('socialChanged')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '操作失败'
  } finally {
    busyId.value = ''
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="新对话" class="w-[min(94vw,500px)]" :draggable="false">
    <IconField class="w-full">
      <InputIcon class="text-surface-500"><Search :size="15" aria-hidden="true" /></InputIcon>
      <InputText
        v-model="query"
        name="user-search"
        autocomplete="off"
        fluid
        variant="filled"
        placeholder="搜索用户名或显示名称…"
        aria-label="搜索用户"
        class="h-10 rounded-lg! border-transparent! bg-surface-100! pl-9! hover:bg-surface-100! focus:border-primary! focus:bg-surface-0!"
      />
    </IconField>
    <Message v-if="error" severity="error" size="small" :closable="false" class="mt-3">{{ error }}</Message>
    <div class="mt-3 max-h-[min(60vh,520px)] overflow-y-auto overscroll-contain">
      <button
        type="button"
        class="flex h-14 w-full touch-manipulation items-center gap-3 border-b border-surface-100 px-1 text-left outline-none transition-colors hover:bg-surface-50 focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset motion-reduce:transition-none"
        @click="emit('createGroup')"
      >
        <span class="grid size-10 place-items-center rounded-full bg-primary text-primary-contrast"
          ><UsersRound :size="18" /></span
        ><span class="text-sm font-medium">新建群聊</span>
      </button>
      <div v-if="searching" class="grid h-24 place-items-center">
        <ProgressSpinner class="size-6!" stroke-width="5" />
      </div>
      <template v-else-if="query.trim().length >= 2">
        <SocialUserRow
          v-for="user in results"
          :key="user.id"
          :user="user"
          :subtitle="user.signature || `@${user.username}`"
        >
          <Button
            v-if="user.relationship === 'friend'"
            text
            rounded
            aria-label="发消息"
            title="发消息"
            :loading="busyId === user.id"
            @click="openChat(user.id)"
            ><MessageCircle :size="17"
          /></Button>
          <Button
            v-else-if="user.relationship === 'incoming'"
            text
            rounded
            aria-label="接受申请"
            title="接受申请"
            :loading="busyId === user.id"
            @click="requestFriend(user)"
            ><Check :size="17"
          /></Button>
          <Button
            v-else-if="user.relationship === 'outgoing'"
            text
            rounded
            severity="secondary"
            aria-label="取消申请"
            title="取消申请"
            :loading="busyId === user.id"
            @click="requestFriend(user)"
            ><Clock3 :size="17"
          /></Button>
          <Button
            v-else
            text
            rounded
            aria-label="添加好友"
            title="添加好友"
            :loading="busyId === user.id"
            @click="requestFriend(user)"
            ><UserPlus :size="17"
          /></Button>
        </SocialUserRow>
        <div v-if="!results.length" class="py-12 text-center text-sm text-muted-color">没有找到用户</div>
      </template>
      <template v-else>
        <h3 v-if="friends.length" class="mb-1 mt-4 text-xs font-semibold text-muted-color">好友</h3>
        <SocialUserRow
          v-for="user in friends"
          :key="user.id"
          :user="user"
          :subtitle="user.signature || `@${user.username}`"
        >
          <Button
            text
            rounded
            aria-label="发消息"
            title="发消息"
            :loading="busyId === user.id"
            @click="openChat(user.id)"
            ><MessageCircle :size="17"
          /></Button>
        </SocialUserRow>
        <div v-if="!friends.length" class="py-12 text-center text-sm text-muted-color">搜索用户并添加好友</div>
      </template>
    </div>
  </Dialog>
</template>
