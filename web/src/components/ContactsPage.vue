<script setup lang="ts">
import { ref } from 'vue'
import { ArrowLeft, Ban, Check, MessageCircle, Plus, RotateCcw, UserMinus, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import SelectButton from 'primevue/selectbutton'
import Skeleton from 'primevue/skeleton'
import type { FriendRequest, SocialUser } from '../types'
import SocialUserRow from './SocialUserRow.vue'

const props = defineProps<{
  friends: SocialUser[]
  incoming: FriendRequest[]
  outgoing: FriendRequest[]
  blocked: SocialUser[]
  loading: boolean
  error: string
  startChat: (userId: string) => Promise<void>
  respond: (userId: string, action: 'accept' | 'decline') => Promise<void>
  cancelRequest: (userId: string) => Promise<void>
  removeFriend: (userId: string) => Promise<void>
  blockUser: (userId: string) => Promise<void>
  unblockUser: (userId: string) => Promise<void>
}>()
const emit = defineEmits<{ back: []; newChat: []; error: [message: string] }>()
const active = ref<'friends' | 'requests' | 'blocked'>('friends')
const busyId = ref('')
const tabs = [
  { label: '好友', value: 'friends' },
  { label: '申请', value: 'requests' },
  { label: '黑名单', value: 'blocked' },
]

async function run(userId: string, action: () => Promise<void>): Promise<void> {
  busyId.value = userId
  try {
    await action()
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '操作失败')
  } finally {
    busyId.value = ''
  }
}

function confirmRemove(user: SocialUser): void {
  if (window.confirm(`删除好友“${user.display_name || user.username}”并关闭私聊？`)) {
    void run(user.id, () => props.removeFriend(user.id))
  }
}

function confirmBlock(user: SocialUser): void {
  if (window.confirm(`拉黑“${user.display_name || user.username}”？双方将无法继续私聊。`)) {
    void run(user.id, () => props.blockUser(user.id))
  }
}
</script>

<template>
  <main class="absolute inset-0 flex min-h-0 flex-col bg-surface-50 md:relative md:inset-auto">
    <header class="flex h-[72px] shrink-0 items-center gap-3 border-b border-surface-200 bg-surface-0 px-3 sm:px-5">
      <Button text rounded severity="secondary" aria-label="返回会话" title="返回会话" @click="emit('back')"
        ><ArrowLeft :size="20"
      /></Button>
      <div class="min-w-0 flex-1">
        <h2 class="text-[15px] font-semibold">联系人</h2>
        <p class="mt-0.5 text-xs text-muted-color">{{ friends.length }} 位好友</p>
      </div>
      <Button size="small" @click="emit('newChat')"
        ><Plus :size="16" /><span class="hidden sm:inline">添加好友</span></Button
      >
    </header>

    <div class="shrink-0 border-b border-surface-200 bg-surface-0 px-4 py-3">
      <SelectButton
        v-model="active"
        :options="tabs"
        option-label="label"
        option-value="value"
        :allow-empty="false"
        aria-label="联系人分类"
      />
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto">
      <div class="mx-auto w-full max-w-3xl px-4 py-4 sm:px-6">
        <Message v-if="error" severity="error" size="small" :closable="false" class="mb-3">{{ error }}</Message>
        <div v-if="loading" class="space-y-3">
          <div v-for="index in 5" :key="index" class="flex h-16 items-center gap-3">
            <Skeleton shape="circle" size="2.75rem" />
            <div class="flex-1 space-y-2">
              <Skeleton width="36%" height="0.8rem" /><Skeleton width="24%" height="0.65rem" />
            </div>
          </div>
        </div>

        <template v-else-if="active === 'friends'">
          <div v-if="!friends.length" class="py-24 text-center text-sm text-muted-color">还没有好友</div>
          <SocialUserRow
            v-for="user in friends"
            :key="user.id"
            :user="user"
            :subtitle="user.signature || `@${user.username}`"
          >
            <Button
              text
              rounded
              severity="secondary"
              aria-label="发消息"
              title="发消息"
              :loading="busyId === user.id"
              @click="run(user.id, () => startChat(user.id))"
              ><MessageCircle :size="17"
            /></Button>
            <Button
              text
              rounded
              severity="secondary"
              aria-label="删除好友"
              title="删除好友"
              @click="confirmRemove(user)"
              ><UserMinus :size="17"
            /></Button>
            <Button text rounded severity="danger" aria-label="拉黑" title="拉黑" @click="confirmBlock(user)"
              ><Ban :size="17"
            /></Button>
          </SocialUserRow>
        </template>

        <template v-else-if="active === 'requests'">
          <section v-if="incoming.length">
            <h3 class="mb-2 text-xs font-semibold text-muted-color">收到的申请</h3>
            <SocialUserRow v-for="request in incoming" :key="request.user.id" :user="request.user">
              <Button
                rounded
                size="small"
                aria-label="接受"
                title="接受"
                :loading="busyId === request.user.id"
                @click="run(request.user.id, () => respond(request.user.id, 'accept'))"
                ><Check :size="16"
              /></Button>
              <Button
                text
                rounded
                severity="secondary"
                aria-label="拒绝"
                title="拒绝"
                @click="run(request.user.id, () => respond(request.user.id, 'decline'))"
                ><X :size="16"
              /></Button>
            </SocialUserRow>
          </section>
          <section v-if="outgoing.length" class="mt-6">
            <h3 class="mb-2 text-xs font-semibold text-muted-color">已发送</h3>
            <SocialUserRow
              v-for="request in outgoing"
              :key="request.user.id"
              :user="request.user"
              subtitle="等待对方接受"
            >
              <Button
                text
                rounded
                severity="secondary"
                aria-label="取消申请"
                title="取消申请"
                :loading="busyId === request.user.id"
                @click="run(request.user.id, () => cancelRequest(request.user.id))"
                ><X :size="16"
              /></Button>
            </SocialUserRow>
          </section>
          <div v-if="!incoming.length && !outgoing.length" class="py-24 text-center text-sm text-muted-color">
            暂无好友申请
          </div>
        </template>

        <template v-else>
          <div v-if="!blocked.length" class="py-24 text-center text-sm text-muted-color">黑名单为空</div>
          <SocialUserRow v-for="user in blocked" :key="user.id" :user="user">
            <Button
              text
              rounded
              severity="secondary"
              aria-label="取消拉黑"
              title="取消拉黑"
              :loading="busyId === user.id"
              @click="run(user.id, () => unblockUser(user.id))"
              ><RotateCcw :size="16"
            /></Button>
          </SocialUserRow>
        </template>
      </div>
    </div>
  </main>
</template>
