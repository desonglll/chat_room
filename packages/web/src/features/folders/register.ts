/** TG-501 side-effect module, imported once by `main.tsx`: 设置 › 聊天文件夹. */
import { createElement, lazy } from 'react'
import { registerSettingsPage } from '../settings/shell'
import { t } from '../../i18n/index'

registerSettingsPage({
  id: 'folders.main',
  section: 'folders',
  get title() {
    return t('w.folders.7aa0e7')
  },
  order: 10,
  component: lazy(() =>
    import('./FolderSettingsPage').then((m) => ({ default: () => createElement(m.FolderSettingsPage) })),
  ),
})
