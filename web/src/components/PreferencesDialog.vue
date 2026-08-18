<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Bell, Keyboard, Save, UserRound } from 'lucide-vue-next'
import Avatar from 'primevue/avatar'
import Button from 'primevue/button'
import Dialog from 'primevue/dialog'
import SelectButton from 'primevue/selectbutton'
import ToggleSwitch from 'primevue/toggleswitch'
import type { ChatPreferences, FocusShortcut, SendShortcut, User } from '../types'

const AVATARS = ['', '😀', '😎', '🥳', '🤓', '🙂', '🫡', '🚀', '🌻', '🍀', '☕', '🎨', '💡', '🔥', '✨', '🌙', '⚡']
const SHORTCUTS: { label: string; value: SendShortcut }[] = [
  { label: 'Enter', value: 'enter' },
  { label: 'Shift + Enter', value: 'shift-enter' },
]
const FOCUS_SHORTCUTS: { label: string; value: FocusShortcut }[] = [
  { label: '空格', value: 'space' },
  { label: '/', value: 'slash' },
  { label: '关闭', value: 'none' },
]

const props = defineProps<{
  open: boolean
  user: User | null
  preferences: ChatPreferences
  saving: boolean
}>()
const emit = defineEmits<{
  close: []
  save: [preferences: ChatPreferences]
}>()

const sendShortcut = ref<SendShortcut>('enter')
const focusShortcut = ref<FocusShortcut>('space')
const notificationsEnabled = ref(false)
const notificationDetails = ref(true)
const avatarEmoji = ref('')
const visible = computed({
  get: () => props.open,
  set: (value: boolean) => { if (!value) emit('close') },
})

watch(() => props.open, (open) => {
  if (!open) return
  sendShortcut.value = props.preferences.sendShortcut
  focusShortcut.value = props.preferences.focusShortcut
  notificationsEnabled.value = props.preferences.notificationsEnabled
  notificationDetails.value = props.preferences.notificationDetails
  avatarEmoji.value = props.user?.avatar_emoji || ''
})

function save(): void {
  emit('save', {
    sendShortcut: sendShortcut.value,
    focusShortcut: focusShortcut.value,
    notificationsEnabled: notificationsEnabled.value,
    notificationDetails: notificationDetails.value,
    avatarEmoji: avatarEmoji.value,
  })
}
</script>

<template>
  <Dialog v-model:visible="visible" modal header="偏好设置" class="w-[min(94vw,500px)]" :draggable="false">
    <form class="space-y-6" @submit.prevent="save">
      <section class="grid gap-3 sm:grid-cols-[150px_1fr] sm:items-center">
        <div class="flex items-center gap-2">
          <Keyboard :size="18" class="text-primary" />
          <span class="text-sm font-medium">发送快捷键</span>
        </div>
        <SelectButton
          v-model="sendShortcut"
          :options="SHORTCUTS"
          option-label="label"
          option-value="value"
          :allow-empty="false"
          class="grid grid-cols-2"
        />
      </section>

      <section class="grid gap-3 border-t border-surface-200 pt-5 sm:grid-cols-[150px_1fr] sm:items-center">
        <div class="flex items-center gap-2">
          <Keyboard :size="18" class="text-primary" />
          <span class="text-sm font-medium">聚焦输入框</span>
        </div>
        <SelectButton
          v-model="focusShortcut"
          :options="FOCUS_SHORTCUTS"
          option-label="label"
          option-value="value"
          :allow-empty="false"
          class="grid grid-cols-3"
        />
      </section>

      <section class="grid gap-3 border-t border-surface-200 pt-5 sm:grid-cols-[150px_1fr]">
        <div class="flex items-center gap-2 pt-1">
          <Bell :size="18" class="text-primary" />
          <span class="text-sm font-medium">消息通知</span>
        </div>
        <div class="space-y-4">
          <label class="flex cursor-pointer items-center justify-between gap-4 text-sm">
            <span>浏览器通知</span>
            <ToggleSwitch v-model="notificationsEnabled" />
          </label>
          <label class="flex cursor-pointer items-center justify-between gap-4 text-sm" :class="{ 'opacity-50': !notificationsEnabled }">
            <span>显示发送者和消息详情</span>
            <ToggleSwitch v-model="notificationDetails" :disabled="!notificationsEnabled" />
          </label>
        </div>
      </section>

      <section class="grid gap-3 border-t border-surface-200 pt-5 sm:grid-cols-[150px_1fr]">
        <div class="flex items-center gap-2 pt-1">
          <UserRound :size="18" class="text-primary" />
          <span class="text-sm font-medium">Emoji 头像</span>
        </div>
        <div v-if="user" class="grid grid-cols-6 gap-2 sm:grid-cols-8">
          <button
            v-for="emoji in AVATARS"
            :key="emoji || 'default'"
            type="button"
            class="grid aspect-square place-items-center rounded-md border text-xl transition hover:-translate-y-0.5 hover:border-primary hover:bg-primary-50"
            :class="emoji === avatarEmoji ? 'border-primary bg-primary-50 shadow-sm' : 'border-surface-200 bg-surface-0'"
            :aria-label="emoji ? `使用 ${emoji} 作为头像` : '使用默认头像'"
            @click="avatarEmoji = emoji"
          >
            <span v-if="emoji">{{ emoji }}</span>
            <Avatar v-else :label="user.username.slice(0, 1).toUpperCase()" shape="circle" size="small" class="bg-surface-200! text-surface-700!" />
          </button>
        </div>
        <p v-else class="text-sm text-muted-color">登录后可设置头像</p>
      </section>

      <div class="flex justify-end gap-2 border-t border-surface-200 pt-4">
        <Button type="button" label="取消" severity="secondary" outlined @click="emit('close')" />
        <Button type="submit" :loading="saving">
          <Save :size="17" />
          <span>保存</span>
        </Button>
      </div>
    </form>
  </Dialog>
</template>
