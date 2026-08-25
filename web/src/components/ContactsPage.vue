<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, Inbox, ShieldOff, UserPlus, UsersRound } from 'lucide-vue-next'
import Button from 'primevue/button'
import Menu from 'primevue/menu'
import type { MenuItem } from 'primevue/menuitem'
import {
  contactEntries,
  contactSection,
  filterContactEntries,
  type ContactEntry,
  type ContactSection,
} from '../contactDirectory'
import type { FriendRequest, SocialUser, UserSummary } from '../types'
import ContactDirectoryPane from './ContactDirectoryPane.vue'
import ContactProfilePane from './ContactProfilePane.vue'
import FriendRemarkDialog from './FriendRemarkDialog.vue'

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
  setRemark: (userId: string, remark: string) => Promise<void>
}>()
const emit = defineEmits<{ back: []; newChat: []; changed: []; error: [message: string] }>()
const route = useRoute()
const router = useRouter()
const active = ref<ContactSection>(contactSection(route.query.section))
const query = ref('')
const selectedKey = ref('')
const detailOpen = ref(false)
const busyId = ref('')
const actionMenu = ref()
const actionItems = ref<MenuItem[]>([])
const remarkUser = ref<SocialUser | null>(null)

const tabs = [
  { label: '全部好友', compactLabel: '好友', value: 'friends' as const, icon: UsersRound },
  { label: '申请', compactLabel: '申请', value: 'requests' as const, icon: Inbox },
  { label: '黑名单', compactLabel: '黑名单', value: 'blocked' as const, icon: ShieldOff },
]
const entries = computed(() =>
  contactEntries(active.value, props.friends, props.incoming, props.outgoing, props.blocked),
)
const visibleEntries = computed(() => filterContactEntries(entries.value, query.value))
const selectedEntry = computed(() => entries.value.find((entry) => entry.key === selectedKey.value) || null)
const counts = computed<Record<ContactSection, number>>(() => ({
  friends: props.friends.length,
  requests: props.incoming.length + props.outgoing.length,
  blocked: props.blocked.length,
}))

watch(
  entries,
  (next) => {
    if (!next.some((entry) => entry.key === selectedKey.value)) selectedKey.value = next[0]?.key || ''
  },
  { immediate: true },
)

watch(active, () => {
  query.value = ''
  detailOpen.value = false
  const section = active.value === 'friends' ? undefined : active.value
  void router.replace({ query: { ...route.query, section } }).catch(() => {})
})

watch(
  () => route.query.section,
  (section) => {
    active.value = contactSection(section)
  },
)

function selectEntry(entry: ContactEntry): void {
  selectedKey.value = entry.key
  detailOpen.value = true
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

function confirmRemove(user: UserSummary): void {
  if (window.confirm(`删除好友“${user.display_name || user.username}”并关闭私聊？`)) {
    void run(user.id, () => props.removeFriend(user.id))
  }
}

function confirmBlock(user: UserSummary): void {
  if (window.confirm(`拉黑“${user.display_name || user.username}”？双方将无法继续私聊。`)) {
    void run(user.id, () => props.blockUser(user.id))
  }
}

function openFriendMenu(event: Event, entry: ContactEntry): void {
  actionItems.value = [
    {
      label: entry.kind === 'friend' && 'remark' in entry.user && entry.user.remark ? '修改备注' : '设置备注',
      command: () => (remarkUser.value = entry.user as SocialUser),
    },
    { label: '删除好友', command: () => confirmRemove(entry.user) },
    { label: '加入黑名单', command: () => confirmBlock(entry.user) },
  ]
  actionMenu.value?.toggle(event)
}
</script>

<template>
  <main id="workspace-main" class="cr-page cr-contacts-page flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
    <header
      class="cr-page-header grid shrink-0 grid-cols-[minmax(0,1fr)_auto] items-center gap-x-2 px-3 py-2 lg:flex lg:gap-2 lg:px-4 lg:py-0"
    >
      <div class="flex min-w-0 items-center gap-2">
        <Button
          text
          rounded
          severity="secondary"
          class="size-11! touch-manipulation lg:size-10!"
          aria-label="返回会话"
          title="返回会话"
          @click="emit('back')"
        >
          <ArrowLeft :size="20" aria-hidden="true" />
        </Button>
        <div class="min-w-0">
          <h1 class="truncate text-base font-semibold text-surface-900">联系人</h1>
          <p class="truncate text-xs text-muted-color">{{ counts.friends }} 位好友</p>
        </div>
      </div>

      <nav
        class="cr-page-tabs order-3 col-span-2 mt-2 flex min-w-0 items-center gap-0.5 overflow-x-auto lg:order-none lg:col-span-1 lg:mt-0 lg:ml-2"
        aria-label="联系人分类"
      >
        <button
          v-for="tab in tabs"
          :key="tab.value"
          type="button"
          class="cr-page-tab relative flex h-10 shrink-0 touch-manipulation items-center gap-2 rounded-md px-3 text-sm font-medium focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
          :class="active === tab.value ? 'cr-page-tab--active' : 'cr-page-tab--idle'"
          :aria-current="active === tab.value ? 'page' : undefined"
          @click="active = tab.value"
        >
          <component :is="tab.icon" :size="16" aria-hidden="true" />
          <span class="hidden lg:inline">{{ tab.label }}</span>
          <span class="lg:hidden">{{ tab.compactLabel }}</span>
          <span
            v-if="tab.value === 'requests' && incoming.length"
            class="grid min-w-5 place-items-center rounded-full bg-danger px-1 text-[10px] leading-5 text-white tabular-nums"
            :aria-label="`${incoming.length} 个待处理申请`"
          >
            {{ incoming.length > 99 ? '99+' : incoming.length }}
          </span>
        </button>
      </nav>

      <Button
        size="small"
        class="min-h-11! touch-manipulation lg:min-h-9!"
        aria-label="添加好友"
        @click="emit('newChat')"
      >
        <UserPlus :size="17" aria-hidden="true" />
        <span class="hidden xl:inline">添加好友</span>
      </Button>
    </header>

    <div class="cr-contacts-workspace relative grid min-h-0 min-w-0 flex-1 xl:grid-cols-[minmax(0,1fr)_360px]">
      <ContactDirectoryPane
        :active="active"
        :entries="visibleEntries"
        :selected-key="selectedKey"
        :query="query"
        :count="counts[active]"
        :loading="loading"
        :error="error"
        :busy-id="busyId"
        @select="selectEntry"
        @update:query="query = $event"
        @message="(id) => run(id, () => startChat(id))"
        @accept="(id) => run(id, () => respond(id, 'accept'))"
        @decline="(id) => run(id, () => respond(id, 'decline'))"
        @cancel="(id) => run(id, () => cancelRequest(id))"
        @unblock="(id) => run(id, () => unblockUser(id))"
        @menu="openFriendMenu"
      />

      <ContactProfilePane
        class="absolute inset-0 z-20 transition-[transform,opacity] duration-[var(--cr-motion-enter)] [transition-timing-function:var(--cr-ease-drawer)] motion-reduce:transition-none xl:relative xl:inset-auto xl:z-auto"
        :class="
          detailOpen
            ? 'visible translate-x-0 opacity-100'
            : 'invisible translate-x-5 opacity-0 xl:visible xl:translate-x-0 xl:opacity-100'
        "
        :entry="selectedEntry"
        :busy="busyId === selectedEntry?.user.id"
        @close="detailOpen = false"
        @message="(id) => run(id, () => startChat(id))"
        @accept="(id) => run(id, () => respond(id, 'accept'))"
        @decline="(id) => run(id, () => respond(id, 'decline'))"
        @cancel="(id) => run(id, () => cancelRequest(id))"
        @unblock="(id) => run(id, () => unblockUser(id))"
        @menu="openFriendMenu"
      />
    </div>
    <Menu ref="actionMenu" :model="actionItems" :popup="true" />
    <FriendRemarkDialog :user="remarkUser" :save="setRemark" @close="remarkUser = null" @saved="emit('changed')" />
  </main>
</template>
