<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Forward } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import type { FavoriteForwardResult, FavoriteItem, Room } from '../types'

const props = defineProps<{
  item: FavoriteItem | null
  rooms: Room[]
  forward: (id: string, roomIds: string[]) => Promise<FavoriteForwardResult[]>
}>()
const emit = defineEmits<{ changed: []; close: []; success: [message: string]; error: [message: string] }>()
const selectedRoomIds = ref<string[]>([])
const busy = ref(false)
const targetRooms = computed(() => props.rooms.filter((room) => room.membership_status === 'active'))
watch(
  () => props.item?.id,
  () => (selectedRoomIds.value = []),
)

function toggleRoom(roomId: string): void {
  selectedRoomIds.value = selectedRoomIds.value.includes(roomId)
    ? selectedRoomIds.value.filter((id) => id !== roomId)
    : [...selectedRoomIds.value, roomId]
}

async function submit(): Promise<void> {
  if (!props.item || !selectedRoomIds.value.length) return
  busy.value = true
  try {
    const results = await props.forward(props.item.id, selectedRoomIds.value)
    const forwarded = results.filter((result) => result.forwarded_message_id).length
    if (!forwarded) throw new Error('没有可转发的目标会话')
    emit('close')
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
  <Dialog
    :visible="Boolean(item)"
    modal
    header="转发收藏"
    class="w-[min(92vw,420px)]"
    :draggable="false"
    @update:visible="!$event && emit('close')"
  >
    <ul class="max-h-72 space-y-1 overflow-y-auto p-0">
      <li v-for="room in targetRooms" :key="room.id">
        <label class="flex min-h-11 cursor-pointer items-center gap-2.5 rounded-md px-2 text-sm hover:bg-surface-100">
          <Checkbox binary :model-value="selectedRoomIds.includes(room.id)" @update:model-value="toggleRoom(room.id)" />
          <span class="min-w-0 flex-1 truncate">{{ room.name }}</span>
        </label>
      </li>
      <li v-if="!targetRooms.length" class="py-6 text-center text-sm text-muted-color">没有可转发的会话</li>
    </ul>
    <div class="mt-5 flex justify-end gap-2 border-t border-surface-200 pt-4">
      <Button label="取消" severity="secondary" text @click="emit('close')" />
      <Button :disabled="!selectedRoomIds.length" :loading="busy" @click="submit">
        <Forward :size="17" /><span>转发</span>
      </Button>
    </div>
  </Dialog>
</template>
