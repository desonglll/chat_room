<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
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
const route = useRoute()
const router = useRouter()
const active = ref<ContactSection>(contactSection(route.query.section))
const query = ref('')
const selectedKey = ref('')
const detailOpen = ref(false)
const busyId = ref('')
const actionMenu = ref()
const actionItems = ref<MenuItem[]>([])

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
    { label: '删除好友', command: () => confirmRemove(entry.user) },
    { label: '加入黑名单', command: () => confirmBlock(entry.user) },
  ]
  actionMenu.value?.toggle(event)
}
</script>

<template>
  <main class="absolute inset-0 z-20 min-h-0 overflow-hidden bg-surface-0 md:relative md:inset-auto md:z-auto">
    <div class="grid h-full min-h-0 min-w-0 lg:grid-cols-[360px_minmax(0,1fr)]">
      <ContactDirectoryPane
        :active="active"
        :entries="visibleEntries"
        :selected-key="selectedKey"
        :query="query"
        :counts="counts"
        :incoming-count="incoming.length"
        :loading="loading"
        :error="error"
        @back="emit('back')"
        @new-chat="emit('newChat')"
        @select="selectEntry"
        @update:active="active = $event"
        @update:query="query = $event"
      />

      <ContactProfilePane
        class="absolute inset-0 z-20 transition-[transform,opacity] duration-200 ease-out motion-reduce:transition-none lg:relative lg:inset-auto lg:z-auto"
        :class="
          detailOpen
            ? 'visible translate-x-0 opacity-100'
            : 'invisible translate-x-6 opacity-0 lg:visible lg:translate-x-0 lg:opacity-100'
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
  </main>
</template>
