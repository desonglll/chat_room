<script setup lang="ts">
import { ref, watch } from 'vue'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import type { SocialUser } from '../types'

const props = defineProps<{ user: SocialUser | null; save: (userId: string, remark: string) => Promise<void> }>()
const emit = defineEmits<{ close: []; saved: [] }>()
const remark = ref('')
const saving = ref(false)
const error = ref('')
watch(
  () => props.user,
  (user) => {
    remark.value = user?.remark || ''
    error.value = ''
  },
)

async function submit(): Promise<void> {
  if (!props.user) return
  saving.value = true
  error.value = ''
  try {
    await props.save(props.user.id, remark.value.trim())
    emit('saved')
    emit('close')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '保存备注失败'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <Dialog
    :visible="Boolean(user)"
    modal
    header="好友备注"
    class="w-[min(92vw,420px)]"
    :draggable="false"
    @update:visible="!$event && emit('close')"
  >
    <form class="space-y-4" @submit.prevent="submit">
      <div>
        <label for="friend-remark" class="mb-2 block text-sm font-medium">备注名</label>
        <InputText id="friend-remark" v-model="remark" maxlength="64" fluid autofocus />
        <small class="mt-1 block text-right text-muted-color">{{ remark.length }}/64</small>
      </div>
      <p v-if="error" class="text-sm text-danger">{{ error }}</p>
      <div class="flex justify-end gap-2">
        <Button type="button" label="取消" severity="secondary" text @click="emit('close')" />
        <Button type="submit" label="保存" :loading="saving" />
      </div>
    </form>
  </Dialog>
</template>
