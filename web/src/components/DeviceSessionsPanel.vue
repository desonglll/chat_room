<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Clock3, Laptop, LogOut, MapPin, RefreshCw, Trash2 } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import Skeleton from 'primevue/skeleton'
import { listDeviceSessions, revokeDeviceSession, revokeOtherDeviceSessions, type DeviceSession } from '../sessionApi'

const props = defineProps<{ token: string }>()
const sessions = ref<DeviceSession[]>([])
const loading = ref(true)
const busyId = ref('')
const error = ref('')
const hasOtherSessions = computed(() => sessions.value.some((session) => !session.current))

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(value))
}

async function refresh(): Promise<void> {
  loading.value = true
  error.value = ''
  try {
    sessions.value = await listDeviceSessions(props.token)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '读取登录设备失败'
  } finally {
    loading.value = false
  }
}

async function revoke(session: DeviceSession): Promise<void> {
  if (!window.confirm(`退出“${session.device_name}”上的登录？`)) return
  busyId.value = session.id
  error.value = ''
  try {
    await revokeDeviceSession(props.token, session.id)
    await refresh()
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '退出设备失败'
  } finally {
    busyId.value = ''
  }
}

async function revokeOthers(): Promise<void> {
  if (!window.confirm('退出除当前设备外的所有登录？')) return
  busyId.value = 'others'
  error.value = ''
  try {
    await revokeOtherDeviceSessions(props.token)
    await refresh()
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '退出其他设备失败'
  } finally {
    busyId.value = ''
  }
}

watch(() => props.token, refresh)
onMounted(refresh)
defineExpose({ refresh })
</script>

<template>
  <section class="cr-form-section py-7" aria-labelledby="device-sessions-title">
    <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-2 text-sm font-semibold">
        <Laptop :size="18" class="shrink-0 text-primary" />
        <span id="device-sessions-title">登录设备</span>
      </div>
      <Button
        v-if="hasOtherSessions"
        size="small"
        severity="secondary"
        outlined
        :loading="busyId === 'others'"
        @click="revokeOthers"
      >
        <LogOut :size="16" />退出其他设备
      </Button>
    </div>

    <div v-if="loading" class="space-y-3" aria-label="正在读取登录设备">
      <div v-for="index in 2" :key="index" class="flex items-center gap-3 py-2">
        <Skeleton shape="circle" size="2.5rem" />
        <div class="min-w-0 flex-1 space-y-2"><Skeleton width="42%" /><Skeleton width="68%" height="0.7rem" /></div>
      </div>
    </div>
    <div v-else-if="error" class="space-y-3">
      <Message severity="error" :closable="false">{{ error }}</Message>
      <Button size="small" severity="secondary" outlined @click="refresh"><RefreshCw :size="15" />重试</Button>
    </div>
    <div v-else class="divide-y divide-[var(--cr-border)]">
      <div v-for="session in sessions" :key="session.id" class="flex min-w-0 items-center gap-3 py-3">
        <span
          class="flex size-10 shrink-0 items-center justify-center rounded-md bg-[var(--cr-primary-soft)] text-primary"
          aria-hidden="true"
        >
          <Laptop :size="18" />
        </span>
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <strong class="break-words text-sm">{{ session.device_name }}</strong>
            <span v-if="session.current" class="text-xs font-medium text-primary">当前设备</span>
          </div>
          <div class="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-color">
            <span class="inline-flex items-center gap-1"
              ><Clock3 :size="13" />{{ formatDate(session.last_used_at) }}</span
            >
            <span v-if="session.ip_hint" class="inline-flex items-center gap-1"
              ><MapPin :size="13" />{{ session.ip_hint }}</span
            >
          </div>
        </div>
        <Button
          v-if="!session.current"
          text
          rounded
          severity="danger"
          aria-label="退出此设备"
          title="退出此设备"
          :loading="busyId === session.id"
          @click="revoke(session)"
        >
          <Trash2 :size="17" />
        </Button>
      </div>
    </div>
  </section>
</template>
