<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
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
  UserMinus,
  UserPlus,
  Users,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Select from 'primevue/select'
import SelectButton from 'primevue/selectbutton'
import Skeleton from 'primevue/skeleton'
import Textarea from 'primevue/textarea'
import type { FavoriteCollaborator, FavoriteForwardResult, FavoriteItem, Room, SocialUser, User } from '../types'
import { favoriteKindLabel, matchesFavoriteFilter, type FavoriteFilter } from '../favoriteView'
import FavoriteCreateDialog from './FavoriteCreateDialog.vue'
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
const editItem = ref<FavoriteItem | null>(null)
const collaborationItem = ref<FavoriteItem | null>(null)
const collaborators = ref<FavoriteCollaborator[]>([])
const selectedFriendId = ref('')
const collaboratorsLoading = ref(false)
const editTitle = ref('')
const editContent = ref('')
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
const availableFriends = computed(() => {
  const existing = new Set(collaborators.value.map((collaborator) => collaborator.user_id))
  return props.friends.filter((friend) => friend.id !== collaborationItem.value?.owner_id && !existing.has(friend.id))
})
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
  editItem.value = item
  editTitle.value = item.title
  editContent.value = item.content
}

function closeEdit(): void {
  editItem.value = null
  if (route.query.edit) void router.replace({ query: { ...route.query, edit: undefined } }).catch(() => {})
}

watch(
  () => [route.query.edit, props.items] as const,
  ([value]) => {
    const id = Array.isArray(value) ? value[0] : value
    const item = id ? props.items.find((candidate) => candidate.id === id) : null
    if (item && editItem.value?.id !== item.id) openEdit(item)
  },
  { immediate: true },
)

async function submitEdit(): Promise<void> {
  if (!editItem.value) return
  busy.value = true
  try {
    await props.update(editItem.value.id, editItem.value.version, editTitle.value, editContent.value)
    closeEdit()
    emit('success', '收藏已更新')
  } catch (caught) {
    const message = caught instanceof Error ? caught.message : '更新收藏失败'
    if (message.includes('其他协作者') && editItem.value) {
      const id = editItem.value.id
      await nextTick()
      editItem.value = props.items.find((item) => item.id === id) || null
    }
    emit('error', message)
  } finally {
    busy.value = false
  }
}

async function openCollaboration(item: FavoriteItem): Promise<void> {
  collaborationItem.value = item
  selectedFriendId.value = ''
  collaboratorsLoading.value = true
  try {
    collaborators.value = await props.listCollaborators(item.id)
  } catch (caught) {
    collaborationItem.value = null
    emit('error', caught instanceof Error ? caught.message : '读取协作者失败')
  } finally {
    collaboratorsLoading.value = false
  }
}

async function addCollaborator(): Promise<void> {
  if (!collaborationItem.value || !selectedFriendId.value) return
  busy.value = true
  try {
    const collaborator = await props.addCollaborator(collaborationItem.value.id, selectedFriendId.value)
    collaborators.value = [...collaborators.value, collaborator]
    selectedFriendId.value = ''
    emit('success', '协作者已添加')
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '添加协作者失败')
  } finally {
    busy.value = false
  }
}

async function removeCollaborator(collaborator: FavoriteCollaborator): Promise<void> {
  if (!collaborationItem.value) return
  const leaving = collaborator.user_id === props.user.id
  if (!window.confirm(leaving ? '退出这条协作收藏？' : `移除 ${collaborator.display_name || collaborator.username}？`))
    return
  try {
    await props.removeCollaborator(collaborationItem.value.id, collaborator.user_id)
    collaborators.value = collaborators.value.filter((item) => item.user_id !== collaborator.user_id)
    if (leaving) collaborationItem.value = null
    emit('success', leaving ? '已退出协作' : '协作者已移除')
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '移除协作者失败')
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

    <Dialog
      :visible="Boolean(editItem)"
      modal
      header="编辑收藏"
      class="w-[min(92vw,560px)]"
      :draggable="false"
      @update:visible="!$event && closeEdit()"
    >
      <form class="space-y-4" @submit.prevent="submitEdit">
        <div>
          <label for="favorite-edit-title" class="mb-2 block text-sm font-medium">标题</label>
          <InputText id="favorite-edit-title" v-model="editTitle" maxlength="120" fluid />
        </div>
        <div>
          <label for="favorite-edit-content" class="mb-2 block text-sm font-medium">内容</label>
          <Textarea id="favorite-edit-content" v-model="editContent" maxlength="8000" rows="9" auto-resize fluid />
        </div>
        <p class="text-xs text-muted-color">版本 {{ editItem?.version }} · 保存时会检查其他协作者的修改</p>
        <div class="flex justify-end gap-2">
          <Button type="button" label="取消" severity="secondary" text @click="closeEdit" />
          <Button
            type="submit"
            label="保存"
            :loading="busy"
            :disabled="editItem?.kind === 'manual' && !editTitle.trim() && !editContent.trim()"
          />
        </div>
      </form>
    </Dialog>

    <Dialog
      :visible="Boolean(collaborationItem)"
      modal
      header="协作成员"
      class="w-[min(92vw,500px)]"
      :draggable="false"
      @update:visible="!$event && (collaborationItem = null)"
    >
      <div v-if="collaborationItem" class="space-y-4">
        <div class="flex items-center gap-3 rounded-md bg-surface-100 px-3 py-2.5">
          <span class="grid size-9 shrink-0 place-items-center rounded-full bg-primary-50 text-sm text-primary">
            {{ collaborationItem.owner_display_name?.[0] || collaborationItem.owner_username[0] }}
          </span>
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium">
              {{ collaborationItem.owner_display_name || collaborationItem.owner_username }}
            </p>
            <p class="text-xs text-muted-color">所有者</p>
          </div>
        </div>

        <div v-if="collaborationItem.access === 'owner'" class="flex items-center gap-2">
          <Select
            v-model="selectedFriendId"
            :options="availableFriends"
            option-value="id"
            :option-label="(friend) => friend.remark || friend.display_name || friend.username"
            filter
            placeholder="选择好友"
            class="min-w-0 flex-1"
            :disabled="busy"
          />
          <Button aria-label="添加协作者" title="添加协作者" :disabled="!selectedFriendId" @click="addCollaborator">
            <UserPlus :size="17" />
          </Button>
        </div>

        <div v-if="collaboratorsLoading" class="space-y-2"><Skeleton v-for="i in 3" :key="i" height="2.75rem" /></div>
        <ul v-else class="max-h-72 divide-y divide-surface-200 overflow-y-auto p-0">
          <li
            v-for="collaborator in collaborators"
            :key="collaborator.user_id"
            class="flex min-h-12 items-center gap-3 py-2"
          >
            <span class="grid size-8 shrink-0 place-items-center rounded-full bg-surface-100 text-sm">
              {{ collaborator.avatar_emoji || collaborator.display_name?.[0] || collaborator.username[0] }}
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate text-sm font-medium">{{ collaborator.display_name || collaborator.username }}</p>
              <p class="truncate text-xs text-muted-color">@{{ collaborator.username }} · 可编辑</p>
            </div>
            <Button
              v-if="collaborationItem.access === 'owner' || collaborator.user_id === user.id"
              text
              rounded
              severity="danger"
              :aria-label="collaborator.user_id === user.id ? '退出协作' : '移除协作者'"
              :title="collaborator.user_id === user.id ? '退出协作' : '移除'"
              @click="removeCollaborator(collaborator)"
            >
              <UserMinus :size="16" />
            </Button>
          </li>
          <li v-if="!collaborators.length" class="py-7 text-center text-sm text-muted-color">尚未添加协作者</li>
        </ul>
      </div>
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
