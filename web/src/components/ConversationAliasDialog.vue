<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ContactRound, UsersRound } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import type { ConversationSummary } from '../types'

const props = defineProps<{
  open: boolean
  conversation: ConversationSummary | null
  setAlias: (roomId: string, alias: string) => Promise<ConversationSummary>
}>()
const emit = defineEmits<{ close: [] }>()
const visible = computed({ get: () => props.open, set: (value) => !value && emit('close') })
const draft = ref('')
const saving = ref(false)
const error = ref('')
const count = computed(() => Array.from(draft.value).length)

watch(
  () => [props.open, props.conversation?.room_id] as const,
  ([open]) => {
    if (!open) return
    draft.value = props.conversation?.alias || ''
    error.value = ''
  },
)

async function save(): Promise<void> {
  if (!props.conversation || saving.value || count.value > 64) return
  saving.value = true
  error.value = ''
  try {
    await props.setAlias(props.conversation.room_id, draft.value)
    emit('close')
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '无法保存备注'
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="会话备注" class="w-[min(92vw,420px)]" :draggable="false">
    <form class="space-y-4" @submit.prevent="save">
      <div class="flex items-center gap-3 rounded-lg bg-surface-50 px-3 py-2.5">
        <span class="grid size-9 shrink-0 place-items-center rounded-full bg-primary-50 text-primary">
          <UsersRound v-if="conversation?.kind === 'group'" :size="17" aria-hidden="true" />
          <ContactRound v-else :size="17" aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <strong class="block truncate text-sm">{{ conversation?.title }}</strong>
          <small class="text-muted-color">{{ conversation?.kind === 'group' ? '群聊' : '私聊' }} · 仅自己可见</small>
        </div>
      </div>
      <label class="block">
        <span class="mb-1.5 flex items-center justify-between text-xs font-medium text-surface-700">
          <span>备注名称</span><span class="font-normal text-muted-color">{{ count }}/64</span>
        </span>
        <InputText
          v-model="draft"
          name="conversation-alias"
          autocomplete="off"
          maxlength="64"
          autofocus
          fluid
          placeholder="输入备注，留空则显示原名称"
          aria-describedby="alias-help"
        />
      </label>
      <p id="alias-help" class="text-xs leading-5 text-muted-color">
        备注只改变你看到的会话名称，不会修改对方昵称或群聊名称。
      </p>
      <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>
      <div class="flex justify-end gap-2 pt-1">
        <Button type="button" label="取消" severity="secondary" text :disabled="saving" @click="emit('close')" />
        <Button type="submit" :label="draft.trim() ? '保存备注' : '清除备注'" :loading="saving" />
      </div>
    </form>
  </Dialog>
</template>
