<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { DoorOpen, Search } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Password from 'primevue/password'
import { getRoom, requestRoomJoin } from '../api'
import type { Room } from '../types'

const props = defineProps<{ open: boolean; token: string }>()
const emit = defineEmits<{
  close: []
  joined: [room: Room, password: string]
}>()

const roomId = ref('')
const password = ref('')
const room = ref<Room | null>(null)
const error = ref('')
const searching = ref(false)
const joining = ref(false)
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => { if (!value) emit('close') },
})

watch(() => props.open, (open) => {
  if (!open) return
  roomId.value = ''
  password.value = ''
  room.value = null
  error.value = ''
})

async function search(): Promise<void> {
  const id = roomId.value.trim()
  if (!id) return
  searching.value = true
  error.value = ''
  room.value = null
  try {
    room.value = await getRoom(id, props.token)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '查找聊天室失败'
  } finally {
    searching.value = false
  }
}

async function join(): Promise<void> {
  if (!room.value) return
  if (room.value.has_password && !password.value) {
    error.value = '请输入房间密码'
    return
  }
  joining.value = true
  error.value = ''
  try {
    let nextRoom = room.value
    if (room.value.membership_status !== 'active') {
      const membership = await requestRoomJoin(room.value.id, props.token, password.value)
      nextRoom = {
        ...room.value,
        membership_status: membership.status,
        membership_role: membership.role,
      }
    }
    emit('joined', nextRoom, password.value)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '加入聊天室失败'
  } finally {
    joining.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="通过 ID 加入聊天室" class="w-[min(94vw,480px)]" :draggable="false">
    <form autocomplete="off" class="space-y-4" @submit.prevent="room ? join() : search()">
      <div>
        <label for="join-room-id" class="mb-2 block text-sm font-medium">聊天室 ID</label>
        <div class="flex gap-2">
          <InputText
            id="join-room-id"
            v-model="roomId"
            name="chat-room-id"
            autocomplete="off"
            spellcheck="false"
            class="min-w-0 flex-1 font-mono text-sm"
            placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
            :disabled="!!room"
          />
          <Button v-if="!room" type="submit" :loading="searching" aria-label="查找聊天室" title="查找聊天室">
            <Search :size="17" />
          </Button>
          <Button v-else type="button" severity="secondary" outlined @click="room = null; password = ''; error = ''">重输</Button>
        </div>
      </div>

      <div v-if="room" class="flex items-center gap-3 border-y border-surface-200 py-4">
        <span class="grid size-10 shrink-0 place-items-center rounded-md bg-primary-50 text-primary"><DoorOpen :size="19" /></span>
        <div class="min-w-0">
          <strong class="block truncate text-sm">{{ room.name }}</strong>
          <small class="mt-1 block text-muted-color">{{ room.membership_status === 'pending' ? '申请待审核' : room.has_password ? '私密聊天室' : '公开聊天室' }}</small>
        </div>
      </div>

      <div v-if="room?.has_password">
        <label for="join-room-password" class="mb-2 block text-sm font-medium">房间密码</label>
        <Password
          id="join-room-password"
          v-model="password"
          name="room-access-password"
          autocomplete="off"
          :feedback="false"
          toggle-mask
          fluid
        />
      </div>

      <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>
      <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button type="button" label="取消" severity="secondary" outlined @click="emit('close')" />
        <Button v-if="room" type="submit" :loading="joining" :disabled="room.membership_status === 'pending'">
          <DoorOpen :size="17" />
          <span>{{ room.membership_status === 'active' ? '打开聊天室' : room.membership_status === 'pending' ? '等待管理员审核' : '申请加入' }}</span>
        </Button>
      </div>
    </form>
  </Dialog>
</template>
