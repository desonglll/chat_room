/**
 * Mounted once by the workspace shell: hosts the sticker management page and the set
 * preview (see `stickerOverlays.ts`). Both are code-split, so a session that never opens
 * them never downloads them.
 */
import { lazy, Suspense } from 'react'
import { Modal } from '@tg/ui'
import { useStore } from 'zustand/react'
import { closeStickerSet, closeStickerSettings, stickerOverlayStore } from './overlayStore'
import { t } from '../../i18n/index'

const StickerSetModal = lazy(() => import('./manage/StickerSetModal').then((m) => ({ default: m.StickerSetModal })))
const StickerSetsSettings = lazy(() =>
  import('./manage/StickerSetsSettings').then((m) => ({ default: m.StickerSetsSettings })),
)

export function StickerOverlays() {
  const settingsOpen = useStore(stickerOverlayStore, (state) => state.settingsOpen)
  const shortName = useStore(stickerOverlayStore, (state) => state.setShortName)
  return (
    <Suspense fallback={null}>
      {settingsOpen ? (
        <Modal
          open
          onClose={closeStickerSettings}
          title={t('w.sticker.f7c0f3')}
          size="md"
          className="tg-sticker-manage-modal"
        >
          <StickerSetsSettings />
        </Modal>
      ) : null}
      {shortName ? <StickerSetModal key={shortName} shortName={shortName} onClose={closeStickerSet} /> : null}
    </Suspense>
  )
}
