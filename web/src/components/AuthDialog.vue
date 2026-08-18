<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { LogIn, UserPlus } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import Password from 'primevue/password'
import SelectButton from 'primevue/selectbutton'
import { loginUser, registerUser } from '../api'
import type { AuthSession } from '../types'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{
  close: []
  authenticated: [session: AuthSession]
}>()

const mode = ref<'login' | 'register'>('login')
const username = ref('')
const password = ref('')
const error = ref('')
const busy = ref(false)
const modeOptions = [
  { label: '登录', value: 'login' },
  { label: '注册', value: 'register' },
]
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => { if (!value) emit('close') },
})

watch(() => props.open, (open) => {
  if (!open) return
  password.value = ''
  error.value = ''
})

watch(mode, () => { error.value = '' })

async function submit(): Promise<void> {
  const normalizedUsername = username.value.trim()
  if (!normalizedUsername) {
    error.value = '请输入用户名'
    return
  }
  if (password.value.length < 8) {
    error.value = '密码至少需要 8 个字符'
    return
  }

  busy.value = true
  error.value = ''
  try {
    const session = mode.value === 'register'
      ? await registerUser(normalizedUsername, password.value)
      : await loginUser(normalizedUsername, password.value)
    emit('authenticated', session)
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '认证失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="用户账户" class="w-[min(92vw,440px)]" :draggable="false">
    <form class="flex flex-col gap-5" :autocomplete="mode === 'login' ? 'on' : 'off'" @submit.prevent="submit">
      <SelectButton v-model="mode" :options="modeOptions" option-label="label" option-value="value" :allow-empty="false" class="grid grid-cols-2" />

      <div class="flex flex-col gap-2">
        <label for="accountUsername" class="text-sm font-medium">用户名</label>
        <InputText id="accountUsername" v-model="username" name="username" maxlength="48" autocomplete="username" autofocus fluid />
      </div>

      <div class="flex flex-col gap-2">
        <label for="accountPassword" class="text-sm font-medium">密码</label>
        <Password
          id="accountPassword"
          v-model="password"
          :name="mode === 'register' ? 'new-password' : 'password'"
          :feedback="false"
          toggle-mask
          fluid
          minlength="8"
          maxlength="256"
          :autocomplete="mode === 'register' ? 'new-password' : 'current-password'"
        />
      </div>

      <Message v-if="error" severity="error" size="small" :closable="false">{{ error }}</Message>

      <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button type="button" label="取消" severity="secondary" outlined @click="emit('close')" />
        <Button type="submit" :loading="busy">
          <UserPlus v-if="mode === 'register'" :size="17" />
          <LogIn v-else :size="17" />
          <span>{{ mode === 'register' ? '注册并登录' : '登录' }}</span>
        </Button>
      </div>
    </form>
  </Dialog>
</template>
