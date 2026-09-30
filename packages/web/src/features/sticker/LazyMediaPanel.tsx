/**
 * The composer's entry to the media panel. The panel (tabs, sticker grid, search, preview)
 * is split into its own chunk and fetched the first time the panel opens; until then the
 * emoji body is shown on its own so the button never feels dead.
 */
import { lazy, Suspense } from 'react'
import type { MediaPanelProps } from './panel/MediaPanel'

const MediaPanel = lazy(() => import('./panel/MediaPanel').then((module) => ({ default: module.MediaPanel })))

export function LazyMediaPanel(props: MediaPanelProps) {
  return (
    <Suspense fallback={<div className="tg-media-panel">{props.emoji}</div>}>
      <MediaPanel {...props} />
    </Suspense>
  )
}
