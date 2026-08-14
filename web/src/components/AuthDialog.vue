<script setup lang="ts">
import { Eye, EyeOff, LogIn, UserPlus, X } from 'lucide-vue-next'
import { ref, watch } from 'vue'
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
const passwordVisible = ref(false)
const error = ref('')
const busy = ref(false)

watch(() => props.open, (open) => {
  if (!open) return
  password.value = ''
  passwordVisible.value = false
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
  <Teleport to="body">
    <div v-if="open" class="modal-backdrop" @mousedown.self="emit('close')">
      <section class="modal auth-modal" role="dialog" aria-modal="true" aria-labelledby="authTitle">
        <header class="modal-header">
          <h2 id="authTitle">用户账户</h2>
          <button class="icon-button" type="button" aria-label="关闭" title="关闭" @click="emit('close')">
            <X :size="18" />
          </button>
        </header>
        <form @submit.prevent="submit">
          <div class="modal-body form-stack">
            <div class="auth-modes" role="tablist" aria-label="认证方式">
              <button type="button" :class="{ active: mode === 'login' }" role="tab" :aria-selected="mode === 'login'" @click="mode = 'login'">登录</button>
              <button type="button" :class="{ active: mode === 'register' }" role="tab" :aria-selected="mode === 'register'" @click="mode = 'register'">注册</button>
            </div>

            <label for="accountUsername">用户名</label>
            <input id="accountUsername" v-model="username" type="text" maxlength="48" autocomplete="username" required autofocus>

            <label for="accountPassword">密码</label>
            <div class="password-input">
              <input id="accountPassword" v-model="password" :type="passwordVisible ? 'text' : 'password'" minlength="8" maxlength="256" :autocomplete="mode === 'register' ? 'new-password' : 'current-password'" required>
              <button type="button" :aria-label="passwordVisible ? '隐藏密码' : '显示密码'" :title="passwordVisible ? '隐藏密码' : '显示密码'" @click="passwordVisible = !passwordVisible">
                <EyeOff v-if="passwordVisible" :size="18" />
                <Eye v-else :size="18" />
              </button>
            </div>
            <p v-if="error" class="form-error" role="alert">{{ error }}</p>
          </div>
          <footer class="modal-footer">
            <button class="secondary-button" type="button" @click="emit('close')">取消</button>
            <button class="primary-button compact" type="submit" :disabled="busy">
              <UserPlus v-if="mode === 'register'" :size="17" />
              <LogIn v-else :size="17" />
              {{ busy ? '请稍候' : (mode === 'register' ? '注册并登录' : '登录') }}
            </button>
          </footer>
        </form>
      </section>
    </div>
  </Teleport>
</template>
