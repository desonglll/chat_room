/**
 * TG-511: «我的二维码» — a QR code of the add-contact link, in the accent colour on a white
 * card so it scans on both the day and the night theme (the card, not the page, is the
 * background the camera sees). Scanning opens `/add/<username>`.
 */
import { useEffect, useState } from 'react'
import { authStore } from '@tg/core'
import { useStore } from 'zustand/react'
import { addContactLink } from './profileApi'
import { t } from '../../i18n/index'

/** A token as the `#rrggbb` the QR library accepts, or undefined (the library's black/white). */
function tokenHex(name: string): string | undefined {
  const value = globalThis.getComputedStyle?.(document.documentElement).getPropertyValue(name).trim() ?? ''
  return /^#[0-9a-f]{6}$/i.test(value) ? value : undefined
}

export function ProfileQrCard() {
  const user = useStore(authStore, (state) => state.session?.user ?? null)
  const [image, setImage] = useState('')
  const link = user ? addContactLink(globalThis.location?.origin ?? '', user.username) : ''

  useEffect(() => {
    if (!link) return
    let alive = true
    void import('qrcode').then(({ toDataURL }) =>
      toDataURL(link, {
        margin: 2,
        width: 240,
        color: { dark: tokenHex('--tg-accent'), light: tokenHex('--tg-scan-surface') },
      }).then((url) => {
        if (alive) setImage(url)
      }),
    )
    return () => {
      alive = false
    }
  }, [link])

  if (!user) return null
  return (
    <section className="tg-profile-qr" aria-label={t('w.profile.de99c3')}>
      <div className="tg-profile-qr__card">
        {image ? <img className="tg-profile-qr__image" src={image} alt={t('w.profile.d133af', user.username)} /> : null}
        <p className="tg-profile-qr__name">@{user.username}</p>
      </div>
      <p className="tg-profile-qr__hint">{t('w.profile.a17215')}</p>
      <p className="tg-profile-qr__link">{link}</p>
    </section>
  )
}
