<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RefreshCw } from 'lucide-vue-next'
import Button from 'primevue/button'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Select from 'primevue/select'
import { listRoomAuditEvents, listSystemAuditEvents, type AuditEvent } from '../auditApi'

const props = defineProps<{
  scope: 'system' | 'room'
  token: string
  roomId?: string
}>()

const events = ref<AuditEvent[]>([])
const actor = ref('')
const eventType = ref('')
const from = ref('')
const to = ref('')
const nextCursor = ref<string | null>(null)
const loading = ref(false)
const loadingMore = ref(false)
const error = ref('')
let controller: AbortController | null = null

const labels: Record<string, string> = {
  'system_admin.grant_requested': '授予系统管理员',
  'system_admin.revoke_requested': '撤销系统管理员',
  'registration_invite.create_requested': '创建注册邀请',
  'system.lock.update_requested': '更新系统锁',
  'room.lock.update_requested': '更新房间锁',
  'backup.export_requested': '导出备份',
  'backup.restore_requested': '请求恢复备份',
  'backup.restore_completed': '完成备份恢复',
  'ai_model.create_requested': '创建 AI 模型',
  'ai_model.update_requested': '更新 AI 模型',
  'ai_model.delete_requested': '删除 AI 模型',
  'index.rebuild_requested': '重建索引',
  'retention.purge_requested': '清理保留数据',
  'room.member.invite_requested': '邀请成员',
  'room.member.approve_requested': '批准加入',
  'room.member.reject_requested': '拒绝加入',
  'room.member.role_change_requested': '变更角色',
  'room.member.remove_requested': '移除成员',
  'room.member.ban_requested': '封禁成员',
  'room.member.unban_requested': '解除封禁',
}
const systemTypes = [
  'system_admin.grant_requested',
  'system_admin.revoke_requested',
  'registration_invite.create_requested',
  'system.lock.update_requested',
  'room.lock.update_requested',
  'backup.export_requested',
  'backup.restore_requested',
  'backup.restore_completed',
  'ai_model.create_requested',
  'ai_model.update_requested',
  'ai_model.delete_requested',
  'index.rebuild_requested',
  'retention.purge_requested',
]
const roomTypes = [
  'room.member.invite_requested',
  'room.member.approve_requested',
  'room.member.reject_requested',
  'room.member.role_change_requested',
  'room.member.remove_requested',
  'room.member.ban_requested',
  'room.member.unban_requested',
]
const typeOptions = (props.scope === 'system' ? systemTypes : roomTypes).map((value) => ({
  label: labels[value],
  value,
}))

function toIso(value: string): string | undefined {
  if (!value) return undefined
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString()
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  }).format(new Date(value))
}

async function load(append = false): Promise<void> {
  if (append && !nextCursor.value) return
  controller?.abort()
  controller = new AbortController()
  if (append) loadingMore.value = true
  else loading.value = true
  error.value = ''
  try {
    const filters = {
      actor: actor.value.trim() || undefined,
      eventType: eventType.value || undefined,
      from: toIso(from.value),
      to: toIso(to.value),
      cursor: append ? nextCursor.value || undefined : undefined,
      limit: 30,
    }
    const page =
      props.scope === 'system'
        ? await listSystemAuditEvents(props.token, filters, controller.signal)
        : await listRoomAuditEvents(props.roomId || '', props.token, filters, controller.signal)
    events.value = append ? [...events.value, ...page.items] : page.items
    nextCursor.value = page.next_cursor
  } catch (caught) {
    if ((caught as Error).name !== 'AbortError')
      error.value = caught instanceof Error ? caught.message : '读取审计记录失败'
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

watch(
  () => [props.scope, props.roomId, props.token],
  () => void load(),
)
onMounted(() => void load())
onBeforeUnmount(() => controller?.abort())
</script>

<template>
  <section :aria-labelledby="`${scope}-audit-heading`" class="border-t border-surface-200 pt-5">
    <div class="mb-4 flex items-center justify-between gap-3">
      <h2 :id="`${scope}-audit-heading`" class="text-sm font-semibold">审计记录</h2>
      <Button
        text
        rounded
        severity="secondary"
        aria-label="刷新审计记录"
        title="刷新"
        :loading="loading"
        @click="load()"
      >
        <RefreshCw v-if="!loading" :size="17" />
      </Button>
    </div>

    <form class="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" @submit.prevent="load()">
      <label class="grid gap-1.5 text-xs font-medium">操作者<InputText v-model="actor" placeholder="用户名" /></label>
      <label class="grid gap-1.5 text-xs font-medium">
        事件类型
        <Select
          v-model="eventType"
          :options="typeOptions"
          option-label="label"
          option-value="value"
          show-clear
          placeholder="全部类型"
        />
      </label>
      <label class="grid gap-1.5 text-xs font-medium">开始时间<InputText v-model="from" type="datetime-local" /></label>
      <label class="grid gap-1.5 text-xs font-medium">结束时间<InputText v-model="to" type="datetime-local" /></label>
      <Button type="submit" size="small" class="sm:col-span-2 sm:justify-self-start xl:col-span-4" :loading="loading">
        应用筛选
      </Button>
    </form>

    <Message v-if="error" severity="error" size="small" :closable="false" class="mt-4">{{ error }}</Message>
    <div class="mt-5 overflow-x-auto border-y border-surface-200">
      <table class="w-full min-w-[700px] border-collapse text-left text-sm">
        <thead class="border-b border-surface-200 text-xs text-muted-color">
          <tr>
            <th class="px-3 py-2.5 font-medium">时间</th>
            <th class="px-3 py-2.5 font-medium">操作者</th>
            <th class="px-3 py-2.5 font-medium">事件</th>
            <th class="px-3 py-2.5 font-medium">目标与变更</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="event in events" :key="event.id" class="border-b border-surface-100 last:border-0">
            <td class="whitespace-nowrap px-3 py-3 text-xs text-muted-color">{{ formatTime(event.created_at) }}</td>
            <td class="px-3 py-3 font-medium">{{ event.actor_username }}</td>
            <td class="px-3 py-3">{{ labels[event.event_type] || event.event_type }}</td>
            <td class="px-3 py-3 text-xs text-muted-color">
              <span v-if="event.target_type"
                >{{ event.target_type }}<template v-if="event.target_id"> · {{ event.target_id }}</template></span
              >
              <dl v-if="Object.keys(event.details).length" class="mt-1 flex flex-wrap gap-x-3 gap-y-1">
                <div v-for="(value, key) in event.details" :key="key" class="flex gap-1">
                  <dt>{{ key }}:</dt>
                  <dd>{{ value }}</dd>
                </div>
              </dl>
            </td>
          </tr>
          <tr v-if="!loading && !events.length">
            <td colspan="4" class="px-3 py-10 text-center text-muted-color">暂无审计记录</td>
          </tr>
        </tbody>
      </table>
    </div>
    <Button v-if="nextCursor" text size="small" class="mt-3" :loading="loadingMore" @click="load(true)"
      >加载更多</Button
    >
  </section>
</template>
