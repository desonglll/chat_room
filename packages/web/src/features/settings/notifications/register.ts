/** TG-508 side-effect module, imported once by `main.tsx`: 设置 › 通知与声音. */
import { lazy } from 'react'
import { registerSettingsPage } from '../shell'
import './notificationSettings.css'

registerSettingsPage({
  id: 'notifications.main',
  section: 'notifications',
  title: '通知与声音',
  order: 10,
  component: lazy(() => import('./NotificationSettingsPage').then((m) => ({ default: m.NotificationSettingsPage }))),
})
