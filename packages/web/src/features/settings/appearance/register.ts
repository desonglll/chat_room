/** TG-507 side-effect module, imported once by `main.tsx`: 设置 › 外观. */
import { lazy } from 'react'
import { registerSettingsPage } from '../shell'
import { t } from '../../../i18n/index'
import './appearance.css'

registerSettingsPage({
  id: 'appearance.main',
  section: 'appearance',
  get title() {
    return t('w.settings.09b58a')
  },
  order: 10,
  component: lazy(() => import('./AppearanceSettingsPage').then((m) => ({ default: m.AppearanceSettingsPage }))),
})
