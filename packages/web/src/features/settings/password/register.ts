/** TG-704 side-effect module, imported once by `main.tsx`: 设置 › 我的账号 › 密码与账号. */
import { createElement, lazy } from 'react'
import { t } from '../../../i18n/index'
import { registerSettingsPage, SettingsIcon } from '../shell'
import './password.css'

registerSettingsPage({
  id: 'account.password',
  section: 'account',
  get title() {
    return t('w.password.page')
  },
  icon: createElement(SettingsIcon, { name: 'password' }),
  order: 20,
  component: lazy(() => import('./PasswordSettingsPage').then((m) => ({ default: m.PasswordSettingsPage }))),
})
