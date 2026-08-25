<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import {
  Activity,
  ArrowLeft,
  Clock3,
  Database,
  FileStack,
  Gauge,
  HardDrive,
  MessageSquare,
  RefreshCw,
  Server,
  ShieldAlert,
  Trash2,
  Users,
  Wifi,
} from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import ToggleSwitch from 'primevue/toggleswitch'
import { AdminApiError, getAdminOverview, purgeAdminRetention } from '../adminApi'
import type { AdminOverview, AdminPurgeResult } from '../adminTypes'
import { storageGet } from '../browserStorage'
import AdminSystemLockPanel from './AdminSystemLockPanel.vue'
import AdminServiceStatusPanel from './AdminServiceStatusPanel.vue'

const SESSION_TOKEN_KEY = 'chat-room.session-token'
const REFRESH_INTERVAL_MS = 15_000

const router = useRouter()
const token = storageGet(window.localStorage, SESSION_TOKEN_KEY)
const overview = ref<AdminOverview | null>(null)
const loading = ref(true)
const refreshing = ref(false)
const error = ref('')
const autoRefresh = ref(true)
const purgeOpen = ref(false)
const purging = ref(false)
const purgeResult = ref<AdminPurgeResult | null>(null)
let timer = 0

const errorSeverity = computed(() => (error.value.includes('权限') ? 'warn' : 'error'))
const dedupePercent = computed(() => {
  const storage = overview.value?.storage
  if (!storage?.logical_bytes) return 0
  return Math.max(0, Math.min(100, (1 - storage.physical_bytes / storage.logical_bytes) * 100))
})
const failureRate = computed(() => {
  const runtime = overview.value?.runtime
  if (!runtime?.requests) return 0
  return (runtime.failures / runtime.requests) * 100
})

function formatNumber(value: number): string {
  return new Intl.NumberFormat('zh-CN').format(value)
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`
  const units = ['KiB', 'MiB', 'GiB', 'TiB']
  let amount = value
  let unit = -1
  do {
    amount /= 1024
    unit += 1
  } while (amount >= 1024 && unit < units.length - 1)
  return `${amount.toFixed(amount >= 10 ? 1 : 2)} ${units[unit]}`
}

function formatDuration(seconds: number): string {
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  return days ? `${days}天 ${hours}小时` : hours ? `${hours}小时 ${minutes}分` : `${minutes}分钟`
}

function formatTime(value: string | null): string {
  if (!value) return '暂无消息'
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}

async function loadOverview(background = false): Promise<void> {
  if (!token) {
    error.value = '请先登录后再访问系统后台'
    loading.value = false
    return
  }
  if (background) refreshing.value = true
  else loading.value = true
  try {
    overview.value = await getAdminOverview(token)
    error.value = ''
  } catch (caught) {
    error.value = caught instanceof AdminApiError ? caught.message : '系统状态读取失败'
  } finally {
    loading.value = false
    refreshing.value = false
  }
}

async function runPurge(): Promise<void> {
  if (!token) return
  purging.value = true
  purgeResult.value = null
  try {
    purgeResult.value = await purgeAdminRetention(token)
    purgeOpen.value = false
    await loadOverview(true)
  } catch (caught) {
    error.value = caught instanceof AdminApiError ? caught.message : '保留期清理失败'
    purgeOpen.value = false
  } finally {
    purging.value = false
  }
}

function scheduleRefresh(enabled: boolean): void {
  window.clearInterval(timer)
  timer = enabled ? window.setInterval(() => void loadOverview(true), REFRESH_INTERVAL_MS) : 0
}

watch(autoRefresh, scheduleRefresh)
onMounted(() => {
  void loadOverview()
  scheduleRefresh(autoRefresh.value)
})
onBeforeUnmount(() => window.clearInterval(timer))
</script>

<template>
  <main class="h-dvh overflow-y-auto bg-surface-50 text-color">
    <header class="sticky top-0 z-20 border-b border-surface-200 bg-surface-0/95 backdrop-blur">
      <div class="mx-auto flex h-[68px] w-full max-w-[1440px] items-center gap-3 px-4 sm:px-7">
        <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="router.push('/')">
          <ArrowLeft :size="19" />
        </Button>
        <img src="/brand/echo-gate.svg" alt="Echo Gate" width="32" height="32" class="h-8 w-auto" />
        <div class="min-w-0 flex-1 border-l border-surface-200 pl-3">
          <h1 class="truncate text-base font-semibold">系统运维</h1>
          <p class="text-xs text-muted-color">Operations</p>
        </div>
        <div class="hidden items-center gap-2 sm:flex">
          <span class="text-xs text-muted-color">自动刷新</span>
          <ToggleSwitch v-model="autoRefresh" aria-label="自动刷新" />
        </div>
        <Button
          text
          rounded
          severity="secondary"
          aria-label="刷新"
          title="刷新"
          :loading="refreshing"
          @click="loadOverview(true)"
        >
          <RefreshCw v-if="!refreshing" :size="18" />
        </Button>
      </div>
    </header>

    <div class="mx-auto w-full max-w-[1440px] px-4 py-6 sm:px-7 sm:py-8">
      <Message v-if="error" :severity="errorSeverity" :closable="false" class="mb-6">{{ error }}</Message>
      <Message v-if="purgeResult" severity="success" closable class="mb-6" @close="purgeResult = null">
        已清理 {{ formatNumber(purgeResult.attachment_objects_deleted) }} 个附件对象、{{
          formatBytes(purgeResult.attachment_bytes_deleted)
        }}，永久删除 {{ formatNumber(purgeResult.rooms_deleted) }} 个过期房间。
      </Message>

      <div v-if="loading" class="space-y-7" aria-label="加载系统状态">
        <div class="grid grid-cols-2 gap-3 lg:grid-cols-4 xl:grid-cols-6">
          <Skeleton v-for="index in 6" :key="index" height="116px" border-radius="8px" />
        </div>
        <Skeleton height="260px" border-radius="8px" />
      </div>

      <template v-else-if="overview">
        <section
          class="mb-7 flex flex-wrap items-center gap-x-5 gap-y-2 border-b border-surface-200 pb-5 text-xs text-muted-color"
        >
          <span class="inline-flex items-center gap-1.5 font-medium text-success"
            ><span class="size-2 rounded-full bg-success" />服务在线</span
          >
          <span class="inline-flex items-center gap-1.5"><Database :size="14" />{{ overview.database_backend }}</span>
          <span class="inline-flex items-center gap-1.5"
            ><HardDrive :size="14" />{{ overview.attachment_backend }}</span
          >
          <span class="inline-flex items-center gap-1.5"
            ><Clock3 :size="14" />已运行 {{ formatDuration(overview.runtime.uptime_seconds) }}</span
          >
          <span class="ml-auto">更新于 {{ formatTime(overview.generated_at) }}</span>
        </section>

        <AdminSystemLockPanel
          :locked="overview.chat_rooms_locked"
          :token="token"
          @updated="overview.chat_rooms_locked = $event"
          @error="error = $event"
        />

        <AdminServiceStatusPanel
          :services="overview.services"
          :rooms="overview.top_rooms"
          :token="token"
          @error="error = $event"
        />

        <section aria-labelledby="overview-heading">
          <h2 id="overview-heading" class="mb-3 text-sm font-semibold">实时概览</h2>
          <div class="grid grid-cols-2 gap-3 lg:grid-cols-4 xl:grid-cols-6">
            <article class="rounded-lg bg-surface-0 p-4 shadow-xs">
              <Users :size="18" class="mb-5 text-primary" /><strong class="block text-xl">{{
                formatNumber(overview.totals.users)
              }}</strong
              ><span class="text-xs text-muted-color">账户</span>
            </article>
            <article class="rounded-lg bg-surface-0 p-4 shadow-xs">
              <Wifi :size="18" class="mb-5 text-success" /><strong class="block text-xl">{{
                formatNumber(overview.online_users)
              }}</strong
              ><span class="text-xs text-muted-color">在线账户</span>
            </article>
            <article class="rounded-lg bg-surface-0 p-4 shadow-xs">
              <Gauge :size="18" class="mb-5 text-warning" /><strong class="block text-xl">{{
                formatNumber(overview.totals.active_rooms)
              }}</strong
              ><span class="text-xs text-muted-color">活跃房间</span>
            </article>
            <article class="rounded-lg bg-surface-0 p-4 shadow-xs">
              <MessageSquare :size="18" class="mb-5 text-primary" /><strong class="block text-xl">{{
                formatNumber(overview.totals.messages)
              }}</strong
              ><span class="text-xs text-muted-color">消息</span>
            </article>
            <article class="rounded-lg bg-surface-0 p-4 shadow-xs">
              <FileStack :size="18" class="mb-5 text-warning" /><strong class="block text-xl">{{
                formatNumber(overview.totals.attachments)
              }}</strong
              ><span class="text-xs text-muted-color">附件</span>
            </article>
            <article class="rounded-lg bg-surface-0 p-4 shadow-xs">
              <Activity :size="18" class="mb-5 text-success" /><strong class="block text-xl">{{
                formatNumber(overview.totals.messages_24h)
              }}</strong
              ><span class="text-xs text-muted-color">24 小时消息</span>
            </article>
          </div>
        </section>

        <div class="mt-8 grid gap-8 xl:grid-cols-[minmax(0,1.35fr)_minmax(320px,0.65fr)]">
          <section aria-labelledby="rooms-heading" class="min-w-0">
            <div class="mb-3 flex items-end justify-between gap-4">
              <div>
                <h2 id="rooms-heading" class="text-sm font-semibold">活跃房间</h2>
                <p class="mt-1 text-xs text-muted-color">按累计消息数排序</p>
              </div>
              <span class="text-xs text-muted-color">{{ overview.websocket_connections }} 个 WebSocket 连接</span>
            </div>
            <div class="overflow-x-auto rounded-lg bg-surface-0 shadow-xs">
              <table class="w-full min-w-[620px] border-collapse text-left text-sm">
                <thead class="border-b border-surface-200 text-xs text-muted-color">
                  <tr>
                    <th class="px-4 py-3 font-medium">房间</th>
                    <th class="px-4 py-3 font-medium">消息</th>
                    <th class="px-4 py-3 font-medium">成员</th>
                    <th class="px-4 py-3 font-medium">最后活动</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="room in overview.top_rooms"
                    :key="room.id"
                    class="border-b border-surface-100 last:border-0"
                  >
                    <td class="px-4 py-3.5 font-medium">{{ room.name }}</td>
                    <td class="px-4 py-3.5 tabular-nums">{{ formatNumber(room.messages) }}</td>
                    <td class="px-4 py-3.5 tabular-nums">
                      {{ formatNumber(room.active_members) }}
                    </td>
                    <td class="px-4 py-3.5 text-muted-color">
                      {{ formatTime(room.last_message_at) }}
                    </td>
                  </tr>
                  <tr v-if="!overview.top_rooms.length">
                    <td colspan="4" class="px-4 py-10 text-center text-muted-color">暂无活跃房间</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </section>

          <div class="space-y-8">
            <section aria-labelledby="runtime-heading">
              <h2 id="runtime-heading" class="mb-3 text-sm font-semibold">请求运行状况</h2>
              <div class="rounded-lg bg-surface-0 p-5 shadow-xs">
                <dl class="grid grid-cols-2 gap-x-6 gap-y-5 text-sm">
                  <div>
                    <dt class="text-xs text-muted-color">累计请求</dt>
                    <dd class="mt-1 font-semibold tabular-nums">
                      {{ formatNumber(overview.runtime.requests) }}
                    </dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">当前处理中</dt>
                    <dd class="mt-1 font-semibold tabular-nums">
                      {{ formatNumber(overview.runtime.active_requests) }}
                    </dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">平均延迟</dt>
                    <dd class="mt-1 font-semibold tabular-nums">
                      {{ overview.runtime.average_latency_ms.toFixed(1) }} ms
                    </dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">最大延迟</dt>
                    <dd class="mt-1 font-semibold tabular-nums">{{ overview.runtime.max_latency_ms.toFixed(1) }} ms</dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">服务错误</dt>
                    <dd class="mt-1 font-semibold tabular-nums" :class="failureRate ? 'text-danger' : 'text-success'">
                      {{ failureRate.toFixed(2) }}%
                    </dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">有效会话</dt>
                    <dd class="mt-1 font-semibold tabular-nums">
                      {{ formatNumber(overview.totals.active_sessions) }}
                    </dd>
                  </div>
                </dl>
              </div>
            </section>

            <section aria-labelledby="storage-heading">
              <h2 id="storage-heading" class="mb-3 text-sm font-semibold">附件存储</h2>
              <div class="rounded-lg bg-surface-0 p-5 shadow-xs">
                <div class="mb-3 flex items-end justify-between">
                  <div>
                    <span class="text-xs text-muted-color">逻辑 / 物理</span
                    ><strong class="mt-1 block text-base"
                      >{{ formatBytes(overview.storage.logical_bytes) }} /
                      {{ formatBytes(overview.storage.physical_bytes) }}</strong
                    >
                  </div>
                  <strong class="text-success">节省 {{ dedupePercent.toFixed(1) }}%</strong>
                </div>
                <div class="h-2 overflow-hidden rounded-full bg-surface-100">
                  <div
                    class="h-full rounded-full bg-success transition-[width]"
                    :style="{ width: `${dedupePercent}%` }"
                  />
                </div>
                <dl class="mt-5 grid grid-cols-3 gap-3 border-t border-surface-200 pt-4 text-center">
                  <div>
                    <dt class="text-xs text-muted-color">待上传</dt>
                    <dd class="mt-1 font-semibold">{{ overview.totals.pending_uploads }}</dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">孤儿</dt>
                    <dd class="mt-1 font-semibold" :class="overview.storage.orphaned_attachments ? 'text-warning' : ''">
                      {{ overview.storage.orphaned_attachments }}
                    </dd>
                  </div>
                  <div>
                    <dt class="text-xs text-muted-color">缺哈希</dt>
                    <dd
                      class="mt-1 font-semibold"
                      :class="overview.storage.missing_hashes ? 'text-danger' : 'text-success'"
                    >
                      {{ overview.storage.missing_hashes }}
                    </dd>
                  </div>
                </dl>
              </div>
            </section>
          </div>
        </div>

        <section aria-labelledby="maintenance-heading" class="mt-10 border-t border-surface-200 pt-7">
          <div class="flex flex-col items-start justify-between gap-4 sm:flex-row sm:items-center">
            <div class="flex gap-3">
              <ShieldAlert :size="20" class="mt-0.5 text-warning" />
              <div>
                <h2 id="maintenance-heading" class="text-sm font-semibold">保留期维护</h2>
                <p class="mt-1 text-xs text-muted-color">
                  孤儿附件 {{ overview.orphan_retention_hours }} 小时，软删除房间
                  {{ overview.deleted_room_retention_days }} 天
                </p>
              </div>
            </div>
            <Button severity="danger" outlined @click="purgeOpen = true"><Trash2 :size="17" />执行清理</Button>
          </div>
        </section>
      </template>
    </div>

    <Dialog v-model:visible="purgeOpen" modal header="执行保留期清理" class="w-[min(94vw,480px)]" :draggable="false">
      <div class="space-y-4">
        <Message severity="warn" :closable="false"
          >将永久删除已超过保留期且没有活动引用的附件，以及过期的软删除房间。</Message
        >
        <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
          <Button severity="secondary" outlined @click="purgeOpen = false">取消</Button
          ><Button severity="danger" :loading="purging" @click="runPurge">确认清理</Button>
        </div>
      </div>
    </Dialog>
  </main>
</template>
