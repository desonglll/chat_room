<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  ArrowLeft,
  Bookmark,
  File,
  Forward,
  LocateFixed,
  MessageSquareText,
  Pencil,
  Plus,
  Trash2,
  Users,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import SelectButton from 'primevue/selectbutton'
import Skeleton from 'primevue/skeleton'
import type { FavoriteCollaborator, FavoriteForwardResult, FavoriteItem, Room, SocialUser, User } from '../types'
import { favoriteKindLabel, matchesFavoriteFilter, type FavoriteFilter } from '../favoriteView'
import FavoriteCollaborationDialog from './FavoriteCollaborationDialog.vue'
import FavoriteCreateDialog from './FavoriteCreateDialog.vue'
import FavoriteEditDialog from './FavoriteEditDialog.vue'
import FavoriteForwardDialog from './FavoriteForwardDialog.vue'
import MarkdownContent from './MarkdownContent.vue'
import MessageAttachment from './MessageAttachment.vue'

const props = defineProps<{
  items: FavoriteItem[]
  user: User
  friends: SocialUser[]
  rooms: Room[]
  loading: boolean
  error: string
  refresh: () => Promise<void>
  create: (title: string, content: string) => Promise<FavoriteItem>
  createAttachment: (file: File, title: string, content: string, maxUploadBytes: number) => Promise<FavoriteItem>
  maxUploadBytes: number
  update: (id: string, version: number, title: string, content: string) => Promise<FavoriteItem>
  remove: (id: string) => Promise<void>
  forward: (id: string, roomIds: string[]) => Promise<FavoriteForwardResult[]>
  listCollaborators: (id: string) => Promise<FavoriteCollaborator[]>
  addCollaborator: (id: string, userId: string) => Promise<FavoriteCollaborator>
  removeCollaborator: (id: string, userId: string) => Promise<void>
}>()
const emit = defineEmits<{ back: []; changed: []; success: [message: string]; error: [message: string] }>()
const route = useRoute()
const router = useRouter()
const filter = ref<FavoriteFilter>('all')
const createOpen = ref(false)
const forwardItem = ref<FavoriteItem | null>(null)
const editItemId = ref('')
const collaborationItem = ref<FavoriteItem | null>(null)
const filters = [
  { label: '全部', value: 'all' },
  { label: '文件', value: 'file' },
  { label: '对话', value: 'message' },
  { label: '手动', value: 'manual' },
]
const visibleItems = computed(() => props.items.filter((item) => matchesFavoriteFilter(item, filter.value)))
const editItem = computed(() => props.items.find((item) => item.id === editItemId.value) || null)
onMounted(() => void props.refresh())

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
}

function openSource(item: FavoriteItem): void {
  if (!item.source_room_id || !item.source_message_id) return
  void router.push({
    name: 'room',
    params: { id: item.source_room_id },
    query: { message: item.source_message_id },
    hash: `#message-${item.source_message_id}`,
  })
}

function previewAttachment(url: string): void {
  window.open(url, '_blank', 'noopener')
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

function openEdit(item: FavoriteItem): void {
  editItemId.value = item.id
}

function closeEdit(): void {
  editItemId.value = ''
  if (route.query.edit) void router.replace({ query: { ...route.query, edit: undefined } }).catch(() => {})
}

watch(
  () => [route.query.edit, props.items] as const,
  ([value]) => {
    const id = Array.isArray(value) ? value[0] : value
    const item = id ? props.items.find((candidate) => candidate.id === id) : null
    if (item && editItemId.value !== item.id) openEdit(item)
  },
  { immediate: true },
)

function openCollaboration(item: FavoriteItem): void {
  collaborationItem.value = item
}

function openForward(item: FavoriteItem): void {
  forwardItem.value = item
}
</script>

<template>
  <main id="workspace-main" class="cr-page min-h-0 min-w-0 flex-1 overflow-y-auto">
    <header class="cr-page-header sticky top-0 z-10 flex items-center gap-3 px-4 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')">
        <ArrowLeft :size="19" />
      </Button>
      <div class="min-w-0 flex-1">
        <h1 class="text-base font-semibold">收藏库</h1>
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
        class="mb-5 grid w-full grid-cols-4 sm:inline-grid sm:w-auto"
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
            <div class="flex min-w-0 flex-wrap items-start gap-3 sm:flex-nowrap">
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
                <p class="mt-1 text-[11px] text-muted-color">
                  {{
                    item.access === 'owner' ? '我的收藏' : `由 ${item.owner_display_name || item.owner_username} 共享`
                  }}
                  <template v-if="item.collaborator_count"> · {{ item.collaborator_count }} 人协作</template>
                </p>
                <MessageAttachment
                  v-if="item.attachment"
                  class="mt-3"
                  :attachment="item.attachment"
                  @preview-image="previewAttachment($event.download_url)"
                />
                <MarkdownContent
                  v-if="item.content"
                  :content="item.content"
                  class="mt-3 text-sm leading-6 text-surface-800"
                />
              </div>
              <div class="ml-12 flex w-full shrink-0 justify-end gap-1 sm:ml-0 sm:w-auto">
                <Button text rounded severity="secondary" aria-label="编辑收藏" title="编辑" @click="openEdit(item)">
                  <Pencil :size="17" />
                </Button>
                <Button
                  text
                  rounded
                  severity="secondary"
                  aria-label="管理协作者"
                  title="协作者"
                  @click="openCollaboration(item)"
                >
                  <Users :size="17" />
                </Button>
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
                <Button
                  v-if="item.access === 'owner'"
                  text
                  rounded
                  severity="danger"
                  aria-label="删除收藏"
                  title="删除"
                  @click="remove(item)"
                >
                  <Trash2 :size="17" />
                </Button>
              </div>
            </div>
          </article>
        </li>
      </ol>
    </div>

    <FavoriteCreateDialog
      v-model:visible="createOpen"
      :create="create"
      :create-attachment="createAttachment"
      :max-upload-bytes="maxUploadBytes"
      @success="emit('success', $event)"
      @error="emit('error', $event)"
    />

    <FavoriteEditDialog
      :item="editItem"
      :update="update"
      @close="closeEdit"
      @success="emit('success', $event)"
      @error="emit('error', $event)"
    />
    <FavoriteCollaborationDialog
      :item="collaborationItem"
      :user="user"
      :friends="friends"
      :list="listCollaborators"
      :add="addCollaborator"
      :remove="removeCollaborator"
      @close="collaborationItem = null"
      @success="emit('success', $event)"
      @error="emit('error', $event)"
    />
    <FavoriteForwardDialog
      :item="forwardItem"
      :rooms="rooms"
      :forward="forward"
      @close="forwardItem = null"
      @changed="emit('changed')"
      @success="emit('success', $event)"
      @error="emit('error', $event)"
    />
  </main>
</template>
