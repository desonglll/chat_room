import type { ChatPreferences } from './types'

const SEND_SHORTCUT_KEY = 'chat-room.send-shortcut'
const NOTIFICATIONS_KEY = 'chat-room.notifications'
const NOTIFICATION_DETAILS_KEY = 'chat-room.notification-details'

function read(key: string): string {
  try { return window.localStorage.getItem(key) || '' } catch { return '' }
}

function write(key: string, value: string): void {
  try { window.localStorage.setItem(key, value) } catch { /* Browser storage may be disabled. */ }
}

export function loadPreferences(avatarEmoji = ''): ChatPreferences {
  return {
    sendShortcut: read(SEND_SHORTCUT_KEY) === 'shift-enter' ? 'shift-enter' : 'enter',
    notificationsEnabled: read(NOTIFICATIONS_KEY) === 'true',
    notificationDetails: read(NOTIFICATION_DETAILS_KEY) !== 'false',
    avatarEmoji,
  }
}

export function storePreferences(preferences: ChatPreferences): void {
  write(SEND_SHORTCUT_KEY, preferences.sendShortcut)
  write(NOTIFICATIONS_KEY, String(preferences.notificationsEnabled))
  write(NOTIFICATION_DETAILS_KEY, String(preferences.notificationDetails))
}
