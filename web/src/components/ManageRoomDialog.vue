<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Save, Trash2 } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Popover from 'primevue/popover'
import SelectButton from 'primevue/selectbutton'
import Textarea from 'primevue/textarea'
import EmojiPicker from './EmojiPicker.vue'
import IconSprite from './IconSprite.vue'
import ScopedPasswordField from './ScopedPasswordField.vue'
import { deleteRoom, updateRoom } from '../api'
import RoomMembersPanel from './RoomMembersPanel.vue'
import RoomAiPolicyPanel from './RoomAiPolicyPanel.vue'
import type { Room, RoomUpdateResult } from '../types'

const props = defineProps<{
  open: boolean
  room: Room | null
  credential: string
  token: string
}>()
const emit = defineEmits<{
  close: []
  updated: [result: RoomUpdateResult]
  deleted: [roomId: string]
}>()

const name = ref('')
const newPassword = ref('')
const joinPolicy = ref<'open' | 'approval'>('approval')
const removePassword = ref(false)
const avatarEmoji = ref('')
const description = ref('')
const mode = ref<'settings' | 'members'>('settings')
const confirmingDelete = ref(false)
const error = ref('')
const busy = ref(false)
const avatarPopover = ref()
const visible = computed({
  get: () => props.open && Boolean(props.room),
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})
const modeOptions = [
  { label: '房间设置', value: 'settings' },
  { label: '成员管理', value: 'members' },
]
const policyOptions = [
  { label: '需要审核', value: 'approval' },
  { label: '直接加入', value: 'open' },
]

watch(
  () => props.open,
  (open) => {
    if (!open || !props.room) return
    name.value = props.room.name
    newPassword.value = ''
    joinPolicy.value = props.room.join_policy
    removePassword.value = false
    avatarEmoji.value = props.room.avatar_emoji
    description.value = props.room.description
    mode.value = 'settings'
    confirmingDelete.value = false
    error.value = ''
  },
)

function selectAvatar(emoji: string): void {
  avatarEmoji.value = emoji
  avatarPopover.value?.hide()
}

async function save(): Promise<void> {
  if (!props.room) return
  const normalizedName = name.value.trim()
  if (!normalizedName) {
    error.value = '请输入房间名称'
    return
  }
  const passwordChanged = removePassword.value || newPassword.value.length > 0
  const payload: {
    name: string
    new_password?: string
    join_policy: 'open' | 'approval'
    avatar_emoji: string
    description: string
  } = {
    name: normalizedName,
    join_policy: joinPolicy.value,
    avatar_emoji: avatarEmoji.value,
    description: description.value,
  }
  if (passwordChanged) payload.new_password = removePassword.value ? '' : newPassword.value

  busy.value = true
  error.value = ''
  try {
    const room = await updateRoom(props.room.id, payload, props.token)
    emit('updated', {
      room,
      passwordChanged,
      password: room.has_password ? (passwordChanged ? newPassword.value : props.credential) : '',
    })
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '保存失败'
  } finally {
    busy.value = false
  }
}

function beginDelete(): void {
  confirmingDelete.value = true
  error.value = ''
}

async function confirmDelete(): Promise<void> {
  if (!props.room || props.room.membership_role !== 'owner') return
  const room = props.room
  busy.value = true
  error.value = ''
  try {
    await deleteRoom(room.id, props.token)
    emit('deleted', room.id)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '删除失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Dialog
    v-model:visible="visible"
    modal
    :header="confirmingDelete ? '删除聊天室' : '管理聊天室'"
    class="w-[min(92vw,460px)]"
    :draggable="false"
  >
    <template v-if="room && !confirmingDelete">
      <SelectButton
        v-model="mode"
        :options="modeOptions"
        option-label="label"
        option-value="value"
        :allow-empty="false"
        class="mb-5 grid grid-cols-2"
      />
      <RoomMembersPanel v-if="mode === 'members'" :room="room" :token="token" />
      <form v-else class="flex flex-col gap-5" autocomplete="off" @submit.prevent="save">
        <div class="flex items-center gap-3">
          <Avatar v-if="avatarEmoji" :label="avatarEmoji" shape="circle" class="bg-primary-50! text-xl!" />
          <Avatar v-else shape="circle" class="bg-surface-200! text-surface-700!"
            ><IconSprite name="rooms" :size="18"
          /></Avatar>
          <Button type="button" outlined size="small" @click="avatarPopover.toggle($event)">选择头像</Button>
          <Button v-if="avatarEmoji" type="button" text severity="secondary" size="small" @click="avatarEmoji = ''"
            >清除</Button
          >
        </div>
        <Popover ref="avatarPopover">
          <EmojiPicker @select="selectAvatar" />
        </Popover>

        <div class="flex flex-col gap-2">
          <label for="manageRoomName" class="text-sm font-medium">房间名称</label>
          <InputText
            id="manageRoomName"
            v-model="name"
            name="managed-room-name"
            maxlength="80"
            autocomplete="off"
            fluid
          />
        </div>

        <div class="flex flex-col gap-2">
          <label for="manageRoomDescription" class="text-sm font-medium">
            简介 <span class="font-normal text-muted-color">可选</span>
          </label>
          <Textarea
            id="manageRoomDescription"
            v-model="description"
            name="managed-room-description"
            autocomplete="off"
            maxlength="300"
            rows="2"
            auto-resize
            fluid
          />
        </div>

        <div class="flex flex-col gap-2">
          <label for="newRoomPassword" class="text-sm font-medium">
            新的聊天室访问密码 <span class="font-normal text-muted-color">留空则不更改</span>
          </label>
          <ScopedPasswordField
            v-model="newPassword"
            input-id="newRoomPassword"
            name="managed-room-password"
            scope="room-new"
            :disabled="removePassword"
          />
        </div>

        <div v-if="room.has_password" class="flex items-center gap-2">
          <Checkbox v-model="removePassword" input-id="removeRoomPassword" binary />
          <label for="removeRoomPassword" class="text-sm">移除密码，改为公开房间</label>
        </div>

        <div class="flex flex-col gap-2">
          <label class="text-sm font-medium">加入方式</label>
          <SelectButton
            v-model="joinPolicy"
            :options="policyOptions"
            option-label="label"
            option-value="value"
            :allow-empty="false"
            class="grid grid-cols-2"
          />
        </div>

        <RoomAiPolicyPanel v-if="room.membership_role === 'owner'" :room-id="room.id" :token="token" />

        <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>

        <div class="flex flex-col-reverse gap-2 border-t border-surface-200 pt-4 sm:flex-row sm:justify-between">
          <Button v-if="room.membership_role === 'owner'" type="button" severity="danger" outlined @click="beginDelete">
            <Trash2 :size="17" />
            <span>删除聊天室</span>
          </Button>
          <div class="flex gap-2 sm:justify-end">
            <Button
              type="button"
              label="取消"
              class="flex-1 sm:flex-none"
              severity="secondary"
              outlined
              @click="emit('close')"
            />
            <Button type="submit" class="flex-1 sm:flex-none" :loading="busy">
              <Save :size="17" />
              <span>保存</span>
            </Button>
          </div>
        </div>
      </form>
    </template>

    <template v-else-if="room && room.membership_role === 'owner'">
      <p class="text-sm leading-6 text-surface-600">
        确定删除“<strong class="text-surface-900">{{ room.name }}</strong
        >”吗？该房间的全部消息也会被删除。
      </p>
      <Message v-if="error" severity="error" size="small" :closable="false" class="mt-4">{{ error }}</Message>
      <div class="mt-6 flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button type="button" label="取消" severity="secondary" outlined @click="confirmingDelete = false" />
        <Button type="button" severity="danger" :loading="busy" @click="confirmDelete">
          <Trash2 :size="17" />
          <span>确认删除</span>
        </Button>
      </div>
    </template>
  </Dialog>
</template>
