<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Plus } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Popover from 'primevue/popover'
import SelectButton from 'primevue/selectbutton'
import Textarea from 'primevue/textarea'
import EmojiPicker from './EmojiPicker.vue'
import IconSprite from './IconSprite.vue'
import ScopedPasswordField from './ScopedPasswordField.vue'
import { createRoom } from '../api'
import type { Room } from '../types'

const props = defineProps<{ open: boolean; token: string }>()
const emit = defineEmits<{
  close: []
  created: [room: Room, password: string]
}>()

const name = ref('')
const password = ref('')
const joinPolicy = ref<'approval' | 'open'>('open')
const avatarEmoji = ref('')
const description = ref('')
const error = ref('')
const busy = ref(false)
const avatarPopover = ref()
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})
const policyOptions = [
  { label: '需要审核', value: 'approval' },
  { label: '直接加入', value: 'open' },
]

watch(
  () => props.open,
  (open) => {
    if (!open) return
    name.value = ''
    password.value = ''
    joinPolicy.value = 'open'
    avatarEmoji.value = ''
    description.value = ''
    error.value = ''
  },
)

function selectAvatar(emoji: string): void {
  avatarEmoji.value = emoji
  avatarPopover.value?.hide()
}

async function submit(): Promise<void> {
  const normalizedName = name.value.trim()
  if (!normalizedName) {
    error.value = '请输入房间名称'
    return
  }
  busy.value = true
  error.value = ''
  try {
    const room = await createRoom(
      normalizedName,
      password.value,
      props.token,
      joinPolicy.value,
      avatarEmoji.value,
      description.value,
    )
    emit('created', room, password.value)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '创建房间失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="新建聊天室" class="w-[min(92vw,440px)]" :draggable="false">
    <form class="flex flex-col gap-5" autocomplete="off" @submit.prevent="submit">
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
        <label for="createRoomName" class="text-sm font-medium">房间名称</label>
        <InputText
          id="createRoomName"
          v-model="name"
          name="new-room-name"
          maxlength="80"
          autocomplete="off"
          placeholder="例如：产品讨论…"
          fluid
        />
      </div>

      <div class="flex flex-col gap-2">
        <label for="createRoomDescription" class="text-sm font-medium">
          简介 <span class="font-normal text-muted-color">可选</span>
        </label>
        <Textarea
          id="createRoomDescription"
          v-model="description"
          name="new-room-description"
          autocomplete="off"
          maxlength="300"
          rows="2"
          auto-resize
          fluid
        />
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

      <div class="flex flex-col gap-2">
        <label for="createRoomPassword" class="text-sm font-medium">
          聊天室访问密码 <span class="font-normal text-muted-color">可选</span>
        </label>
        <ScopedPasswordField
          v-model="password"
          input-id="createRoomPassword"
          name="new-room-password"
          scope="room-new"
        />
      </div>

      <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>

      <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button type="button" label="取消" severity="secondary" outlined @click="emit('close')" />
        <Button type="submit" :loading="busy">
          <Plus :size="17" />
          <span>创建</span>
        </Button>
      </div>
    </form>
  </Dialog>
</template>
