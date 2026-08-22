<script setup lang="ts">
import { computed } from 'vue'
import { Inbox, Search, ShieldOff, UsersRound } from 'lucide-vue-next'
import IconField from 'primevue/iconfield'
import InputIcon from 'primevue/inputicon'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import type { ContactEntry, ContactSection } from '../contactDirectory'
import ContactDirectoryRow from './ContactDirectoryRow.vue'

const props = defineProps<{
  active: ContactSection
  entries: ContactEntry[]
  selectedKey: string
  query: string
  count: number
  loading: boolean
  error: string
  busyId: string
}>()
const emit = defineEmits<{
  select: [entry: ContactEntry]
  'update:query': [query: string]
  message: [userId: string]
  accept: [userId: string]
  decline: [userId: string]
  cancel: [userId: string]
  unblock: [userId: string]
  menu: [event: Event, entry: ContactEntry]
}>()

const sectionTitle = computed(() => {
  if (props.active === 'friends') return '全部好友'
  if (props.active === 'requests') return '好友申请'
  return '黑名单'
})
const groups = computed(() => {
  if (props.active !== 'requests') return [{ key: props.active, label: '', entries: props.entries }]
  return [
    { key: 'incoming', label: '收到的申请', entries: props.entries.filter((entry) => entry.kind === 'incoming') },
    { key: 'outgoing', label: '已发出的申请', entries: props.entries.filter((entry) => entry.kind === 'outgoing') },
  ].filter((group) => group.entries.length)
})

function emptyLabel(): string {
  if (props.query) return '没有匹配的联系人'
  if (props.active === 'friends') return '还没有好友'
  if (props.active === 'requests') return '暂无好友申请'
  return '黑名单为空'
}
</script>

<template>
  <section class="flex h-full min-h-0 min-w-0 flex-col bg-surface-0" aria-labelledby="contact-section-title">
    <div class="shrink-0 border-b border-surface-200 px-4 py-3 sm:px-5">
      <div class="flex w-full flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 id="contact-section-title" class="text-base font-semibold text-surface-900">{{ sectionTitle }}</h2>
          <p class="mt-0.5 text-xs text-muted-color tabular-nums">共 {{ count }} 人</p>
        </div>
        <IconField class="w-full sm:max-w-72">
          <InputIcon><Search :size="15" aria-hidden="true" /></InputIcon>
          <InputText
            :model-value="query"
            fluid
            size="small"
            class="text-base! sm:text-sm!"
            name="contact-search"
            autocomplete="off"
            placeholder="搜索姓名或用户名…"
            aria-label="搜索联系人"
            @update:model-value="emit('update:query', String($event ?? ''))"
          />
        </IconField>
      </div>
    </div>

    <Message v-if="error" severity="error" size="small" :closable="false" class="mx-4 mt-3 sm:mx-6">{{
      error
    }}</Message>

    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain" aria-live="polite">
      <div v-if="loading" class="w-full px-3 py-2 sm:px-4">
        <div v-for="index in 7" :key="index" class="flex h-[72px] items-center gap-3 border-b border-surface-100">
          <Skeleton shape="circle" size="2.75rem" />
          <div class="min-w-0 flex-1 space-y-2">
            <Skeleton width="32%" height="0.8rem" /><Skeleton width="48%" height="0.65rem" />
          </div>
          <Skeleton width="5rem" height="2.25rem" border-radius="0.5rem" />
        </div>
      </div>

      <div v-else-if="entries.length" class="w-full px-2 py-2 sm:px-3">
        <section v-for="group in groups" :key="group.key" class="mb-5 last:mb-0">
          <h3 v-if="group.label" class="px-2 pb-2 text-xs font-semibold text-muted-color">
            {{ group.label }}
            <span class="ml-1 font-normal tabular-nums">{{ group.entries.length }}</span>
          </h3>
          <TransitionGroup name="contact-list" tag="ul" class="divide-y divide-surface-100">
            <ContactDirectoryRow
              v-for="entry in group.entries"
              :key="entry.key"
              :entry="entry"
              :selected="entry.key === selectedKey"
              :busy="busyId === entry.user.id"
              @select="emit('select', entry)"
              @message="emit('message', entry.user.id)"
              @accept="emit('accept', entry.user.id)"
              @decline="emit('decline', entry.user.id)"
              @cancel="emit('cancel', entry.user.id)"
              @unblock="emit('unblock', entry.user.id)"
              @menu="emit('menu', $event, entry)"
            />
          </TransitionGroup>
        </section>
      </div>

      <div v-else class="grid min-h-64 place-items-center px-6 text-center text-sm text-muted-color">
        <div>
          <Search v-if="query" class="mx-auto mb-3" :size="24" aria-hidden="true" />
          <ShieldOff v-else-if="active === 'blocked'" class="mx-auto mb-3" :size="24" aria-hidden="true" />
          <Inbox v-else-if="active === 'requests'" class="mx-auto mb-3" :size="24" aria-hidden="true" />
          <UsersRound v-else class="mx-auto mb-3" :size="24" aria-hidden="true" />
          {{ emptyLabel() }}
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.contact-list-enter-active,
.contact-list-leave-active,
.contact-list-move {
  transition:
    opacity var(--cr-motion-normal) var(--cr-ease-out),
    transform var(--cr-motion-normal) var(--cr-ease-out);
}
.contact-list-enter-from,
.contact-list-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
@media (prefers-reduced-motion: reduce) {
  .contact-list-enter-active,
  .contact-list-leave-active,
  .contact-list-move {
    transition: none;
  }
}
</style>
