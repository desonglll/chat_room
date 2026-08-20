<script setup lang="ts">
import { ArrowLeft, Inbox, Plus, Search, ShieldOff, UserRoundCheck } from 'lucide-vue-next'
import Button from 'primevue/button'
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
  counts: Record<ContactSection, number>
  incomingCount: number
  loading: boolean
  error: string
}>()
const emit = defineEmits<{
  back: []
  newChat: []
  select: [entry: ContactEntry]
  'update:active': [section: ContactSection]
  'update:query': [query: string]
}>()

const tabs = [
  { label: '好友', value: 'friends' as const, icon: UserRoundCheck },
  { label: '申请', value: 'requests' as const, icon: Inbox },
  { label: '黑名单', value: 'blocked' as const, icon: ShieldOff },
]

function emptyLabel(): string {
  if (props.query) return '没有匹配的联系人'
  if (props.active === 'friends') return '还没有好友'
  if (props.active === 'requests') return '暂无好友申请'
  return '黑名单为空'
}
</script>

<template>
  <aside class="flex h-full min-h-0 min-w-0 flex-col border-r border-surface-200 bg-surface-0">
    <header class="flex h-[72px] shrink-0 items-center gap-2 px-3">
      <Button text rounded severity="secondary" aria-label="返回会话" title="返回会话" @click="emit('back')">
        <ArrowLeft :size="20" aria-hidden="true" />
      </Button>
      <div class="min-w-0 flex-1">
        <h1 class="truncate text-base font-semibold text-surface-900">联系人</h1>
        <p class="mt-0.5 text-xs text-muted-color">{{ counts.friends }} 位好友</p>
      </div>
      <Button text rounded aria-label="添加好友" title="添加好友" @click="emit('newChat')">
        <Plus :size="20" aria-hidden="true" />
      </Button>
    </header>

    <div class="shrink-0 px-3 pb-3">
      <IconField>
        <InputIcon><Search :size="15" aria-hidden="true" /></InputIcon>
        <InputText
          :model-value="query"
          fluid
          size="small"
          name="contact-search"
          autocomplete="off"
          placeholder="搜索联系人…"
          aria-label="搜索联系人"
          @update:model-value="emit('update:query', String($event ?? ''))"
        />
      </IconField>
    </div>

    <nav class="grid shrink-0 grid-cols-3 border-y border-surface-200 px-2 py-1.5" aria-label="联系人分类">
      <button
        v-for="tab in tabs"
        :key="tab.value"
        type="button"
        class="relative flex h-10 min-w-0 items-center justify-center gap-1.5 rounded-md px-2 text-xs font-medium transition-colors focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset"
        :class="
          active === tab.value
            ? 'bg-primary-50 text-primary-800'
            : 'text-muted-color hover:bg-surface-50 hover:text-color'
        "
        :aria-current="active === tab.value ? 'page' : undefined"
        @click="emit('update:active', tab.value)"
      >
        <component :is="tab.icon" :size="16" aria-hidden="true" />
        <span class="truncate">{{ tab.label }}</span>
        <span
          v-if="tab.value === 'requests' && incomingCount"
          class="grid min-w-5 place-items-center rounded-full bg-danger px-1 text-[10px] leading-5 text-white tabular-nums"
        >
          {{ incomingCount > 99 ? '99+' : incomingCount }}
        </span>
      </button>
    </nav>

    <Message v-if="error" severity="error" size="small" :closable="false" class="mx-3 mt-3">{{ error }}</Message>

    <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain py-2" aria-live="polite">
      <div v-if="loading" class="px-3">
        <div v-for="index in 6" :key="index" class="flex h-[72px] items-center gap-3">
          <Skeleton shape="circle" size="3rem" />
          <div class="flex-1 space-y-2">
            <Skeleton width="52%" height="0.8rem" /><Skeleton width="72%" height="0.65rem" />
          </div>
        </div>
      </div>
      <TransitionGroup v-else-if="entries.length" name="contact-list" tag="div" class="divide-y divide-surface-100">
        <ContactDirectoryRow
          v-for="entry in entries"
          :key="entry.key"
          :entry="entry"
          :selected="entry.key === selectedKey"
          @select="emit('select', entry)"
        />
      </TransitionGroup>
      <div v-else class="grid min-h-52 place-items-center px-6 text-center text-sm text-muted-color">
        <div>
          <Search v-if="query" class="mx-auto mb-3" :size="23" aria-hidden="true" />
          <Inbox v-else class="mx-auto mb-3" :size="23" aria-hidden="true" />
          {{ emptyLabel() }}
        </div>
      </div>
    </div>
  </aside>
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
