<script setup lang="ts">
import { ref, watch } from 'vue'
import { LockKeyhole, LockOpen } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import Message from 'primevue/message'
import { AdminApiError, getAdminRoomLock, setAdminRoomLock } from '../adminApi'

const props = defineProps<{ roomId: string; token: string }>()
const authorized = ref(false)
const locked = ref(false)
const confirmOpen = ref(false)
const saving = ref(false)
const error = ref('')

watch(
  () => [props.roomId, props.token] as const,
  async ([roomId, token], _, onCleanup) => {
    authorized.value = false
    locked.value = false
    confirmOpen.value = false
    error.value = ''
    if (!roomId || !token) return
    let active = true
    onCleanup(() => {
      active = false
    })
    try {
      const result = await getAdminRoomLock(roomId, token)
      if (!active) return
      authorized.value = true
      locked.value = result.locked
    } catch (caught) {
      if (active && caught instanceof AdminApiError && caught.status !== 403 && caught.status !== 401) {
        console.error('Unable to read room lock status', caught)
      }
    }
  },
  { immediate: true },
)

async function updateLock(): Promise<void> {
  if (saving.value) return
  saving.value = true
  error.value = ''
  try {
    const result = await setAdminRoomLock(props.roomId, !locked.value, props.token)
    locked.value = result.locked
    confirmOpen.value = false
  } catch (caught) {
    error.value = caught instanceof AdminApiError ? caught.message : '会话锁定状态更新失败'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <template v-if="authorized">
    <Button
      class="cr-header-action"
      :class="{ 'is-room-locked': locked }"
      text
      rounded
      :severity="locked ? 'danger' : 'secondary'"
      :aria-label="locked ? '解除当前会话锁定' : '锁定当前会话'"
      :title="locked ? '该会话已锁定，点击解锁' : '锁定当前会话'"
      @click="confirmOpen = true"
    >
      <LockOpen v-if="locked" :size="18" aria-hidden="true" />
      <LockKeyhole v-else :size="18" aria-hidden="true" />
    </Button>
    <span class="sr-only" aria-live="polite">{{ locked ? '当前会话已锁定' : '' }}</span>

    <Dialog
      v-model:visible="confirmOpen"
      modal
      :header="locked ? '解除当前会话锁定' : '锁定当前会话'"
      class="w-[min(94vw,460px)]"
      :draggable="false"
    >
      <div class="space-y-4">
        <Message :severity="locked ? 'info' : 'warn'" :closable="false">
          {{
            locked
              ? '解锁后，成员可以重新进入这个会话；此前断开的连接不会自动恢复。'
              : '确认后会立即断开这个会话的所有连接，并拒绝成员再次进入。其他会话不受影响。'
          }}
        </Message>
        <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
        <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
          <Button severity="secondary" outlined :disabled="saving" @click="confirmOpen = false">取消</Button>
          <Button :severity="locked ? 'success' : 'danger'" :loading="saving" @click="updateLock">
            <LockOpen v-if="locked" :size="17" aria-hidden="true" />
            <LockKeyhole v-else :size="17" aria-hidden="true" />
            {{ locked ? '确认解锁' : '确认锁定' }}
          </Button>
        </div>
      </div>
    </Dialog>
  </template>
</template>
