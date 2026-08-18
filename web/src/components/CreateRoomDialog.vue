<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Plus } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Password from 'primevue/password'
import SelectButton from 'primevue/selectbutton'
import { createRoom } from '../api'
import type { Room } from '../types'

const props = defineProps<{ open: boolean; token: string }>()
const emit = defineEmits<{
  close: []
  created: [room: Room, password: string]
}>()

const name = ref('')
const password = ref('')
const joinPolicy = ref<'approval' | 'open'>('approval')
const error = ref('')
const busy = ref(false)
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => { if (!value) emit('close') },
})
const policyOptions = [
  { label: '需要审核', value: 'approval' },
  { label: '直接加入', value: 'open' },
]

watch(() => props.open, (open) => {
  if (!open) return
  name.value = ''
  password.value = ''
  joinPolicy.value = 'approval'
  error.value = ''
})

async function submit(): Promise<void> {
  const normalizedName = name.value.trim()
  if (!normalizedName) {
    error.value = '请输入房间名称'
    return
  }
  busy.value = true
  error.value = ''
  try {
    const room = await createRoom(normalizedName, password.value, props.token, joinPolicy.value)
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
    <form class="flex flex-col gap-5" @submit.prevent="submit">
      <div class="flex flex-col gap-2">
        <label for="createRoomName" class="text-sm font-medium">房间名称</label>
        <InputText id="createRoomName" v-model="name" maxlength="80" placeholder="例如：产品讨论" autofocus fluid />
      </div>

      <div class="flex flex-col gap-2">
        <label class="text-sm font-medium">加入方式</label>
        <SelectButton v-model="joinPolicy" :options="policyOptions" option-label="label" option-value="value" :allow-empty="false" class="grid grid-cols-2" />
      </div>

      <div class="flex flex-col gap-2">
        <label for="createRoomPassword" class="text-sm font-medium">
          密码 <span class="font-normal text-muted-color">可选</span>
        </label>
        <Password id="createRoomPassword" v-model="password" :feedback="false" toggle-mask fluid maxlength="256" autocomplete="new-password" />
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
