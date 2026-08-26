<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { UserMinus, UserPlus } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import Select from 'primevue/select'
import Skeleton from 'primevue/skeleton'
import type { FavoriteCollaborator, FavoriteItem, SocialUser, User } from '../types'

const props = defineProps<{
  item: FavoriteItem | null
  user: User
  friends: SocialUser[]
  list: (id: string) => Promise<FavoriteCollaborator[]>
  add: (id: string, userId: string) => Promise<FavoriteCollaborator>
  remove: (id: string, userId: string) => Promise<void>
}>()
const emit = defineEmits<{ close: []; success: [message: string]; error: [message: string] }>()
const collaborators = ref<FavoriteCollaborator[]>([])
const selectedFriendId = ref('')
const loading = ref(false)
const busy = ref(false)
const availableFriends = computed(() => {
  const existing = new Set(collaborators.value.map((collaborator) => collaborator.user_id))
  return props.friends.filter((friend) => friend.id !== props.item?.owner_id && !existing.has(friend.id))
})

watch(
  () => props.item?.id,
  async (id) => {
    collaborators.value = []
    selectedFriendId.value = ''
    if (!id) return
    loading.value = true
    try {
      collaborators.value = await props.list(id)
    } catch (caught) {
      emit('close')
      emit('error', caught instanceof Error ? caught.message : '读取协作者失败')
    } finally {
      loading.value = false
    }
  },
)

async function addCollaborator(): Promise<void> {
  if (!props.item || !selectedFriendId.value) return
  busy.value = true
  try {
    const collaborator = await props.add(props.item.id, selectedFriendId.value)
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
  if (!props.item) return
  const leaving = collaborator.user_id === props.user.id
  if (!window.confirm(leaving ? '退出这条协作收藏？' : `移除 ${collaborator.display_name || collaborator.username}？`))
    return
  try {
    await props.remove(props.item.id, collaborator.user_id)
    collaborators.value = collaborators.value.filter((item) => item.user_id !== collaborator.user_id)
    if (leaving) emit('close')
    emit('success', leaving ? '已退出协作' : '协作者已移除')
  } catch (caught) {
    emit('error', caught instanceof Error ? caught.message : '移除协作者失败')
  }
}
</script>

<template>
  <Dialog
    :visible="Boolean(item)"
    modal
    header="协作成员"
    class="w-[min(92vw,500px)]"
    :draggable="false"
    @update:visible="!$event && emit('close')"
  >
    <div v-if="item" class="space-y-4">
      <div class="flex items-center gap-3 rounded-md bg-surface-100 px-3 py-2.5">
        <span class="grid size-9 shrink-0 place-items-center rounded-full bg-primary-50 text-sm text-primary">
          {{ item.owner_display_name?.[0] || item.owner_username[0] }}
        </span>
        <div class="min-w-0 flex-1">
          <p class="truncate text-sm font-medium">{{ item.owner_display_name || item.owner_username }}</p>
          <p class="text-xs text-muted-color">所有者</p>
        </div>
      </div>
      <div v-if="item.access === 'owner'" class="flex items-center gap-2">
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
      <div v-if="loading" class="space-y-2"><Skeleton v-for="i in 3" :key="i" height="2.75rem" /></div>
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
            v-if="item.access === 'owner' || collaborator.user_id === user.id"
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
</template>
