<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { Bot, Check, Pencil, Plus, Trash2, X } from 'lucide-vue-next'
import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Select from 'primevue/select'
import ToggleSwitch from 'primevue/toggleswitch'
import {
  AdminApiError,
  createAdminAiModel,
  deleteAdminAiModel,
  listAdminAiModels,
  updateAdminAiModel,
} from '../adminApi'
import type { AdminAiModelOption, SaveAdminAiModelOption } from '../adminTypes'

const props = defineProps<{ token: string }>()
const emit = defineEmits<{ error: [message: string] }>()
const models = ref<AdminAiModelOption[]>([])
const loading = ref(false)
const editingId = ref('')
const editing = ref(false)
const providers = [
  { label: 'OpenAI 兼容', value: 'openai' },
  { label: 'Anthropic', value: 'anthropic' },
]
const form = reactive<SaveAdminAiModelOption>({
  label: '',
  provider: 'openai',
  base_url: '',
  model: '',
  api_key_env: 'CHAT_ROOM_AI_API_KEY',
  enabled: true,
})

function report(caught: unknown, fallback: string): void {
  emit('error', caught instanceof AdminApiError ? caught.message : fallback)
}

async function load(): Promise<void> {
  loading.value = true
  try {
    models.value = await listAdminAiModels(props.token)
  } catch (caught) {
    report(caught, '读取模型配置失败')
  } finally {
    loading.value = false
  }
}

function resetForm(): void {
  editing.value = false
  editingId.value = ''
  Object.assign(form, {
    label: '',
    provider: 'openai',
    base_url: '',
    model: '',
    api_key_env: 'CHAT_ROOM_AI_API_KEY',
    enabled: true,
  })
}

function startCreate(): void {
  resetForm()
  editing.value = true
}

function startEdit(option: AdminAiModelOption): void {
  editingId.value = option.id
  editing.value = true
  Object.assign(form, {
    label: option.label,
    provider: option.provider,
    base_url: option.base_url,
    model: option.model,
    api_key_env: option.api_key_env,
    enabled: option.enabled,
  })
}

async function save(): Promise<void> {
  if (!form.label.trim() || !form.base_url.trim() || !form.model.trim() || !form.api_key_env.trim()) return
  loading.value = true
  try {
    if (editingId.value) await updateAdminAiModel(props.token, editingId.value, form)
    else await createAdminAiModel(props.token, form)
    resetForm()
    await load()
  } catch (caught) {
    report(caught, '保存模型配置失败')
  } finally {
    loading.value = false
  }
}

async function remove(option: AdminAiModelOption): Promise<void> {
  if (!window.confirm(`删除模型配置“${option.label}”？`)) return
  try {
    await deleteAdminAiModel(props.token, option.id)
    await load()
  } catch (caught) {
    report(caught, '删除模型配置失败')
  }
}

onMounted(load)
</script>

<template>
  <section aria-labelledby="ai-models-heading" class="mt-8 border-t border-surface-200 pt-7">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <h2 id="ai-models-heading" class="text-sm font-semibold">AI 渠道与模型</h2>
        <p class="mt-1 text-xs text-muted-color">真实 API key 只从配置所指向的环境变量读取</p>
      </div>
      <Button size="small" :disabled="editing" @click="startCreate"><Plus :size="16" />添加配置</Button>
    </div>

    <div class="divide-y divide-surface-200 border-y border-surface-200">
      <div v-for="option in models" :key="option.id" class="flex min-h-16 items-center gap-3 py-3">
        <Bot :size="18" class="shrink-0 text-primary" />
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <strong class="text-sm">{{ option.label }}</strong>
            <span class="text-xs" :class="option.ready ? 'text-success' : 'text-danger'">
              {{ option.ready ? '可用' : option.enabled ? '缺少凭据' : '已停用' }}
            </span>
          </div>
          <p class="mt-1 break-all text-xs text-muted-color">
            {{ option.provider }} · {{ option.model }} · {{ option.base_url || '供应商默认地址' }}
          </p>
        </div>
        <span v-if="option.source === 'environment'" class="text-xs text-muted-color">.env</span>
        <template v-else>
          <Button text rounded severity="secondary" aria-label="编辑模型配置" title="编辑" @click="startEdit(option)">
            <Pencil :size="16" />
          </Button>
          <Button text rounded severity="danger" aria-label="删除模型配置" title="删除" @click="remove(option)">
            <Trash2 :size="16" />
          </Button>
        </template>
      </div>
    </div>

    <form v-if="editing" class="mt-5 grid gap-3 border-b border-surface-200 pb-5 lg:grid-cols-2" @submit.prevent="save">
      <label class="grid gap-1.5 text-xs font-medium">显示名称<InputText v-model="form.label" maxlength="80" /></label>
      <label class="grid gap-1.5 text-xs font-medium">
        协议<Select v-model="form.provider" :options="providers" option-label="label" option-value="value" />
      </label>
      <label class="grid gap-1.5 text-xs font-medium">Base URL<InputText v-model="form.base_url" /></label>
      <label class="grid gap-1.5 text-xs font-medium">模型名<InputText v-model="form.model" /></label>
      <label class="grid gap-1.5 text-xs font-medium">API key 环境变量<InputText v-model="form.api_key_env" /></label>
      <div class="flex items-end justify-between gap-3">
        <label class="flex min-h-10 items-center gap-2 text-xs font-medium">
          <ToggleSwitch v-model="form.enabled" />启用
        </label>
        <div class="flex gap-2">
          <Button type="button" severity="secondary" outlined @click="resetForm"><X :size="16" />取消</Button>
          <Button type="submit" :loading="loading"><Check :size="16" />保存</Button>
        </div>
      </div>
    </form>
  </section>
</template>
