<script setup lang="ts">
import { ref } from 'vue'
import { ArrowLeft, Gauge, KeyRound, Settings, Trash2 } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import { changeAccountPassword, deleteAccount } from '../api'
import ScopedPasswordField from './ScopedPasswordField.vue'
import type { User } from '../types'

const props = defineProps<{ user: User; token: string }>()
const emit = defineEmits<{ back: []; deleted: []; preferences: [] }>()

const currentPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const passwordSaving = ref(false)
const passwordError = ref('')
const passwordSaved = ref(false)
const deleteOpen = ref(false)
const deletePassword = ref('')
const deleteConfirmation = ref('')
const deleting = ref(false)
const deleteError = ref('')

async function savePassword(): Promise<void> {
  passwordError.value = ''
  passwordSaved.value = false
  if (newPassword.value !== confirmPassword.value) {
    passwordError.value = '两次输入的新密码不一致'
    return
  }
  passwordSaving.value = true
  try {
    await changeAccountPassword(props.token, currentPassword.value, newPassword.value)
    currentPassword.value = ''
    newPassword.value = ''
    confirmPassword.value = ''
    passwordSaved.value = true
  } catch (caught) {
    passwordError.value = caught instanceof Error ? caught.message : '修改密码失败'
  } finally {
    passwordSaving.value = false
  }
}

async function confirmDelete(): Promise<void> {
  if (deleteConfirmation.value !== props.user.username) return
  deleting.value = true
  deleteError.value = ''
  try {
    await deleteAccount(props.token, deletePassword.value)
    emit('deleted')
  } catch (caught) {
    deleteError.value = caught instanceof Error ? caught.message : '注销账户失败'
  } finally {
    deleting.value = false
  }
}
</script>

<template>
  <main id="workspace-main" class="cr-page min-h-0 min-w-0 flex-1 overflow-y-auto">
    <header class="cr-page-header sticky top-0 z-10 flex items-center gap-3 px-4 sm:px-7">
      <Button text rounded severity="secondary" aria-label="返回聊天" title="返回聊天" @click="emit('back')"
        ><ArrowLeft :size="19"
      /></Button>
      <div>
        <h2 class="text-base font-semibold">设置</h2>
        <p class="mt-0.5 text-xs text-muted-color">账户与应用偏好</p>
      </div>
    </header>

    <div class="cr-page-form mx-auto w-full max-w-2xl px-5 py-8 sm:px-8">
      <section class="cr-setting-row cr-form-section pb-7">
        <div class="flex items-center justify-between gap-4">
          <div class="flex items-center gap-3">
            <Settings :size="19" class="text-primary" />
            <div>
              <strong class="block text-sm">聊天与通知偏好</strong
              ><small class="text-muted-color">快捷键、浏览器通知与消息详情</small>
            </div>
          </div>
          <Button severity="secondary" outlined @click="emit('preferences')">打开</Button>
        </div>
      </section>

      <section class="cr-setting-row cr-form-section py-7">
        <div class="flex items-center justify-between gap-4">
          <div class="flex items-center gap-3">
            <Gauge :size="19" class="text-warning" />
            <div>
              <strong class="block text-sm">系统运维</strong
              ><small class="text-muted-color">服务状态与保留期维护</small>
            </div>
          </div>
          <Button as="a" href="/admin" severity="secondary" outlined>打开</Button>
        </div>
      </section>

      <form autocomplete="on" class="cr-form-section space-y-5 py-7" @submit.prevent="savePassword">
        <div class="flex items-center gap-2 text-sm font-semibold">
          <KeyRound :size="18" class="text-primary" />修改账户密码
        </div>
        <div>
          <label for="account-current-password" class="mb-2 block text-sm font-medium">当前账户密码</label>
          <ScopedPasswordField
            v-model="currentPassword"
            input-id="account-current-password"
            name="account-current-password"
            scope="account-current"
            required
          />
        </div>
        <div class="grid gap-4 sm:grid-cols-2">
          <div>
            <label for="account-new-password" class="mb-2 block text-sm font-medium">新账户密码</label
            ><ScopedPasswordField
              v-model="newPassword"
              input-id="account-new-password"
              name="account-new-password"
              scope="account-new"
              required
            />
          </div>
          <div>
            <label for="account-confirm-password" class="mb-2 block text-sm font-medium">确认新账户密码</label
            ><ScopedPasswordField
              v-model="confirmPassword"
              input-id="account-confirm-password"
              name="account-new-password-confirmation"
              scope="account-new"
              required
            />
          </div>
        </div>
        <Message v-if="passwordError" severity="error" :closable="false">{{ passwordError }}</Message>
        <Message v-else-if="passwordSaved" severity="success" :closable="false"
          >密码已修改，其他设备的登录已退出</Message
        >
        <div class="flex justify-end">
          <Button type="submit" :loading="passwordSaving">保存新密码</Button>
        </div>
      </form>

      <section class="cr-setting-row pt-7">
        <div class="flex items-start justify-between gap-4">
          <div class="flex gap-3">
            <Trash2 :size="19" class="mt-0.5 text-danger" />
            <div>
              <strong class="block text-sm text-danger">注销账户</strong
              ><small class="mt-1 block text-muted-color">你创建的聊天室也会被永久删除</small>
            </div>
          </div>
          <Button severity="danger" outlined @click="deleteOpen = true">注销</Button>
        </div>
      </section>
    </div>

    <Dialog v-model:visible="deleteOpen" modal header="确认注销账户" class="w-[min(94vw,460px)]" :draggable="false">
      <form autocomplete="off" class="space-y-4" @submit.prevent="confirmDelete">
        <Message severity="warn" :closable="false">此操作无法撤销。请输入用户名和账户密码确认。</Message>
        <div>
          <label for="delete-confirmation" class="mb-2 block text-sm font-medium">用户名</label
          ><InputText
            id="delete-confirmation"
            v-model="deleteConfirmation"
            name="delete-account-confirmation"
            autocomplete="off"
            fluid
          />
        </div>
        <div>
          <label for="delete-password" class="mb-2 block text-sm font-medium">账户密码</label
          ><ScopedPasswordField
            v-model="deletePassword"
            input-id="delete-password"
            name="delete-account-current-password"
            scope="account-current"
          />
        </div>
        <Message v-if="deleteError" severity="error" :closable="false">{{ deleteError }}</Message>
        <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
          <Button type="button" severity="secondary" outlined @click="deleteOpen = false">取消</Button
          ><Button
            type="submit"
            severity="danger"
            :loading="deleting"
            :disabled="deleteConfirmation !== user.username || !deletePassword"
            >永久注销</Button
          >
        </div>
      </form>
    </Dialog>
  </main>
</template>
