<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { LockKeyhole, LogOut, UnlockKeyhole } from 'lucide-vue-next'
import Button from 'primevue/button'
import Message from 'primevue/message'
import { verifyCurrentPassword } from '../api'
import { storageGet, storageSet } from '../browserStorage'
import { matchesPrivacyLockShortcut } from '../privacyLock'
import type { PrivacyLockShortcut } from '../types'
import ScopedPasswordField from './ScopedPasswordField.vue'

const LOCKED_KEY = 'chat-room.privacy-locked'
const props = defineProps<{
  token: string
  shortcut: PrivacyLockShortcut
}>()
const emit = defineEmits<{ change: [locked: boolean]; logout: [] }>()
const locked = ref(Boolean(props.token && storageGet(window.sessionStorage, LOCKED_KEY) === 'true'))
const password = ref('')
const error = ref('')
const busy = ref(false)
const concealedElements = new Map<HTMLElement, { inert: boolean; ariaHidden: string | null }>()

function restoreDocument(): void {
  for (const [element, previous] of concealedElements) {
    element.toggleAttribute('inert', previous.inert)
    if (previous.ariaHidden === null) element.removeAttribute('aria-hidden')
    else element.setAttribute('aria-hidden', previous.ariaHidden)
  }
  concealedElements.clear()
}

function applyDocumentLock(value: boolean): void {
  restoreDocument()
  if (value) {
    for (const child of document.body.children) {
      if (!(child instanceof HTMLElement) || child.dataset.privacyLockRoot !== undefined) continue
      concealedElements.set(child, {
        inert: child.hasAttribute('inert'),
        ariaHidden: child.getAttribute('aria-hidden'),
      })
      child.setAttribute('inert', '')
      child.setAttribute('aria-hidden', 'true')
    }
  }
  emit('change', value)
  if (value) void nextTick(() => document.getElementById('privacy-lock-password')?.focus())
}

function lock(): void {
  if (!props.token || locked.value) return
  password.value = ''
  error.value = ''
  locked.value = true
  storageSet(window.sessionStorage, LOCKED_KEY, 'true')
  const activeElement = document.activeElement as HTMLElement | null
  activeElement?.blur()
}

function clearLock(): void {
  locked.value = false
  password.value = ''
  error.value = ''
  storageSet(window.sessionStorage, LOCKED_KEY, '')
}

async function unlock(): Promise<void> {
  if (!password.value || busy.value) return
  busy.value = true
  error.value = ''
  try {
    await verifyCurrentPassword(props.token, password.value)
    clearLock()
  } catch (caught) {
    error.value = caught instanceof Error ? caught.message : '无法解锁'
  } finally {
    busy.value = false
  }
}

function handleShortcut(event: KeyboardEvent): void {
  if (locked.value || !props.token || !matchesPrivacyLockShortcut(event, props.shortcut)) return
  event.preventDefault()
  event.stopImmediatePropagation()
  lock()
}

function logout(): void {
  clearLock()
  emit('logout')
}

watch(locked, applyDocumentLock, { immediate: true, flush: 'post' })
watch(
  () => props.token,
  (token) => {
    if (!token) clearLock()
    else if (storageGet(window.sessionStorage, LOCKED_KEY) === 'true') locked.value = true
  },
  { immediate: true },
)
onMounted(() => window.addEventListener('keydown', handleShortcut))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleShortcut)
  applyDocumentLock(false)
})
defineExpose({ lock })
</script>

<template>
  <Teleport to="body">
    <div
      v-if="locked"
      data-privacy-lock-root
      class="fixed inset-0 z-[2147483647] grid place-items-center bg-[#111412] p-5"
      role="dialog"
      aria-modal="true"
      aria-labelledby="privacy-lock-title"
    >
      <form
        class="w-full max-w-[380px] rounded-md border border-surface-200 bg-surface-0 p-6 text-color shadow-xl sm:p-8"
        autocomplete="on"
        @submit.prevent="unlock"
      >
        <div class="flex items-center gap-3">
          <span class="grid size-11 shrink-0 place-items-center rounded-md bg-primary text-primary-contrast"
            ><LockKeyhole :size="21"
          /></span>
          <div class="min-w-0">
            <h1 id="privacy-lock-title" class="text-lg font-semibold">Chat Room 已锁定</h1>
            <p class="mt-0.5 text-sm text-muted-color">会话内容已隐藏</p>
          </div>
        </div>
        <div class="mt-7">
          <label for="privacy-lock-password" class="mb-2 block text-sm font-medium">账户密码</label>
          <ScopedPasswordField
            v-model="password"
            input-id="privacy-lock-password"
            name="privacy-lock-current-password"
            scope="account-current"
            required
            :disabled="busy"
          />
        </div>
        <Message v-if="error" class="mt-4" severity="error" size="small" :closable="false">{{ error }}</Message>
        <Button type="submit" class="mt-5 w-full" :loading="busy" :disabled="!password">
          <UnlockKeyhole :size="17" /><span>解锁</span>
        </Button>
        <Button type="button" class="mt-2 w-full" text severity="secondary" :disabled="busy" @click="logout">
          <LogOut :size="16" /><span>退出登录</span>
        </Button>
      </form>
    </div>
  </Teleport>
</template>
