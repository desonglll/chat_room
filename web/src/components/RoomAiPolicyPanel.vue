<script setup lang="ts">
import { ref, watch } from 'vue'
import { Bot, Save } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import SelectButton from 'primevue/selectbutton'
import { getRoomAiPolicy, updateRoomAiPolicy, type RoomAiMode, type RoomAiPolicy } from '../roomAiPolicyApi'

const props = defineProps<{ roomId: string; token: string }>()
const policy = ref<RoomAiPolicy | null>(null)
const mode = ref<RoomAiMode>('members')
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const saved = ref(false)
const options = [
  { label: '禁用', value: 'disabled' },
  { label: '成员可用', value: 'members' },
  { label: '仅管理员', value: 'admins' },
]

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    policy.value = await getRoomAiPolicy(props.roomId, props.token)
    mode.value = policy.value.mode
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '读取 AI 策略失败'
  } finally {
    loading.value = false
  }
}

async function save(): Promise<void> {
  if (!policy.value) return
  saving.value = true
  saved.value = false
  error.value = ''
  try {
    policy.value = await updateRoomAiPolicy(props.roomId, props.token, mode.value, policy.value.version)
    saved.value = true
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '保存 AI 策略失败'
  } finally {
    saving.value = false
  }
}

watch(() => props.roomId, load, { immediate: true })
</script>

<template>
  <fieldset class="border-t border-surface-200 pt-5" :disabled="loading || saving">
    <legend class="flex items-center gap-2 pr-3 text-sm font-medium"><Bot :size="17" />AI 权限</legend>
    <div class="mt-3 flex flex-col gap-3">
      <SelectButton
        v-model="mode"
        :options="options"
        option-label="label"
        option-value="value"
        :allow-empty="false"
        class="grid grid-cols-3"
      />
      <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
      <Message v-else-if="saved" severity="success" size="small" closable @close="saved = false">策略已保存</Message>
      <div class="flex justify-end">
        <Button type="button" size="small" :loading="saving" :disabled="!policy || mode === policy.mode" @click="save">
          <Save :size="16" />保存 AI 权限
        </Button>
      </div>
    </div>
  </fieldset>
</template>
