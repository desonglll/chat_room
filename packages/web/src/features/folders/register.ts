/** TG-501 side-effect module, imported once by `main.tsx`: 设置 › 聊天文件夹. */
import { createElement, lazy } from 'react'
import { registerSettingsPage } from '../settings/shell'

registerSettingsPage({
  id: 'folders.main',
  section: 'folders',
  title: '聊天文件夹',
  order: 10,
  component: lazy(() =>
    import('./FolderSettingsPage').then((m) => ({ default: () => createElement(m.FolderSettingsPage) })),
  ),
})
