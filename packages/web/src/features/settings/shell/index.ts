/**
 * TG-110 settings shell — public surface. M5 tasks plug pages in with `registerSettingsPage`
 * from their own `register.ts`; anything may open the panel with `openSettings()`.
 */
export { registerSettingsPage, SETTINGS_SECTIONS } from './settingsRegistry'
export type {
  SettingsPageProps,
  SettingsPageRegistration,
  SettingsSection,
  SettingsSectionId,
} from './settingsRegistry'
export { openSettings, settingsNavigation } from './settingsNavigation'
export type { SettingsView } from './settingsNavigation'
export { SettingsHost } from './SettingsHost'
export { SettingsIcon } from './settingsIcons'
