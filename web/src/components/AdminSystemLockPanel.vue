<script setup lang="ts">
import { ref } from 'vue'
import { LockKeyhole, LockOpen } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import Message from 'primevue/message'
import { AdminApiError, setAdminChatLock } from '../adminApi'

const props = defineProps<{ locked: boolean; token: string }>()
const emit = defineEmits<{ updated: [locked: boolean]; error: [message: string] }>()
const confirmOpen = ref(false)
const saving = ref(false)

async function updateLock(): Promise<void> {
  saving.value = true
  try {
    const result = await setAdminChatLock(!props.locked, props.token)
    emit('updated', result.locked)
    confirmOpen.value = false
  } catch (caught) {
    emit('error', caught instanceof AdminApiError ? caught.message : '聊天室锁定状态更新失败')
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <section
    aria-labelledby="chat-lock-heading"
    class="mb-7 flex flex-col gap-4 border-y px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-5"
    :class="locked ? 'border-danger-200 bg-danger-50' : 'border-surface-200 bg-surface-0'"
  >
    <div class="flex min-w-0 items-start gap-3">
      <span
        class="grid size-10 shrink-0 place-items-center rounded-full"
        :class="locked ? 'bg-danger text-white' : 'bg-success-50 text-success'"
      >
        <LockKeyhole v-if="locked" :size="19" aria-hidden="true" />
        <LockOpen v-else :size="19" aria-hidden="true" />
      </span>
      <div class="min-w-0">
        <h2 id="chat-lock-heading" class="text-sm font-semibold">聊天室访问控制</h2>
        <p class="mt-1 text-xs leading-5 text-muted-color">
          {{
            locked
              ? '系统已锁定。现有连接已断开，所有用户暂时无法进入或新建聊天室。'
              : '系统开放中。用户可以正常进入、新建群聊和开始私聊。'
          }}
        </p>
      </div>
    </div>
    <Button :severity="locked ? 'success' : 'danger'" :outlined="!locked" @click="confirmOpen = true">
      <LockOpen v-if="locked" :size="17" aria-hidden="true" />
      <LockKeyhole v-else :size="17" aria-hidden="true" />
      {{ locked ? '解除系统锁定' : '一键锁定全部聊天室' }}
    </Button>
  </section>

  <Dialog
    v-model:visible="confirmOpen"
    modal
    :header="locked ? '解除聊天室锁定' : '锁定全部聊天室'"
    class="w-[min(94vw,480px)]"
    :draggable="false"
  >
    <div class="space-y-4">
      <Message :severity="locked ? 'info' : 'warn'" :closable="false">
        {{
          locked
            ? '解锁后，用户可以重新进入聊天室。系统不会自动恢复此前断开的连接。'
            : '确认后会立即关闭所有聊天室连接，并拒绝进入、加入、新建群聊和发起私聊。'
        }}
      </Message>
      <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button severity="secondary" outlined :disabled="saving" @click="confirmOpen = false">取消</Button>
        <Button :severity="locked ? 'success' : 'danger'" :loading="saving" @click="updateLock">
          {{ locked ? '确认解锁' : '确认锁定' }}
        </Button>
      </div>
    </div>
  </Dialog>
</template>
