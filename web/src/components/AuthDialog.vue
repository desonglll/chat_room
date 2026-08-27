<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { LogIn, UserPlus } from 'lucide-vue-next'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import Message from 'primevue/message'
import SelectButton from 'primevue/selectbutton'
import { getPublicConfig } from '../api'
import { loginUser, registerUser } from '../accountAuthApi'
import ScopedPasswordField from './ScopedPasswordField.vue'
import type { AuthSession, RegistrationMode } from '../types'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{
  close: []
  authenticated: [session: AuthSession]
}>()

const mode = ref<'login' | 'register'>('login')
const username = ref('')
const password = ref('')
const inviteToken = ref('')
const error = ref('')
const busy = ref(false)
const registrationMode = ref<RegistrationMode>('open')
const modeOptions = computed(() => [
  { label: '登录', value: 'login' },
  ...(registrationMode.value === 'disabled' ? [] : [{ label: '注册', value: 'register' as const }]),
])
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})

watch(
  () => props.open,
  (open) => {
    if (!open) return
    password.value = ''
    inviteToken.value = ''
    error.value = ''
    void getPublicConfig()
      .then((config) => {
        registrationMode.value = config.registration_mode
        if (config.registration_mode === 'disabled') mode.value = 'login'
      })
      .catch(() => {})
  },
)

watch(mode, () => {
  password.value = ''
  error.value = ''
})

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
    const session =
      mode.value === 'register'
        ? await registerUser(normalizedUsername, password.value, inviteToken.value.trim())
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
      <SelectButton
        v-model="mode"
        :options="modeOptions"
        option-label="label"
        option-value="value"
        :allow-empty="false"
        class="grid"
        :class="registrationMode === 'disabled' ? 'grid-cols-1' : 'grid-cols-2'"
      />

      <div class="flex flex-col gap-2">
        <label for="accountUsername" class="text-sm font-medium">账户用户名</label>
        <InputText
          id="accountUsername"
          v-model="username"
          name="account-username"
          maxlength="48"
          autocomplete="section-user-account username"
          spellcheck="false"
          fluid
        />
      </div>

      <div class="flex flex-col gap-2">
        <label for="accountPassword" class="text-sm font-medium">{{
          mode === 'register' ? '设置账户密码' : '账户密码'
        }}</label>
        <ScopedPasswordField
          v-model="password"
          input-id="accountPassword"
          :name="mode === 'register' ? 'account-new-password' : 'account-current-password'"
          :scope="mode === 'register' ? 'account-new' : 'account-current'"
          required
        />
      </div>

      <div v-if="mode === 'register' && registrationMode === 'invite_only'" class="flex flex-col gap-2">
        <label for="accountInvite" class="text-sm font-medium">一次性邀请码</label>
        <InputText
          id="accountInvite"
          v-model="inviteToken"
          name="account-invite"
          autocomplete="off"
          spellcheck="false"
          required
          fluid
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
