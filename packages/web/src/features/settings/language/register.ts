/** TG-510 side-effect module, imported once by `main.tsx`: 设置 › 语言. */
import { lazy } from 'react'
import { t } from '../../../i18n/index'
import { registerSettingsPage } from '../shell'

registerSettingsPage({
  id: 'language.main',
  section: 'language',
  get title() {
    return t('w.settings.cd99b2')
  },
  order: 10,
  component: lazy(() => import('./LanguageSettingsPage').then((m) => ({ default: m.LanguageSettingsPage }))),
})
