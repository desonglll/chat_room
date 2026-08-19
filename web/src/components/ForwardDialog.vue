<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Forward } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import Message from 'primevue/message'
import { forwardMessages } from '../api'
import type { Room } from '../types'

const props = defineProps<{
  open: boolean
  messageIds: string[]
  rooms: Room[]
  token: string
}>()
const emit = defineEmits<{ close: []; forwarded: [] }>()

const selectedRoomIds = ref<string[]>([])
const busy = ref(false)
const error = ref('')
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})
const targetRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))

watch(
  () => props.open,
  (open) => {
    if (!open) return
    selectedRoomIds.value = []
    error.value = ''
  },
)

function toggleRoom(roomId: string): void {
  selectedRoomIds.value = selectedRoomIds.value.includes(roomId)
    ? selectedRoomIds.value.filter((id) => id !== roomId)
    : [...selectedRoomIds.value, roomId]
}

async function confirmForward(): Promise<void> {
  if (!selectedRoomIds.value.length || !props.messageIds.length) return
  busy.value = true
  error.value = ''
  try {
    const results = await forwardMessages(props.messageIds, selectedRoomIds.value, props.token)
    const skipped = results.filter((result) => result.skipped_reason)
    if (skipped.length === results.length) {
      error.value = '转发失败：没有可转发的消息或目标房间'
      return
    }
    emit('forwarded')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '转发失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="转发消息" class="w-[min(92vw,420px)]" :draggable="false">
    <p class="mb-4 text-sm text-muted-color">选择要转发到的聊天室（已选择 {{ messageIds.length }} 条消息）</p>
    <ul class="max-h-72 space-y-1 overflow-y-auto p-0">
      <li v-for="room in targetRooms" :key="room.id">
        <label class="flex min-h-11 cursor-pointer items-center gap-2.5 rounded-md px-2 text-sm hover:bg-surface-100">
          <Checkbox binary :model-value="selectedRoomIds.includes(room.id)" @update:model-value="toggleRoom(room.id)" />
          <span class="min-w-0 flex-1 truncate">{{ room.name }}</span>
        </label>
      </li>
      <li v-if="!targetRooms.length" class="py-6 text-center text-sm text-muted-color">没有可转发的聊天室</li>
    </ul>
    <Message v-if="error" severity="error" size="small" :closable="false" class="mt-4">{{ error }}</Message>
    <div class="mt-5 flex justify-end gap-2 border-t border-surface-200 pt-4">
      <Button type="button" label="取消" severity="secondary" outlined @click="emit('close')" />
      <Button type="button" :disabled="!selectedRoomIds.length" :loading="busy" @click="confirmForward">
        <Forward :size="17" />
        <span>转发</span>
      </Button>
    </div>
  </Dialog>
</template>
