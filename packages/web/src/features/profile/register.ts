/**
 * TG-511 side-effect module, imported once by `main.tsx`: «头像» and «我的二维码» pages in
 * 设置 › 我的账号.
 */
import { createElement, lazy } from 'react'
import { authStore } from '@tg/core'
import { registerSettingsPage, SettingsIcon } from '../settings/shell'
import { t } from '../../i18n/index'
import './profile.css'

const AvatarCarousel = lazy(() => import('./AvatarCarousel').then((m) => ({ default: m.AvatarCarousel })))

registerSettingsPage({
  id: 'account.avatars',
  section: 'account',
  get title() {
    return t('w.profile.4ceeeb')
  },
  icon: createElement(SettingsIcon, { name: 'avatar' }),
  order: 20,
  component: () => createElement(AvatarCarousel, { userId: authStore.getState().session?.user.id ?? '' }),
})

registerSettingsPage({
  id: 'account.qr',
  section: 'account',
  get title() {
    return t('w.profile.de99c3')
  },
  icon: createElement(SettingsIcon, { name: 'qr' }),
  order: 30,
  component: lazy(() => import('./ProfileQrCard').then((m) => ({ default: m.ProfileQrCard }))),
})
