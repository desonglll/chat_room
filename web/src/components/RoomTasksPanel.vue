<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { CalendarClock, Check, ListChecks, LocateFixed, Pencil, Plus, Trash2, UserRound, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Drawer from 'primevue/drawer'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Select from 'primevue/select'
import SelectButton from 'primevue/selectbutton'
import Tag from 'primevue/tag'
import { useRoomTasks } from '../composables/useRoomTasks'
import { filterRoomTasks, type RoomTask, type RoomTaskSource, type TaskFilter, type TaskStatus } from '../tasksApi'
import type { BroadcastMessage, RoomMember } from '../types'

const props = defineProps<{
  open: boolean
  roomId: string
  token: string
  password: string
  participants: RoomMember[]
  source: BroadcastMessage | null
}>()
const emit = defineEmits<{ close: []; locate: [messageId: string] }>()
const { tasks, loading, saving, error, load, create, update, remove } = useRoomTasks({
  roomId: () => props.roomId,
  token: () => props.token,
  password: () => props.password,
})
const filter = ref<TaskFilter>('active')
const editingId = ref('')
const title = ref('')
const status = ref<TaskStatus>('open')
const assigneeId = ref<string | null>(null)
const dueAt = ref('')
const draftSource = ref<RoomTaskSource | null>(null)
const visibleTasks = computed(() => filterRoomTasks(tasks.value, filter.value))
const memberOptions = computed(() =>
  props.participants.map((member) => ({ label: member.username, value: member.user_id })),
)
const filterOptions = [
  { label: '进行中', value: 'active' },
  { label: '已完成', value: 'done' },
  { label: '全部', value: 'all' },
]
const statusOptions = [
  { label: '未开始', value: 'open' },
  { label: '进行中', value: 'in_progress' },
  { label: '已完成', value: 'done' },
  { label: '已取消', value: 'cancelled' },
]

watch(
  () => [props.open, props.source?.message_id] as const,
  ([open]) => {
    if (!open) return
    void load()
    if (props.source) beginCreate(props.source)
  },
)

function resetForm(): void {
  editingId.value = ''
  title.value = ''
  status.value = 'open'
  assigneeId.value = null
  dueAt.value = ''
  draftSource.value = null
}

function beginCreate(source?: BroadcastMessage): void {
  resetForm()
  editingId.value = 'new'
  if (!source) return
  title.value = source.content.trim().slice(0, 120)
  draftSource.value = {
    message_id: source.message_id,
    sender: source.sender,
    excerpt: source.content,
    recalled: false,
    sent_at: source.timestamp,
  }
}

function beginEdit(task: RoomTask): void {
  editingId.value = task.id
  title.value = task.title
  status.value = task.status
  assigneeId.value = task.assignee_id
  dueAt.value = localDateTime(task.due_at)
  draftSource.value = task.source
}

async function submit(): Promise<void> {
  const cleanTitle = title.value.trim()
  if (!cleanTitle) return
  const due = dueAt.value ? new Date(dueAt.value).toISOString() : null
  const editing = tasks.value.find((task) => task.id === editingId.value)
  if (editingId.value !== 'new' && !editing) {
    resetForm()
    return
  }
  const saved = editing
    ? await update(editing, {
        title: cleanTitle,
        status: status.value,
        assignee_id: assigneeId.value,
        due_at: due,
        version: editing.version,
      })
    : await create({
        title: cleanTitle,
        assignee_id: assigneeId.value,
        due_at: due,
        source_message_id: draftSource.value?.message_id || null,
      })
  if (saved) resetForm()
}

async function toggleDone(task: RoomTask): Promise<void> {
  if (!task.can_update) return
  await update(task, {
    title: task.title,
    status: task.status === 'done' ? 'open' : 'done',
    assignee_id: task.assignee_id,
    due_at: task.due_at,
    version: task.version,
  })
}

async function confirmDelete(task: RoomTask): Promise<void> {
  if (window.confirm(`删除待办“${task.title}”？`)) await remove(task)
}

function localDateTime(value: string | null): string {
  if (!value) return ''
  const date = new Date(value)
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 16)
}

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}

function statusLabel(value: TaskStatus): string {
  return statusOptions.find((option) => option.value === value)?.label || value
}
</script>

<template>
  <Drawer
    :visible="open"
    position="right"
    class="w-full! sm:w-[32rem]!"
    :dismissable="true"
    @update:visible="!$event && emit('close')"
  >
    <template #header>
      <div class="flex min-w-0 items-center gap-2">
        <ListChecks :size="20" class="shrink-0 text-primary" />
        <strong class="truncate text-base">聊天室待办</strong>
      </div>
    </template>

    <div class="flex min-h-full flex-col gap-4">
      <div class="flex items-center justify-between gap-3">
        <SelectButton
          v-model="filter"
          :options="filterOptions"
          option-label="label"
          option-value="value"
          :allow-empty="false"
          size="small"
        />
        <Button size="small" :disabled="editingId === 'new'" @click="beginCreate()">
          <Plus :size="16" />
          <span>新建</span>
        </Button>
      </div>

      <form v-if="editingId" class="space-y-3 border-y border-surface-200 py-4" @submit.prevent="submit">
        <div class="flex items-center justify-between gap-3">
          <strong class="text-sm">{{ editingId === 'new' ? '新建待办' : '编辑待办' }}</strong>
          <Button text rounded severity="secondary" aria-label="关闭编辑" title="关闭编辑" @click="resetForm">
            <X :size="17" />
          </Button>
        </div>
        <label class="block">
          <span class="mb-1 block text-xs text-muted-color">标题</span>
          <InputText v-model="title" class="w-full" maxlength="120" autofocus placeholder="需要完成什么？" />
        </label>
        <div class="grid gap-3 sm:grid-cols-2">
          <label class="block">
            <span class="mb-1 block text-xs text-muted-color">负责人</span>
            <Select
              v-model="assigneeId"
              class="w-full"
              :options="memberOptions"
              option-label="label"
              option-value="value"
              show-clear
              placeholder="暂不分配"
            />
          </label>
          <label v-if="editingId !== 'new'" class="block">
            <span class="mb-1 block text-xs text-muted-color">状态</span>
            <Select
              v-model="status"
              class="w-full"
              :options="statusOptions"
              option-label="label"
              option-value="value"
            />
          </label>
          <label class="block" :class="{ 'sm:col-span-2': editingId === 'new' }">
            <span class="mb-1 block text-xs text-muted-color">截止时间</span>
            <InputText v-model="dueAt" class="w-full" type="datetime-local" />
          </label>
        </div>
        <div v-if="draftSource" class="flex items-start gap-2 border-l-2 border-primary pl-3 text-xs">
          <div class="min-w-0 flex-1">
            <strong>来自 {{ draftSource.sender }}</strong>
            <p class="mt-0.5 line-clamp-2 break-words text-muted-color">{{ draftSource.excerpt }}</p>
          </div>
          <Button
            v-if="editingId === 'new'"
            text
            rounded
            severity="secondary"
            aria-label="移除消息来源"
            title="移除来源"
            @click="draftSource = null"
          >
            <X :size="15" />
          </Button>
        </div>
        <Button type="submit" :loading="saving === editingId" :disabled="!title.trim()" class="w-full">
          <Check :size="17" />
          <span>保存待办</span>
        </Button>
      </form>

      <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
      <div v-if="loading" class="flex flex-1 items-center justify-center py-16 text-sm text-muted-color">
        正在读取待办…
      </div>
      <div v-else-if="!visibleTasks.length" class="flex flex-1 flex-col items-center justify-center py-16 text-center">
        <ListChecks :size="30" class="mb-3 text-muted-color" />
        <strong class="text-sm">此视图暂无待办</strong>
      </div>
      <ol v-else class="divide-y divide-surface-200 border-y border-surface-200">
        <li v-for="task in visibleTasks" :key="task.id" class="flex gap-3 py-4">
          <button
            type="button"
            class="mt-0.5 grid size-6 shrink-0 place-items-center rounded border outline-none focus-visible:ring-2 focus-visible:ring-primary"
            :class="task.status === 'done' ? 'border-success bg-success text-white' : 'border-surface-300'"
            :disabled="!task.can_update || saving === task.id"
            :aria-label="task.status === 'done' ? '重新打开待办' : '标记待办完成'"
            @click="toggleDone(task)"
          >
            <Check v-if="task.status === 'done'" :size="15" />
          </button>
          <div class="min-w-0 flex-1">
            <div class="flex items-start gap-2">
              <strong
                class="min-w-0 flex-1 break-words text-sm"
                :class="{ 'line-through opacity-60': task.status === 'done' }"
              >
                {{ task.title }}
              </strong>
              <Tag :value="statusLabel(task.status)" severity="secondary" />
            </div>
            <div class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-color">
              <span v-if="task.assignee_id" class="flex items-center gap-1">
                <UserRound :size="13" />
                {{ task.assignee_name }}{{ task.assignee_active ? '' : '（已离开）' }}
              </span>
              <span v-if="task.due_at" class="flex items-center gap-1"
                ><CalendarClock :size="13" />{{ formatDate(task.due_at) }}</span
              >
              <button
                v-if="task.source && !task.source.recalled"
                type="button"
                class="flex items-center gap-1 rounded-sm text-primary outline-none hover:underline focus-visible:ring-2 focus-visible:ring-primary"
                @click="emit('locate', task.source.message_id)"
              >
                <LocateFixed :size="13" />查看原消息
              </button>
              <span v-else-if="task.source?.recalled">原消息已撤回</span>
            </div>
            <div v-if="task.can_update || task.can_delete" class="mt-2 flex justify-end gap-1">
              <Button
                v-if="task.can_update"
                text
                rounded
                severity="secondary"
                size="small"
                aria-label="编辑待办"
                title="编辑"
                @click="beginEdit(task)"
                ><Pencil :size="15"
              /></Button>
              <Button
                v-if="task.can_delete"
                text
                rounded
                severity="danger"
                size="small"
                aria-label="删除待办"
                title="删除"
                :loading="saving === task.id"
                @click="confirmDelete(task)"
                ><Trash2 :size="15"
              /></Button>
            </div>
          </div>
        </li>
      </ol>
    </div>
  </Drawer>
</template>
