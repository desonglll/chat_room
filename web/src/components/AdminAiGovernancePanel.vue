<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { Bot, RefreshCw, Save } from 'lucide-vue-next'
import Button from 'primevue/button'
import InputNumber from 'primevue/inputnumber'
import SelectButton from 'primevue/selectbutton'
import ToggleSwitch from 'primevue/toggleswitch'
import { AdminApiError } from '../adminApi'
import {
  getAiGovernance,
  getAiUsage,
  saveAiGovernance,
  type AiGovernanceSettings,
  type AiGovernedModel,
  type AiUsageReport,
} from '../aiGovernanceApi'

const props = defineProps<{ token: string }>()
const emit = defineEmits<{ error: [message: string] }>()
const settings = ref<AiGovernanceSettings | null>(null)
const usage = ref<AiUsageReport | null>(null)
const groupBy = ref<'room' | 'model'>('room')
const loading = ref(false)
const saving = ref(false)
const groupOptions = [
  { label: '按房间', value: 'room' },
  { label: '按模型', value: 'model' },
]

function report(caught: unknown, fallback: string): void {
  emit('error', caught instanceof AdminApiError ? caught.message : fallback)
}

async function load(): Promise<void> {
  loading.value = true
  try {
    ;[settings.value, usage.value] = await Promise.all([
      getAiGovernance(props.token),
      getAiUsage(props.token, groupBy.value),
    ])
  } catch (caught) {
    report(caught, '读取 AI 治理状态失败')
  } finally {
    loading.value = false
  }
}

async function loadUsage(): Promise<void> {
  try {
    usage.value = await getAiUsage(props.token, groupBy.value)
  } catch (caught) {
    report(caught, '读取 AI 用量失败')
  }
}

async function save(): Promise<void> {
  if (!settings.value) return
  saving.value = true
  try {
    settings.value = await saveAiGovernance(props.token, settings.value)
    await loadUsage()
  } catch (caught) {
    report(caught, '保存 AI 治理设置失败')
  } finally {
    saving.value = false
  }
}

function dollars(model: AiGovernedModel, kind: 'input' | 'output'): number {
  const micros = kind === 'input' ? model.input_price_micros_per_million : model.output_price_micros_per_million
  return micros / 1_000_000
}

function setDollars(model: AiGovernedModel, kind: 'input' | 'output', value: number | null): void {
  const micros = Math.max(0, Math.round((value || 0) * 1_000_000))
  if (kind === 'input') model.input_price_micros_per_million = micros
  else model.output_price_micros_per_million = micros
}

function number(value: number): string {
  return new Intl.NumberFormat('zh-CN').format(value)
}

function cost(micros: number): string {
  return `$${(micros / 1_000_000).toFixed(4)}`
}

watch(groupBy, loadUsage)
onMounted(load)
</script>

<template>
  <section aria-labelledby="ai-governance-heading" class="mt-8 border-t border-surface-200 pt-7">
    <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <Bot :size="18" class="text-primary" />
        <h2 id="ai-governance-heading" class="text-sm font-semibold">AI 治理与用量</h2>
      </div>
      <div class="flex gap-2">
        <Button
          text
          rounded
          severity="secondary"
          aria-label="刷新 AI 治理"
          title="刷新"
          :loading="loading"
          @click="load"
        >
          <RefreshCw v-if="!loading" :size="17" />
        </Button>
        <Button size="small" :loading="saving" :disabled="!settings" @click="save"><Save :size="16" />保存治理</Button>
      </div>
    </div>

    <template v-if="settings">
      <div class="grid gap-3 border-y border-surface-200 py-4 sm:grid-cols-3">
        <label class="grid gap-1.5 text-xs font-medium">
          最大并发
          <InputNumber v-model="settings.max_concurrent_runs" :min="1" :max="1000" fluid />
        </label>
        <label class="grid gap-1.5 text-xs font-medium">
          用户每日 token 上限
          <InputNumber v-model="settings.daily_user_token_limit" :min="1" placeholder="不限" fluid />
        </label>
        <label class="grid gap-1.5 text-xs font-medium">
          房间每日 token 上限
          <InputNumber v-model="settings.daily_room_token_limit" :min="1" placeholder="不限" fluid />
        </label>
      </div>

      <div class="flex min-h-14 items-center justify-between gap-3 border-b border-surface-200 py-3">
        <span class="text-sm font-medium">模型 allowlist</span>
        <ToggleSwitch v-model="settings.allowlist_enabled" aria-label="模型 allowlist" />
      </div>

      <div class="overflow-x-auto border-b border-surface-200">
        <table class="w-full min-w-[720px] border-collapse text-left text-xs">
          <thead class="text-muted-color">
            <tr>
              <th class="py-3 pr-3">模型</th>
              <th class="p-3">允许</th>
              <th class="p-3">输入 $/百万</th>
              <th class="py-3 pl-3">输出 $/百万</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-surface-200">
            <tr v-for="model in settings.models" :key="model.id">
              <td class="py-3 pr-3">
                <strong class="block text-sm">{{ model.label }}</strong
                ><span class="text-muted-color"
                  >{{ model.provider }} · {{ model.model }} · {{ model.ready ? '可用' : '未就绪' }}</span
                >
              </td>
              <td class="p-3"><ToggleSwitch v-model="model.allowed" :aria-label="`允许 ${model.label}`" /></td>
              <td class="p-3">
                <InputNumber
                  :model-value="dollars(model, 'input')"
                  :min="0"
                  :max-fraction-digits="6"
                  fluid
                  @update:model-value="setDollars(model, 'input', $event)"
                />
              </td>
              <td class="py-3 pl-3">
                <InputNumber
                  :model-value="dollars(model, 'output')"
                  :min="0"
                  :max-fraction-digits="6"
                  fluid
                  @update:model-value="setDollars(model, 'output', $event)"
                />
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>

    <div class="mt-6 flex items-center justify-between gap-3">
      <h3 class="text-sm font-semibold">近 30 天聚合</h3>
      <SelectButton
        v-model="groupBy"
        :options="groupOptions"
        option-label="label"
        option-value="value"
        :allow-empty="false"
        size="small"
      />
    </div>
    <div class="mt-3 overflow-x-auto border-y border-surface-200">
      <table class="w-full min-w-[680px] border-collapse text-left text-xs">
        <thead class="text-muted-color">
          <tr>
            <th class="py-3 pr-3">{{ groupBy === 'room' ? '房间' : '模型' }}</th>
            <th class="p-3">运行</th>
            <th class="p-3">失败</th>
            <th class="p-3">Token</th>
            <th class="py-3 pl-3">估算成本</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-surface-200">
          <tr v-for="item in usage?.items || []" :key="item.key">
            <td class="py-3 pr-3 font-medium">{{ item.label }}</td>
            <td class="p-3 tabular-nums">{{ number(item.runs) }}</td>
            <td class="p-3 tabular-nums" :class="item.failed_runs ? 'text-danger' : ''">
              {{ number(item.failed_runs) }}
            </td>
            <td class="p-3 tabular-nums">{{ number(item.total_tokens) }}</td>
            <td class="py-3 pl-3 tabular-nums">{{ cost(item.estimated_cost_micros) }}</td>
          </tr>
          <tr v-if="usage && !usage.items.length">
            <td colspan="5" class="py-7 text-center text-muted-color">暂无用量</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
