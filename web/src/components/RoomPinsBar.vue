<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronRight, LocateFixed, Pencil, Pin, PinOff } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import { useToast } from 'primevue/usetoast'
import { listRoomPins, pinRoomMessage, unpinRoomMessage } from '../roomPinsApi'
import type { RoomPin } from '../types'

const props = defineProps<{ roomId: string; token: string; canPin: boolean }>()
const emit = defineEmits<{
  'update:messageIds': [ids: string[]]
  locate: [messageId: string]
  editFavorite: [favoriteId: string]
}>()
const toast = useToast()
const pins = ref<RoomPin[]>([])
const open = ref(false)
const busyId = ref('')
const messageIds = computed(() => pins.value.map((pin) => pin.message.id))

function excerpt(pin: RoomPin): string {
  return pin.message.content || pin.message.attachment?.file_name || '消息'
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
}

async function refresh(): Promise<void> {
  if (!props.roomId || !props.token) {
    pins.value = []
    return
  }
  try {
    pins.value = await listRoomPins(props.roomId, props.token)
  } catch (error) {
    pins.value = []
    toast.add({ severity: 'error', summary: error instanceof Error ? error.message : '读取置顶消息失败', life: 3200 })
  }
}

async function toggle(messageId: string): Promise<void> {
  if (!props.canPin || busyId.value) return
  busyId.value = messageId
  try {
    if (messageIds.value.includes(messageId)) await unpinRoomMessage(props.roomId, messageId, props.token)
    else await pinRoomMessage(props.roomId, messageId, props.token)
    await refresh()
  } catch (error) {
    toast.add({ severity: 'error', summary: error instanceof Error ? error.message : '更新置顶消息失败', life: 3200 })
  } finally {
    busyId.value = ''
  }
}

function locate(messageId: string): void {
  open.value = false
  emit('locate', messageId)
}

watch([() => props.roomId, () => props.token], refresh, { immediate: true })
watch(messageIds, (ids) => emit('update:messageIds', ids), { immediate: true })
defineExpose({ toggle, refresh })
</script>

<template>
  <div v-if="pins.length" class="flex h-10 shrink-0 items-center border-b border-surface-200 bg-surface-0 px-3 sm:px-5">
    <Pin :size="14" class="mr-2 shrink-0 text-primary" :fill="pins[0].message.favorite_id ? 'currentColor' : 'none'" />
    <button
      type="button"
      class="flex min-w-0 flex-1 items-center gap-2 text-left text-xs outline-none focus-visible:ring-2 focus-visible:ring-primary"
      @click="open = true"
    >
      <strong class="shrink-0">置顶</strong>
      <span class="truncate text-muted-color">{{ excerpt(pins[0]) }}</span>
      <span v-if="pins.length > 1" class="shrink-0 text-muted-color">共 {{ pins.length }} 条</span>
      <ChevronRight :size="14" class="ml-auto shrink-0 text-muted-color" />
    </button>
  </div>

  <Dialog v-model:visible="open" modal header="置顶消息" class="w-[min(92vw,600px)]" :draggable="false">
    <ul class="divide-y divide-surface-200">
      <li v-for="pin in pins" :key="pin.message.id" class="flex min-w-0 items-start gap-3 py-3 first:pt-0">
        <Pin :size="15" class="mt-1 shrink-0 text-primary" />
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-x-2 text-xs">
            <strong>{{ pin.message.sender }}</strong>
            <time class="text-muted-color">{{ formatTime(pin.pinned_at) }}</time>
          </div>
          <p class="mt-1 line-clamp-3 whitespace-pre-wrap break-words text-sm">{{ excerpt(pin) }}</p>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <Button
            v-if="pin.message.favorite_id"
            text
            rounded
            severity="secondary"
            aria-label="编辑收藏"
            title="编辑收藏"
            @click="emit('editFavorite', pin.message.favorite_id)"
            ><Pencil :size="15"
          /></Button>
          <Button
            text
            rounded
            severity="secondary"
            aria-label="定位消息"
            title="定位消息"
            @click="locate(pin.message.id)"
          >
            <LocateFixed :size="15" />
          </Button>
          <Button
            v-if="canPin"
            text
            rounded
            severity="danger"
            aria-label="取消置顶"
            title="取消置顶"
            :loading="busyId === pin.message.id"
            @click="toggle(pin.message.id)"
            ><PinOff :size="15"
          /></Button>
        </div>
      </li>
    </ul>
  </Dialog>
</template>
