/** TG-601: «有新版本» — shown when a new service worker is waiting; refreshing activates it. */
import { Button } from '@tg/ui'
import { useStore } from 'zustand/react'
import { applyUpdate, pwaStore } from '../../app/pwa'
import { t } from '../../i18n/index'

export function UpdateBanner() {
  const ready = useStore(pwaStore, (state) => state.updateReady)
  if (!ready) return null
  return (
    <div className="tg-pwa-update" role="status">
      <span>{t('w.pwa.updateReady')}</span>
      <Button size="sm" onClick={applyUpdate}>
        {t('w.pwa.refresh')}
      </Button>
    </div>
  )
}
