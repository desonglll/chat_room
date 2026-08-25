<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowUpRight, Bot, BrainCircuit, Boxes, Database, Search, Server } from 'lucide-vue-next'
import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Select from 'primevue/select'
import { AdminApiError, probeAdminVectorSearch } from '../adminApi'
import type { AdminServiceOverview, AdminServiceState, AdminTopRoom, AdminVectorProbeResult } from '../adminTypes'

const props = defineProps<{
  services: AdminServiceOverview
  rooms: AdminTopRoom[]
  token: string
}>()
const emit = defineEmits<{ error: [message: string] }>()

const roomId = ref('')
const query = ref('')
const probing = ref(false)
const result = ref<AdminVectorProbeResult | null>(null)
const vectorEnabled = computed(() =>
  props.services.items.some((item) => item.id === 'vector_store' && item.state !== 'disabled'),
)
const icons = {
  database: Database,
  redis: Boxes,
  vector_store: Server,
  embedding: BrainCircuit,
  ai_provider: Bot,
}

function stateLabel(state: AdminServiceState): string {
  return { healthy: '正常', degraded: '异常', disabled: '未启用', configured: '已配置' }[state]
}

function stateClass(state: AdminServiceState): string {
  return {
    healthy: 'text-success',
    degraded: 'text-danger',
    disabled: 'text-muted-color',
    configured: 'text-primary',
  }[state]
}

async function runProbe(): Promise<void> {
  if (!roomId.value || !query.value.trim() || probing.value) return
  probing.value = true
  result.value = null
  try {
    result.value = await probeAdminVectorSearch(roomId.value, query.value.trim(), props.token)
  } catch (caught) {
    const message =
      caught instanceof AdminApiError && caught.status === 403
        ? '当前管理员账号需要先加入该房间，才能查看语义匹配内容'
        : caught instanceof AdminApiError && caught.status === 409
          ? '向量检索尚未启用'
          : '向量检索探测失败'
    emit('error', message)
  } finally {
    probing.value = false
  }
}

function openMessage(messageId: string): void {
  window.location.assign(`/rooms/${roomId.value}?message=${encodeURIComponent(messageId)}`)
}
</script>

<template>
  <section aria-labelledby="services-heading" class="mt-8 border-t border-surface-200 pt-7">
    <div class="mb-3 flex flex-wrap items-end justify-between gap-3">
      <div>
        <h2 id="services-heading" class="text-sm font-semibold">依赖服务</h2>
        <p class="mt-1 text-xs text-muted-color">连接状态、探测耗时与向量索引积压</p>
      </div>
      <p class="text-xs text-muted-color">
        向量 {{ services.vector_index.points ?? '—' }} · 待处理 {{ services.vector_index.pending_jobs }} · 重试
        {{ services.vector_index.retrying_jobs }}
      </p>
    </div>

    <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
      <article v-for="service in services.items" :key="service.id" class="rounded-lg bg-surface-0 p-4 shadow-xs">
        <div class="mb-4 flex items-center justify-between gap-3">
          <component :is="icons[service.id]" :size="18" class="text-primary" />
          <span class="inline-flex items-center gap-1.5 text-xs font-medium" :class="stateClass(service.state)">
            <span class="size-2 rounded-full bg-current" />{{ stateLabel(service.state) }}
          </span>
        </div>
        <strong class="block text-sm">{{ service.label }}</strong>
        <p class="mt-1 break-words text-xs text-muted-color">{{ service.detail }}</p>
        <p v-if="service.latency_ms !== null" class="mt-3 text-xs tabular-nums text-muted-color">
          {{ service.latency_ms }} ms
        </p>
      </article>
    </div>

    <p v-if="services.vector_index.last_error" class="mt-3 break-words text-xs text-danger">
      最近索引错误：{{ services.vector_index.last_error }}
    </p>

    <div class="mt-6 border-t border-surface-200 pt-5">
      <div class="grid items-end gap-3 md:grid-cols-[minmax(180px,0.7fr)_minmax(240px,1.3fr)_auto]">
        <label class="grid gap-1.5 text-xs font-medium">
          房间
          <Select
            v-model="roomId"
            :options="rooms"
            option-label="name"
            option-value="id"
            placeholder="选择房间"
            :disabled="!vectorEnabled"
            fluid
          />
        </label>
        <label class="grid gap-1.5 text-xs font-medium">
          语义查询
          <InputText v-model="query" placeholder="输入与历史消息语义相关的描述" :disabled="!vectorEnabled" fluid />
        </label>
        <Button :loading="probing" :disabled="!vectorEnabled || !roomId || !query.trim()" @click="runProbe">
          <Search v-if="!probing" :size="17" />检验检索
        </Button>
      </div>

      <div v-if="result" class="mt-5 border-t border-surface-200 pt-4">
        <p class="mb-3 text-xs text-muted-color">{{ result.latency_ms }} ms · {{ result.matches.length }} 条匹配</p>
        <ol v-if="result.matches.length" class="divide-y divide-surface-100">
          <li v-for="match in result.matches" :key="match.message_id" class="flex items-start gap-3 py-3 text-sm">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <strong class="text-xs">{{ match.sender }}</strong>
                <span class="text-xs text-muted-color">相关度 {{ match.score.toFixed(3) }}</span>
              </div>
              <p class="mt-1 line-clamp-2 break-words text-muted-color">{{ match.content }}</p>
            </div>
            <Button
              text
              rounded
              severity="secondary"
              aria-label="打开原消息"
              title="打开原消息"
              @click="openMessage(match.message_id)"
            >
              <ArrowUpRight :size="17" />
            </Button>
          </li>
        </ol>
        <p v-else class="text-sm text-muted-color">没有达到相似度阈值的消息。</p>
      </div>
    </div>
  </section>
</template>
