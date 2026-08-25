<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { ArrowLeft, Bookmark, File, Forward, LocateFixed, MessageSquareText, Plus, Trash2 } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import SelectButton from 'primevue/selectbutton'
import Skeleton from 'primevue/skeleton'
import Textarea from 'primevue/textarea'
import type { FavoriteForwardResult, FavoriteItem, Room } from '../types'
import { favoriteKindLabel, matchesFavoriteFilter, type FavoriteFilter } from '../favoriteView'
import MessageAttachment from './MessageAttachment.vue'

const props = defineProps<{
  items: FavoriteItem[]
  rooms: Room[]
  loading: boolean
  error: string
  create: (title: string, content: string) => Promise<FavoriteItem>
  remove: (id: string) => Promise<void>
  forward: (id: string, roomIds: string[]) => Promise<FavoriteForwardResult[]>
}>()
const emit = defineEmits<{ back: []; changed: []; success: [message: string]; error: [message: string] }>()
const router = useRouter()
const filter = ref<FavoriteFilter>('all')
const createOpen = ref(false)
const forwardItem = ref<FavoriteItem | null>(null)
const title = ref('')
const content = ref('')
const selectedRoomIds = ref<string[]>([])
const busy = ref(false)
const filters = [
  { label: '全部', value: 'all' },
  { label: '文件', value: 'file' },
  { label: '对话', value: 'message' },
  { label: '手动', value: 'manual' },
]
const visibleItems = computed(() => props.items.filter((item) => matchesFavoriteFilter(item, filter.value)))
const targetRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
}

function openSource(item: FavoriteItem): void {
  if (!item.source_room_id || !item.source_message_id) return
  void router.push({
    name: 'room',
    params: { id: item.source_room_id },
    query: { message: item.source_message_id },
  })
}

function previewAttachment(url: string): void {
  window.open(url, '_blank', 'noopener')
}

async function submitCreate(): Promise<void> {
  busy.value = true
  try {
    await props.create(title.value, content.value)
    title.value = ''
    content.value = ''
    createOpen.value = false
    emit('success', '收藏已创建')
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '创建收藏失败')
  } finally {
    busy.value = false
  }
}

async function remove(item: FavoriteItem): Promise<void> {
  if (!window.confirm('删除这条收藏？')) return
  try {
    await props.remove(item.id)
    emit('success', '收藏已删除')
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '删除收藏失败')
  }
}

function openForward(item: FavoriteItem): void {
  selectedRoomIds.value = []
  forwardItem.value = item
}

function toggleRoom(roomId: string): void {
  selectedRoomIds.value = selectedRoomIds.value.includes(roomId)
    ? selectedRoomIds.value.filter((id) => id !== roomId)
    : [...selectedRoomIds.value, roomId]
}

async function submitForward(): Promise<void> {
  if (!forwardItem.value || !selectedRoomIds.value.length) return
  busy.value = true
  try {
    const results = await props.forward(forwardItem.value.id, selectedRoomIds.value)
    const forwarded = results.filter((result) => result.forwarded_message_id).length
    if (!forwarded) throw new Error('没有可转发的目标会话')
    forwardItem.value = null
    emit('changed')
    emit('success', forwarded === results.length ? '收藏已转发' : `已转发到 ${forwarded} 个会话`)
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '转发收藏失败')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <main id="workspace-main" class="cr-page min-h-0 min-w-0 flex-1 overflow-y-auto">
    <header class="cr-page-header sticky top-0 z-10 flex items-center gap-3 px-4 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')">
        <ArrowLeft :size="19" />
      </Button>
      <div class="min-w-0 flex-1">
        <h1 class="text-base font-semibold">我的收藏</h1>
        <p class="mt-0.5 text-xs text-muted-color">{{ items.length }} 条内容</p>
      </div>
      <Button size="small" @click="createOpen = true"><Plus :size="17" /><span>新建</span></Button>
    </header>

    <div class="mx-auto w-full max-w-4xl px-4 py-5 sm:px-7">
      <SelectButton
        v-model="filter"
        :options="filters"
        option-label="label"
        option-value="value"
        :allow-empty="false"
        class="mb-5 inline-grid grid-cols-4"
      />
      <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>
      <div v-if="loading" class="divide-y divide-surface-200">
        <div v-for="index in 4" :key="index" class="space-y-3 py-5"><Skeleton width="9rem" /><Skeleton /></div>
      </div>
      <div v-else-if="!visibleItems.length" class="grid min-h-72 place-items-center text-center text-muted-color">
        <div>
          <Bookmark :size="32" class="mx-auto opacity-40" />
          <p class="mt-3 text-sm">暂无收藏</p>
        </div>
      </div>
      <ol v-else class="divide-y divide-surface-200">
        <li v-for="item in visibleItems" :key="item.id" class="py-5 first:pt-1">
          <article class="min-w-0">
            <div class="flex min-w-0 items-start gap-3">
              <span class="mt-0.5 grid size-9 shrink-0 place-items-center rounded-md bg-surface-100 text-surface-600">
                <File v-if="item.attachment" :size="18" />
                <MessageSquareText v-else :size="18" />
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                  <h2 class="break-words text-sm font-semibold text-surface-900">
                    {{ item.title || item.source_sender || favoriteKindLabel(item) }}
                  </h2>
                  <span class="text-[11px] text-muted-color"
                    >{{ favoriteKindLabel(item) }} · {{ formatDate(item.created_at) }}</span
                  >
                </div>
                <p v-if="item.source_room_name" class="mt-1 text-xs text-muted-color">
                  {{ item.source_sender }} · {{ item.source_room_name }}
                </p>
                <MessageAttachment
                  v-if="item.attachment"
                  class="mt-3"
                  :attachment="item.attachment"
                  @preview-image="previewAttachment($event.download_url)"
                />
                <p v-if="item.content" class="mt-3 whitespace-pre-wrap break-words text-sm leading-6 text-surface-800">
                  {{ item.content }}
                </p>
              </div>
              <div class="flex shrink-0 gap-1">
                <Button
                  v-if="item.source_room_id && item.source_message_id"
                  text
                  rounded
                  severity="secondary"
                  aria-label="回到原消息"
                  title="回到原消息"
                  @click="openSource(item)"
                >
                  <LocateFixed :size="17" />
                </Button>
                <Button text rounded severity="secondary" aria-label="转发收藏" title="转发" @click="openForward(item)">
                  <Forward :size="17" />
                </Button>
                <Button text rounded severity="danger" aria-label="删除收藏" title="删除" @click="remove(item)">
                  <Trash2 :size="17" />
                </Button>
              </div>
            </div>
          </article>
        </li>
      </ol>
    </div>

    <Dialog v-model:visible="createOpen" modal header="新建收藏" class="w-[min(92vw,520px)]" :draggable="false">
      <form class="space-y-4" @submit.prevent="submitCreate">
        <div>
          <label for="favorite-title" class="mb-2 block text-sm font-medium">标题</label
          ><InputText id="favorite-title" v-model="title" maxlength="120" fluid />
        </div>
        <div>
          <label for="favorite-content" class="mb-2 block text-sm font-medium">内容</label
          ><Textarea id="favorite-content" v-model="content" maxlength="8000" rows="7" auto-resize fluid />
        </div>
        <div class="flex justify-end gap-2">
          <Button type="button" label="取消" severity="secondary" text @click="createOpen = false" /><Button
            type="submit"
            label="创建"
            :loading="busy"
            :disabled="!title.trim() && !content.trim()"
          />
        </div>
      </form>
    </Dialog>

    <Dialog
      :visible="Boolean(forwardItem)"
      modal
      header="转发收藏"
      class="w-[min(92vw,420px)]"
      :draggable="false"
      @update:visible="!$event && (forwardItem = null)"
    >
      <ul class="max-h-72 space-y-1 overflow-y-auto p-0">
        <li v-for="room in targetRooms" :key="room.id">
          <label class="flex min-h-11 cursor-pointer items-center gap-2.5 rounded-md px-2 text-sm hover:bg-surface-100"
            ><Checkbox
              binary
              :model-value="selectedRoomIds.includes(room.id)"
              @update:model-value="toggleRoom(room.id)"
            /><span class="min-w-0 flex-1 truncate">{{ room.name }}</span></label
          >
        </li>
        <li v-if="!targetRooms.length" class="py-6 text-center text-sm text-muted-color">没有可转发的会话</li>
      </ul>
      <div class="mt-5 flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button label="取消" severity="secondary" text @click="forwardItem = null" /><Button
          :disabled="!selectedRoomIds.length"
          :loading="busy"
          @click="submitForward"
          ><Forward :size="17" /><span>转发</span></Button
        >
      </div>
    </Dialog>
  </main>
</template>
