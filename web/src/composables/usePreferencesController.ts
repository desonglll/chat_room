import { ref, type Ref } from 'vue'
import { updateCurrentUser } from '../api'
import { storePreferences } from '../preferences'
import type { ChatPreferences, User } from '../types'

interface PreferenceOptions {
  preferences: Ref<ChatPreferences>
  user: Ref<User | null>
  token: Ref<string>
  configureNotifications: (enabled: boolean, details: boolean) => void
  showSuccess: (message: string) => void
  showError: (message: string) => void
}

export function usePreferencesController(options: PreferenceOptions) {
  const open = ref(false)
  const saving = ref(false)

  async function save(next: ChatPreferences): Promise<void> {
    saving.value = true
    try {
      if (next.notificationsEnabled) {
        if (typeof Notification === 'undefined') throw new Error('当前浏览器不支持消息通知')
        const permission =
          Notification.permission === 'default' ? await Notification.requestPermission() : Notification.permission
        if (permission !== 'granted') throw new Error('浏览器没有授予通知权限')
      }
      if (options.user.value && options.token.value && next.avatarEmoji !== options.user.value.avatar_emoji) {
        options.user.value = await updateCurrentUser(options.token.value, {
          avatar_emoji: next.avatarEmoji,
        })
      }
      options.preferences.value = { ...next, avatarEmoji: options.user.value?.avatar_emoji || '' }
      storePreferences(options.preferences.value)
      options.configureNotifications(next.notificationsEnabled, next.notificationDetails)
      open.value = false
      options.showSuccess('偏好设置已保存')
    } catch (caught) {
      options.showError(caught instanceof Error ? caught.message : '保存失败')
    } finally {
      saving.value = false
    }
  }

  function profileUpdated(user: User): void {
    options.user.value = user
    options.preferences.value = { ...options.preferences.value, avatarEmoji: user.avatar_emoji }
    storePreferences(options.preferences.value)
    options.showSuccess('个人资料已保存')
  }

  return { open, profileUpdated, save, saving }
}
