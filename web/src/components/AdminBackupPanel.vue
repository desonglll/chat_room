<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ArchiveRestore, DatabaseBackup, Download, FileArchive, Play, RefreshCw, Upload } from 'lucide-vue-next'
import Button from 'primevue/button'
import Checkbox from 'primevue/checkbox'
import Dialog from 'primevue/dialog'
import Message from 'primevue/message'
import SelectButton from 'primevue/selectbutton'
import {
  AdminApiError,
  executeAdminBackup,
  exportAdminBackup,
  getAdminBackupStatus,
  runAdminBackup,
  validateAdminBackup,
} from '../adminApi'
import type { AdminBackupStatus, AdminRestoreValidationResult } from '../adminTypes'

type BackupScope = 'data' | 'complete'

const props = defineProps<{
  token: string
  databaseBackend: 'sqlite' | 'postgres'
  attachmentBackend: 'local' | 'oss'
}>()
const emit = defineEmits<{ error: [message: string]; restored: [] }>()
const scopes = [
  { label: '仅数据库', value: 'data' },
  { label: '数据库 + 文件', value: 'complete' },
]
const scope = ref<BackupScope>('data')
const exporting = ref(false)
const running = ref(false)
const loadingStatus = ref(false)
const validating = ref(false)
const restoring = ref(false)
const restoreOpen = ref(false)
const restoreFile = ref<File | null>(null)
const restoreConfirmed = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)
const success = ref('')
const backupStatus = ref<AdminBackupStatus | null>(null)
const restoreValidation = ref<AdminRestoreValidationResult | null>(null)

const fileModeAvailable = computed(() => props.attachmentBackend === 'local')
const latestRun = computed(() => backupStatus.value?.runs[0] || null)

async function loadBackupStatus(): Promise<void> {
  loadingStatus.value = true
  try {
    backupStatus.value = await getAdminBackupStatus(props.token)
  } catch (caught) {
    emit('error', caught instanceof AdminApiError ? caught.message : '读取备份状态失败')
  } finally {
    loadingStatus.value = false
  }
}

async function runBackupNow(): Promise<void> {
  if (scope.value === 'complete' && !fileModeAvailable.value) return
  running.value = true
  success.value = ''
  try {
    await runAdminBackup(props.token, scope.value === 'complete')
    success.value = '备份运行完成，归档已通过校验。'
    await loadBackupStatus()
  } catch (caught) {
    emit('error', caught instanceof AdminApiError ? caught.message : '运行备份失败')
    await loadBackupStatus()
  } finally {
    running.value = false
  }
}

async function downloadBackup(): Promise<void> {
  if (scope.value === 'complete' && !fileModeAvailable.value) return
  exporting.value = true
  success.value = ''
  try {
    const { blob, filename } = await exportAdminBackup(props.token, scope.value === 'complete')
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    link.click()
    window.setTimeout(() => URL.revokeObjectURL(url), 1000)
    success.value = `备份已导出：${filename}`
  } catch (caught) {
    emit('error', caught instanceof AdminApiError ? caught.message : '备份导出失败')
  } finally {
    exporting.value = false
  }
}

async function chooseRestoreFile(event: Event): Promise<void> {
  restoreFile.value = (event.target as HTMLInputElement).files?.[0] || null
  restoreConfirmed.value = false
  restoreValidation.value = null
  if (!restoreFile.value) return
  validating.value = true
  try {
    restoreValidation.value = await validateAdminBackup(props.token, restoreFile.value)
    restoreOpen.value = true
  } catch (caught) {
    emit('error', caught instanceof AdminApiError ? caught.message : '备份校验失败')
    closeRestore()
  } finally {
    validating.value = false
  }
}

function closeRestore(): void {
  if (restoring.value) return
  restoreOpen.value = false
  restoreConfirmed.value = false
  restoreValidation.value = null
  restoreFile.value = null
  if (fileInput.value) fileInput.value.value = ''
}

async function runRestore(): Promise<void> {
  if (!restoreFile.value || !restoreConfirmed.value) return
  restoring.value = true
  success.value = ''
  try {
    const result = await executeAdminBackup(props.token, restoreFile.value)
    const restored = result.included_files
      ? '数据库与文件恢复完成，聊天室保持锁定。'
      : '数据库恢复完成，现有文件未改动，聊天室保持锁定。'
    success.value = `${restored} 已重新排队 ${result.vector_messages_queued.toLocaleString('zh-CN')} 条向量消息。`
    emit('restored')
    restoreOpen.value = false
  } catch (caught) {
    emit('error', caught instanceof AdminApiError ? caught.message : '备份恢复失败')
  } finally {
    restoring.value = false
    if (!restoreOpen.value) closeRestore()
  }
}

function formatRunTime(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}

onMounted(loadBackupStatus)
</script>

<template>
  <section aria-labelledby="backup-heading" class="mt-10 border-t border-surface-200 pt-7">
    <div class="mb-5 flex items-start gap-3">
      <DatabaseBackup :size="20" class="mt-0.5 text-primary" aria-hidden="true" />
      <div>
        <h2 id="backup-heading" class="text-sm font-semibold">数据备份与恢复</h2>
        <p class="mt-1 text-xs leading-5 text-muted-color">
          {{ databaseBackend }} 数据库 · {{ attachmentBackend === 'local' ? '本地文件' : '对象存储' }}
        </p>
      </div>
    </div>

    <Message v-if="attachmentBackend === 'oss'" severity="info" :closable="false" class="mb-5">
      对象存储模式可备份数据库，文件需使用对象存储服务的备份能力。
    </Message>
    <Message v-if="latestRun?.status === 'failed'" severity="error" :closable="false" class="mb-5">
      最近备份失败：{{ latestRun.error }}
    </Message>
    <Message v-if="success" severity="success" closable class="mb-5" @close="success = ''">{{ success }}</Message>

    <div class="mb-7 border-y border-surface-200 py-4">
      <div class="flex flex-wrap items-center gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">{{ backupStatus?.enabled ? '自动备份已启用' : '自动备份未启用' }}</p>
          <p class="mt-1 text-xs text-muted-color">
            RPO {{ backupStatus?.rpo_minutes || '—' }} 分钟 · 保留 {{ backupStatus?.retention_count || '—' }} 份 ·
            {{ backupStatus?.target_backend || 'local' }}
          </p>
        </div>
        <Button severity="secondary" text :loading="loadingStatus" aria-label="刷新备份状态" @click="loadBackupStatus">
          <RefreshCw v-if="!loadingStatus" :size="16" aria-hidden="true" />
        </Button>
        <Button :loading="running" :disabled="scope === 'complete' && !fileModeAvailable" @click="runBackupNow">
          <Play v-if="!running" :size="16" aria-hidden="true" />立即运行
        </Button>
      </div>
      <div v-if="backupStatus?.runs.length" class="mt-4 divide-y divide-surface-200 border-t border-surface-200">
        <div v-for="run in backupStatus.runs.slice(0, 3)" :key="run.id" class="flex items-center gap-3 py-2 text-xs">
          <span :class="run.status === 'succeeded' ? 'text-success' : 'text-danger'">
            {{ run.status === 'succeeded' ? '成功' : '失败' }}
          </span>
          <span>{{ run.trigger === 'scheduled' ? '自动' : '手动' }}</span>
          <span class="text-muted-color">{{ run.includes_files ? '完整' : '数据库' }}</span>
          <span class="ml-auto tabular-nums text-muted-color">{{ formatRunTime(run.started_at) }}</span>
        </div>
      </div>
    </div>

    <div class="grid gap-7 lg:grid-cols-2 lg:gap-10">
      <div class="min-w-0">
        <div class="mb-3 flex items-center gap-2">
          <Download :size="17" class="text-primary" aria-hidden="true" />
          <h3 class="text-sm font-medium">导出备份</h3>
        </div>
        <SelectButton
          v-model="scope"
          :options="scopes"
          option-label="label"
          option-value="value"
          :allow-empty="false"
          class="grid grid-cols-2"
        />
        <p class="mt-3 min-h-10 text-xs leading-5 text-muted-color">
          {{
            scope === 'data'
              ? '包含全部账户、房间、消息及其他数据库记录。'
              : fileModeAvailable
                ? '同时包含本地附件；导出期间会短暂锁定聊天室。'
                : '对象存储文件不能通过此处导出。'
          }}
        </p>
        <Button
          class="mt-3"
          :disabled="scope === 'complete' && !fileModeAvailable"
          :loading="exporting"
          @click="downloadBackup"
        >
          <FileArchive v-if="!exporting" :size="17" aria-hidden="true" />导出 .tar.gz
        </Button>
      </div>

      <div class="min-w-0 border-t border-surface-200 pt-7 lg:border-l lg:border-t-0 lg:pl-10 lg:pt-0">
        <div class="mb-3 flex items-center gap-2">
          <ArchiveRestore :size="17" class="text-warning" aria-hidden="true" />
          <h3 class="text-sm font-medium">恢复备份</h3>
        </div>
        <p class="min-h-10 text-xs leading-5 text-muted-color">
          自动校验归档范围与文件哈希；恢复后聊天室保持锁定，核验数据后手动解锁。
        </p>
        <input
          ref="fileInput"
          class="sr-only"
          type="file"
          accept=".gz,.tgz,application/gzip"
          :disabled="validating"
          @change="chooseRestoreFile"
        />
        <Button class="mt-3" severity="warning" outlined :loading="validating" @click="fileInput?.click()">
          <Upload v-if="!validating" :size="17" aria-hidden="true" />选择并校验归档
        </Button>
      </div>
    </div>
  </section>

  <Dialog
    v-model:visible="restoreOpen"
    modal
    header="恢复数据库备份"
    class="w-[min(94vw,520px)]"
    :draggable="false"
    :closable="!restoring"
    @hide="closeRestore"
  >
    <div class="space-y-4">
      <Message severity="warn" :closable="false">
        恢复会替换当前全部数据库记录并断开在线连接。归档包含文件时，本地文件也会被替换。
      </Message>
      <div class="rounded-lg bg-surface-50 px-3 py-2.5 text-sm">
        <span class="block truncate font-medium">{{ restoreFile?.name }}</span>
        <span class="text-xs text-muted-color">{{
          restoreFile ? `${(restoreFile.size / 1024 / 1024).toFixed(2)} MiB` : ''
        }}</span>
        <span v-if="restoreValidation" class="mt-1 block text-xs text-success">
          SHA-256 已验证 · {{ restoreValidation.database_kind }} · {{ restoreValidation.file_count }} 个文件
        </span>
      </div>
      <label class="flex cursor-pointer items-start gap-2 text-sm">
        <Checkbox v-model="restoreConfirmed" binary input-id="confirmBackupRestore" />
        <span>我确认用此备份替换当前数据库</span>
      </label>
      <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button severity="secondary" outlined :disabled="restoring" @click="closeRestore">取消</Button>
        <Button severity="danger" :disabled="!restoreConfirmed" :loading="restoring" @click="runRestore">
          <ArchiveRestore v-if="!restoring" :size="17" aria-hidden="true" />确认恢复
        </Button>
      </div>
    </div>
  </Dialog>
</template>
