/** TG-508 side-effect module, imported once by `main.tsx`: 设置 › 通知与声音. */
import { lazy } from 'react'
import { registerSettingsPage } from '../shell'
import { t } from '../../../i18n/index'
import './notificationSettings.css'

registerSettingsPage({
  id: 'notifications.main',
  section: 'notifications',
  get title() {
    return t('w.settings.bea87e')
  },
  order: 10,
  component: lazy(() => import('./NotificationSettingsPage').then((m) => ({ default: m.NotificationSettingsPage }))),
})
