<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  ArrowLeft,
  File,
  FileText,
  Film,
  Headphones,
  Image as ImageIcon,
  LoaderCircle,
  LocateFixed,
  MessageCircle,
  Search,
  UsersRound,
  X,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Select from 'primevue/select'
import Skeleton from 'primevue/skeleton'
import { useGlobalSearch } from '../composables/useGlobalSearch'
import type { GlobalSearchContentType, GlobalSearchResult } from '../globalSearchApi'
import type { Room, SocialUser, User } from '../types'

const props = defineProps<{
  token: string
  user: User
  friends: SocialUser[]
  rooms: Room[]
}>()
const emit = defineEmits<{ back: [] }>()
const router = useRouter()
const resultList = ref<HTMLElement | null>(null)
const { filters, items, nextCursor, loading, loadingMore, searched, error, submit, loadMore } = useGlobalSearch(
  () => props.token,
)

const contentTypeOptions: Array<{ label: string; value: GlobalSearchContentType }> = [
  { label: '全部内容', value: 'all' },
  { label: '文本', value: 'text' },
  { label: '文件', value: 'file' },
  { label: '图片', value: 'image' },
  { label: '视频', value: 'video' },
  { label: '音频', value: 'audio' },
]
const roomOptions = computed(() => [
  { label: '全部会话', value: '' },
  ...props.rooms.map((room) => ({ label: room.name, value: room.id })),
])
const senderOptions = computed(() => {
  const senders = new Map<string, string>()
  senders.set(props.user.id, `${props.user.display_name || props.user.username}（我）`)
  for (const friend of props.friends) senders.set(friend.id, friend.remark || friend.display_name || friend.username)
  for (const item of items.value) if (item.sender_id) senders.set(item.sender_id, item.sender)
  return [{ label: '全部发送者', value: '' }, ...[...senders].map(([value, label]) => ({ label, value }))]
})
const invalidRange = computed(() =>
  Boolean(filters.value.from && filters.value.to && filters.value.from > filters.value.to),
)

onMounted(() => document.querySelector<HTMLInputElement>('[data-global-search-input]')?.focus())

function resetFilters(): void {
  filters.value = { ...filters.value, roomId: '', senderId: '', from: '', to: '', contentType: 'all' }
  void submit()
}

function openResult(result: GlobalSearchResult): void {
  void router.push({
    name: 'room',
    params: { id: result.room_id },
    query: { message: result.message_id },
    hash: `#message-${result.message_id}`,
  })
}

async function focusResult(index: number): Promise<void> {
  await nextTick()
  const buttons = [...(resultList.value?.querySelectorAll<HTMLButtonElement>('[data-global-search-result]') || [])]
  if (!buttons.length) return
  buttons[Math.max(0, Math.min(index, buttons.length - 1))]?.focus()
}

function resultKeydown(event: KeyboardEvent, index: number): void {
  if (event.key === 'ArrowDown') void focusResult(index + 1)
  else if (event.key === 'ArrowUp') void focusResult(index - 1)
  else if (event.key === 'Home') void focusResult(0)
  else if (event.key === 'End') void focusResult(items.value.length - 1)
  else return
  event.preventDefault()
}

function formatTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(date)
}

function resultIcon(type: GlobalSearchContentType) {
  if (type === 'image') return ImageIcon
  if (type === 'video') return Film
  if (type === 'audio') return Headphones
  if (type === 'file') return File
  return FileText
}
</script>

<template>
  <main id="workspace-main" class="cr-page flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
    <header class="cr-page-header flex shrink-0 items-center gap-3 px-3 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')">
        <ArrowLeft :size="19" aria-hidden="true" />
      </Button>
      <div class="min-w-0 flex-1">
        <h1 class="text-base font-semibold">搜索</h1>
        <p class="mt-0.5 text-xs text-muted-color" aria-live="polite">
          {{ searched ? `${items.length}${nextCursor ? '+' : ''} 条结果` : '全部会话' }}
        </p>
      </div>
    </header>

    <section class="shrink-0 border-b border-surface-200 px-4 py-4 sm:px-7" aria-label="搜索条件">
      <form class="mx-auto grid w-full max-w-5xl gap-3" @submit.prevent="submit">
        <div class="flex min-w-0 gap-2">
          <div class="relative min-w-0 flex-1">
            <Search
              :size="18"
              class="pointer-events-none absolute left-3 top-1/2 z-10 -translate-y-1/2 text-muted-color"
              aria-hidden="true"
            />
            <InputText
              v-model="filters.q"
              data-global-search-input
              maxlength="200"
              autocomplete="off"
              placeholder="搜索消息"
              aria-label="搜索消息"
              fluid
              class="h-11 pl-10!"
              @keydown.down.prevent="focusResult(0)"
            />
            <LoaderCircle
              v-if="loading"
              :size="17"
              class="absolute right-3 top-1/2 -translate-y-1/2 animate-spin text-primary motion-reduce:animate-none"
              aria-label="正在搜索"
            />
          </div>
          <Button type="submit" class="h-11! shrink-0" :disabled="!filters.q.trim() || invalidRange || loading">
            <Search :size="17" aria-hidden="true" /><span>搜索</span>
          </Button>
        </div>

        <div class="grid grid-cols-2 gap-2 lg:grid-cols-5">
          <label class="grid min-w-0 gap-1 text-xs font-medium text-surface-700">
            会话
            <Select
              v-model="filters.roomId"
              :options="roomOptions"
              option-label="label"
              option-value="value"
              filter
              fluid
            />
          </label>
          <label class="grid min-w-0 gap-1 text-xs font-medium text-surface-700">
            发送者
            <Select
              v-model="filters.senderId"
              :options="senderOptions"
              option-label="label"
              option-value="value"
              filter
              fluid
            />
          </label>
          <label class="grid min-w-0 gap-1 text-xs font-medium text-surface-700">
            类型
            <Select
              v-model="filters.contentType"
              :options="contentTypeOptions"
              option-label="label"
              option-value="value"
              fluid
            />
          </label>
          <label class="grid min-w-0 gap-1 text-xs font-medium text-surface-700">
            开始日期
            <input v-model="filters.from" type="date" class="p-inputtext h-10 min-w-0 w-full" />
          </label>
          <label class="grid min-w-0 gap-1 text-xs font-medium text-surface-700">
            结束日期
            <input v-model="filters.to" type="date" class="p-inputtext h-10 min-w-0 w-full" />
          </label>
        </div>
        <div class="flex min-h-8 items-center justify-between gap-3">
          <small v-if="invalidRange" class="text-red-600">开始日期不能晚于结束日期</small>
          <span v-else />
          <Button type="button" text size="small" severity="secondary" @click="resetFilters">
            <X :size="15" aria-hidden="true" /><span>清除筛选</span>
          </Button>
        </div>
      </form>
    </section>

    <div ref="resultList" class="min-h-0 flex-1 overflow-y-auto px-4 sm:px-7">
      <div class="mx-auto w-full max-w-5xl py-3 sm:py-5">
        <Message v-if="error" severity="error" :closable="false">
          <span>{{ error }}</span>
          <Button text size="small" severity="danger" @click="submit">重试</Button>
        </Message>
        <div v-else-if="loading" class="divide-y divide-surface-200" aria-label="正在加载搜索结果">
          <div v-for="index in 5" :key="index" class="grid grid-cols-[2.25rem_minmax(0,1fr)] gap-3 py-4">
            <Skeleton shape="circle" size="2.25rem" />
            <div class="space-y-2"><Skeleton width="12rem" /><Skeleton /></div>
          </div>
        </div>
        <div v-else-if="searched && !items.length" class="grid min-h-64 place-items-center text-muted-color">
          <div class="text-center">
            <Search :size="30" class="mx-auto opacity-40" />
            <p class="mt-3 text-sm">没有匹配消息</p>
          </div>
        </div>
        <div v-else-if="!searched" class="grid min-h-64 place-items-center text-muted-color">
          <Search :size="34" class="opacity-30" aria-hidden="true" />
        </div>
        <ol v-else class="divide-y divide-surface-200">
          <li v-for="(result, index) in items" :key="result.message_id">
            <button
              type="button"
              data-global-search-result
              class="group grid w-full grid-cols-[2.25rem_minmax(0,1fr)_auto] gap-3 px-1 py-4 text-left outline-none hover:bg-surface-50 focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-inset sm:px-3"
              :aria-label="`打开 ${result.conversation_title} 中 ${result.sender} 的消息`"
              @click="openResult(result)"
              @keydown="resultKeydown($event, index)"
            >
              <span class="grid size-9 place-items-center rounded-md bg-surface-100 text-surface-600">
                <component :is="resultIcon(result.content_type)" :size="17" aria-hidden="true" />
              </span>
              <span class="min-w-0">
                <span class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-color">
                  <strong class="max-w-full truncate text-sm text-surface-900">{{ result.conversation_title }}</strong>
                  <span class="inline-flex items-center gap-1">
                    <UsersRound v-if="result.conversation_kind === 'group'" :size="12" aria-hidden="true" />
                    <MessageCircle v-else :size="12" aria-hidden="true" />
                    {{ result.sender }}
                  </span>
                  <time>{{ formatTime(result.created_at) }}</time>
                </span>
                <span class="mt-1 block break-words text-sm leading-6 text-surface-800">{{ result.excerpt }}</span>
                <span v-if="result.attachment_file_name" class="mt-1 flex items-center gap-1 text-xs text-muted-color">
                  <File :size="13" aria-hidden="true" />{{ result.attachment_file_name }}
                </span>
                <span
                  v-if="result.context_before || result.context_after"
                  class="mt-2 block text-xs leading-5 text-muted-color"
                >
                  <span v-if="result.context_before" class="line-clamp-1">↑ {{ result.context_before }}</span>
                  <span v-if="result.context_after" class="line-clamp-1">↓ {{ result.context_after }}</span>
                </span>
              </span>
              <LocateFixed :size="17" class="mt-1 text-muted-color group-hover:text-primary" aria-hidden="true" />
            </button>
          </li>
        </ol>
        <div v-if="nextCursor && !loading" class="flex justify-center py-5">
          <Button outlined severity="secondary" :loading="loadingMore" @click="loadMore">加载更多</Button>
        </div>
      </div>
    </div>
  </main>
</template>
