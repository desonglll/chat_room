/**
 * `StickerView` behind `import()`, for the eagerly loaded surfaces (the message bubble body
 * and the suggestion strip): the renderer's code arrives with the first sticker on screen,
 * not with the app. Until then a same-size placeholder holds the layout.
 */
import { lazy, Suspense } from 'react'
import type { StickerViewProps } from './StickerView'

const StickerView = lazy(() => import('./StickerView').then((module) => ({ default: module.StickerView })))

export function LazyStickerView(props: StickerViewProps) {
  const placeholder = (
    <span
      className="tg-sticker-placeholder"
      role="img"
      aria-label={props.label}
      style={{ width: props.size, height: props.size }}
    />
  )
  return (
    <Suspense fallback={placeholder}>
      <StickerView {...props} />
    </Suspense>
  )
}
