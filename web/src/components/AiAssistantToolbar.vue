<script setup lang="ts">
import { computed } from 'vue'
import { CheckCheck, Hash, ListChecks, Sparkles, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import Password from 'primevue/password'
import Select from 'primevue/select'
import ToggleSwitch from 'primevue/toggleswitch'
import type { AiModelChoice, Room } from '../types'

const props = defineProps<{
  room: Room | null
  thinkingEnabled: boolean
  aiReady: boolean
  loading: boolean
  models: AiModelChoice[]
  modelId: string
}>()

const password = defineModel<string>('password', { required: true })
const emit = defineEmits<{
  clearRoom: []
  thinking: [enabled: boolean]
  model: [id: string]
  quick: [question: string]
}>()

const selectableModels = computed(() =>
  props.models.map((option) => ({
    ...option,
    display_name: `${option.label} · ${option.model}`,
    disabled: !option.ready,
  })),
)
</script>

<template>
  <div class="flex min-h-14 flex-wrap items-center gap-2 border-b border-surface-200 px-4 py-2 sm:px-7">
    <div v-if="room" class="flex min-h-8 items-center gap-2 rounded-md bg-surface-100 px-2 text-sm">
      <Hash :size="15" class="text-primary" />
      <span class="max-w-44 truncate">{{ room.name }}</span>
      <Button
        text
        rounded
        severity="secondary"
        aria-label="清除引用会话"
        title="清除引用会话"
        class="size-7! p-0!"
        :disabled="loading"
        @click="emit('clearRoom')"
      >
        <X :size="14" />
      </Button>
    </div>
    <span v-else class="text-xs text-muted-color">可直接提问，输入 @ 可引用聊天会话</span>
    <Password
      v-if="room?.has_password"
      v-model="password"
      :feedback="false"
      toggle-mask
      autocomplete="off"
      placeholder="聊天室密码"
      input-class="w-full"
      class="min-w-40 sm:max-w-52"
      :input-props="{ form: 'ai-assistant-query-form' }"
      :disabled="loading"
    />
    <Select
      :model-value="modelId"
      :options="selectableModels"
      option-label="display_name"
      option-value="id"
      option-disabled="disabled"
      aria-label="选择 AI 渠道和模型"
      class="min-w-52 max-w-80"
      :disabled="loading || !selectableModels.length"
      @update:model-value="emit('model', String($event || ''))"
    />
    <label class="ml-auto flex min-h-8 items-center gap-2 text-xs text-muted-color">
      <ToggleSwitch
        :model-value="thinkingEnabled"
        aria-label="深度思考"
        :disabled="loading"
        @update:model-value="emit('thinking', Boolean($event))"
      />
      深度思考
    </label>
    <Button
      text
      severity="secondary"
      size="small"
      :disabled="!room || !aiReady || loading"
      @click="emit('quick', '总结这段对话')"
    >
      <Sparkles :size="16" /><span>总结</span>
    </Button>
    <Button
      text
      severity="secondary"
      size="small"
      :disabled="!room || !aiReady || loading"
      @click="emit('quick', '提取对话中的待办事项')"
    >
      <ListChecks :size="16" /><span>待办</span>
    </Button>
    <Button
      text
      severity="secondary"
      size="small"
      :disabled="!room || !aiReady || loading"
      @click="emit('quick', '梳理这段对话已经形成的结论')"
    >
      <CheckCheck :size="16" /><span>结论</span>
    </Button>
  </div>
</template>
