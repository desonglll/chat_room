<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  ArrowLeft,
  Ban,
  Check,
  Clock3,
  Ellipsis,
  Inbox,
  MessageCircle,
  Plus,
  RotateCcw,
  Search,
  ShieldOff,
  UserRoundCheck,
  X,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import IconField from 'primevue/iconfield'
import InputIcon from 'primevue/inputicon'
import InputText from 'primevue/inputtext'
import Menu from 'primevue/menu'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import type { MenuItem } from 'primevue/menuitem'
import type { FriendRequest, SocialUser, UserSummary } from '../types'
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
const query = ref('')
const actionMenu = ref()
const actionItems = ref<MenuItem[]>([])
const tabs = [
  { label: '好友', value: 'friends' as const, icon: UserRoundCheck },
  { label: '申请', value: 'requests' as const, icon: Inbox },
  { label: '黑名单', value: 'blocked' as const, icon: ShieldOff },
]

function matches(user: UserSummary, needle: string): boolean {
  return `${user.display_name} ${user.username}`.toLowerCase().includes(needle)
}

const needle = computed(() => query.value.trim().toLowerCase())
const visibleFriends = computed(() => props.friends.filter((user) => matches(user, needle.value)))
const visibleIncoming = computed(() => props.incoming.filter((request) => matches(request.user, needle.value)))
const visibleOutgoing = computed(() => props.outgoing.filter((request) => matches(request.user, needle.value)))
const visibleBlocked = computed(() => props.blocked.filter((user) => matches(user, needle.value)))
const requestCount = computed(() => props.incoming.length + props.outgoing.length)
const activeLabel = computed(() => tabs.find((tab) => tab.value === active.value)?.label || '')
const activeCount = computed(() => {
  if (active.value === 'friends') return props.friends.length
  if (active.value === 'requests') return requestCount.value
  return props.blocked.length
})

function tabCount(value: (typeof tabs)[number]['value']): number {
  if (value === 'friends') return props.friends.length
  if (value === 'requests') return requestCount.value
  return props.blocked.length
}

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

function openFriendMenu(event: Event, user: SocialUser): void {
  actionItems.value = [
    { label: '删除好友', command: () => confirmRemove(user) },
    { label: '加入黑名单', command: () => confirmBlock(user) },
  ]
  actionMenu.value?.toggle(event)
}
</script>

<template>
  <main class="absolute inset-0 flex min-h-0 flex-col bg-surface-0 md:relative md:inset-auto">
    <header class="h-[72px] shrink-0 border-b border-surface-200 bg-surface-0 px-3 sm:px-5">
      <div class="mx-auto flex h-full w-full max-w-6xl items-center gap-3">
        <Button text rounded severity="secondary" aria-label="返回会话" title="返回会话" @click="emit('back')">
          <ArrowLeft :size="20" />
        </Button>
        <div class="min-w-0 flex-1">
          <h2 class="text-[15px] font-semibold">联系人</h2>
          <p class="mt-0.5 text-xs text-muted-color">{{ friends.length }} 位好友</p>
        </div>
        <Button size="small" @click="emit('newChat')"> <Plus :size="16" /><span>添加好友</span> </Button>
      </div>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <div class="mx-auto grid min-h-full w-full max-w-6xl md:grid-cols-[220px_minmax(0,1fr)]">
        <nav
          class="flex gap-1 overflow-x-auto border-b border-surface-200 bg-surface-50 px-3 py-2 md:flex-col md:border-r md:border-b-0 md:px-3 md:py-5"
          aria-label="联系人分类"
        >
          <button
            v-for="tab in tabs"
            :key="tab.value"
            type="button"
            class="flex h-11 min-w-28 items-center gap-3 rounded-md px-3 text-left text-sm transition-colors md:w-full"
            :class="
              active === tab.value
                ? 'bg-surface-0 font-semibold text-primary shadow-sm'
                : 'text-surface-600 hover:bg-surface-100'
            "
            :aria-current="active === tab.value ? 'page' : undefined"
            @click="active = tab.value"
          >
            <component :is="tab.icon" :size="18" />
            <span class="flex-1">{{ tab.label }}</span>
            <span
              class="min-w-5 rounded-full px-1.5 py-0.5 text-center text-[11px] tabular-nums"
              :class="
                tab.value === 'requests' && incoming.length ? 'bg-danger text-white' : 'bg-surface-200 text-muted-color'
              "
              >{{ tabCount(tab.value) }}</span
            >
          </button>
        </nav>

        <section class="min-w-0 bg-surface-0">
          <div
            class="flex flex-col gap-3 border-b border-surface-200 px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-6"
          >
            <div>
              <h3 class="text-base font-semibold">{{ activeLabel }}</h3>
              <p class="mt-0.5 text-xs text-muted-color">共 {{ activeCount }} 人</p>
            </div>
            <IconField class="w-full sm:max-w-72">
              <InputIcon><Search :size="15" /></InputIcon>
              <InputText
                v-model="query"
                fluid
                size="small"
                :placeholder="`搜索${activeLabel}`"
                :aria-label="`搜索${activeLabel}`"
              />
            </IconField>
          </div>

          <div class="px-4 py-4 sm:px-6">
            <Message v-if="error" severity="error" size="small" :closable="false" class="mb-3">{{ error }}</Message>
            <div v-if="loading" class="divide-y divide-surface-100 border-y border-surface-200">
              <div v-for="index in 5" :key="index" class="flex h-[72px] items-center gap-3 px-3">
                <Skeleton shape="circle" size="3rem" />
                <div class="flex-1 space-y-2">
                  <Skeleton width="36%" height="0.8rem" /><Skeleton width="24%" height="0.65rem" />
                </div>
              </div>
            </div>

            <template v-else-if="active === 'friends'">
              <div v-if="visibleFriends.length" class="divide-y divide-surface-100 border-y border-surface-200">
                <SocialUserRow v-for="user in visibleFriends" :key="user.id" :user="user" :subtitle="user.signature">
                  <Button
                    size="small"
                    outlined
                    :loading="busyId === user.id"
                    @click="run(user.id, () => startChat(user.id))"
                  >
                    <MessageCircle :size="16" /><span>发消息</span>
                  </Button>
                  <Button
                    text
                    rounded
                    severity="secondary"
                    aria-label="更多好友操作"
                    title="更多"
                    @click="openFriendMenu($event, user)"
                  >
                    <Ellipsis :size="18" />
                  </Button>
                </SocialUserRow>
              </div>
              <div v-else class="grid min-h-56 place-items-center text-center text-sm text-muted-color">
                <div>
                  <Search v-if="query" class="mx-auto mb-3" :size="22" /><UserRoundCheck
                    v-else
                    class="mx-auto mb-3"
                    :size="22"
                  />{{ query ? '没有匹配的好友' : '还没有好友' }}
                </div>
              </div>
            </template>

            <template v-else-if="active === 'requests'">
              <section v-if="visibleIncoming.length">
                <h4 class="mb-2 flex items-center gap-2 text-xs font-semibold text-muted-color">
                  <Inbox :size="14" />收到的申请
                </h4>
                <div class="divide-y divide-surface-100 border-y border-surface-200">
                  <SocialUserRow
                    v-for="request in visibleIncoming"
                    :key="request.user.id"
                    :user="request.user"
                    subtitle="希望添加你为好友"
                  >
                    <Button
                      size="small"
                      :loading="busyId === request.user.id"
                      @click="run(request.user.id, () => respond(request.user.id, 'accept'))"
                    >
                      <Check :size="16" /><span>接受</span>
                    </Button>
                    <Button
                      text
                      size="small"
                      severity="secondary"
                      @click="run(request.user.id, () => respond(request.user.id, 'decline'))"
                    >
                      <X :size="16" /><span>拒绝</span>
                    </Button>
                  </SocialUserRow>
                </div>
              </section>
              <section v-if="visibleOutgoing.length" :class="{ 'mt-6': visibleIncoming.length }">
                <h4 class="mb-2 flex items-center gap-2 text-xs font-semibold text-muted-color">
                  <Clock3 :size="14" />已发送
                </h4>
                <div class="divide-y divide-surface-100 border-y border-surface-200">
                  <SocialUserRow
                    v-for="request in visibleOutgoing"
                    :key="request.user.id"
                    :user="request.user"
                    subtitle="等待对方接受"
                  >
                    <Button
                      outlined
                      size="small"
                      severity="secondary"
                      :loading="busyId === request.user.id"
                      @click="run(request.user.id, () => cancelRequest(request.user.id))"
                    >
                      <X :size="16" /><span>撤回申请</span>
                    </Button>
                  </SocialUserRow>
                </div>
              </section>
              <div
                v-if="!visibleIncoming.length && !visibleOutgoing.length"
                class="grid min-h-56 place-items-center text-center text-sm text-muted-color"
              >
                <div><Inbox class="mx-auto mb-3" :size="22" />{{ query ? '没有匹配的申请' : '暂无好友申请' }}</div>
              </div>
            </template>

            <template v-else>
              <div v-if="visibleBlocked.length" class="divide-y divide-surface-100 border-y border-surface-200">
                <SocialUserRow
                  v-for="user in visibleBlocked"
                  :key="user.id"
                  :user="user"
                  subtitle="已限制私聊与好友申请"
                >
                  <Button
                    outlined
                    size="small"
                    severity="secondary"
                    :loading="busyId === user.id"
                    @click="run(user.id, () => unblockUser(user.id))"
                  >
                    <RotateCcw :size="16" /><span>取消拉黑</span>
                  </Button>
                </SocialUserRow>
              </div>
              <div v-else class="grid min-h-56 place-items-center text-center text-sm text-muted-color">
                <div><Ban class="mx-auto mb-3" :size="22" />{{ query ? '没有匹配的用户' : '黑名单为空' }}</div>
              </div>
            </template>
          </div>
        </section>
      </div>
    </div>
    <Menu ref="actionMenu" :model="actionItems" :popup="true" />
  </main>
</template>
