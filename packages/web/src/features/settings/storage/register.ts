/** TG-509 side-effect module, imported once by `main.tsx`: 设置 › 数据与存储. */
import { lazy } from 'react'
import { registerSettingsPage } from '../shell'
import './storage.css'

registerSettingsPage({
  id: 'storage.main',
  section: 'storage',
  title: '数据与存储',
  order: 10,
  component: lazy(() => import('./StorageSettingsPage').then((m) => ({ default: m.StorageSettingsPage }))),
})
