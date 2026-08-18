import type { ChatPreferences } from './types'

const SEND_SHORTCUT_KEY = 'chat-room.send-shortcut'
const FOCUS_SHORTCUT_KEY = 'chat-room.focus-shortcut'
const NOTIFICATIONS_KEY = 'chat-room.notifications'
const NOTIFICATION_DETAILS_KEY = 'chat-room.notification-details'
const THEME_KEY = 'chat-room.theme'

function read(key: string): string {
  try { return window.localStorage.getItem(key) || '' } catch { return '' }
}

function write(key: string, value: string): void {
  try { window.localStorage.setItem(key, value) } catch { /* Browser storage may be disabled. */ }
}

export function loadPreferences(avatarEmoji = ''): ChatPreferences {
  const focusShortcut = read(FOCUS_SHORTCUT_KEY)
  const theme = read(THEME_KEY)
  return {
    sendShortcut: read(SEND_SHORTCUT_KEY) === 'shift-enter' ? 'shift-enter' : 'enter',
    focusShortcut: focusShortcut === 'slash' || focusShortcut === 'none' ? focusShortcut : 'space',
    notificationsEnabled: read(NOTIFICATIONS_KEY) === 'true',
    notificationDetails: read(NOTIFICATION_DETAILS_KEY) !== 'false',
    avatarEmoji,
    theme: theme === 'light' || theme === 'dark' ? theme : 'system',
  }
}

export function storePreferences(preferences: ChatPreferences): void {
  write(SEND_SHORTCUT_KEY, preferences.sendShortcut)
  write(FOCUS_SHORTCUT_KEY, preferences.focusShortcut)
  write(NOTIFICATIONS_KEY, String(preferences.notificationsEnabled))
  write(NOTIFICATION_DETAILS_KEY, String(preferences.notificationDetails))
  write(THEME_KEY, preferences.theme)
}
