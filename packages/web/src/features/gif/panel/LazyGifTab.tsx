/**
 * The GIF tab behind `import()`: `register.ts` is imported eagerly by `main.tsx`, the tab
 * (grid, masonry, library use cases, CSS) is fetched the first time the tab opens.
 */
import { lazy, Suspense } from 'react'
import { Spinner } from '@tg/ui'
import type { MediaPanelTabContext } from '../../sticker/panel/mediaPanelTabs'
import { t } from '../../../i18n/index'

const GifTab = lazy(() => import('./GifTab').then((module) => ({ default: module.GifTab })))

export function LazyGifTab(props: MediaPanelTabContext) {
  return (
    <Suspense
      fallback={
        <div className="tg-sticker-tab__state">
          <Spinner size="md" label={t('w.gif.864bf0')} />
        </div>
      }
    >
      <GifTab {...props} />
    </Suspense>
  )
}
