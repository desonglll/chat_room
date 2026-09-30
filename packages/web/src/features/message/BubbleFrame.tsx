/**
 * The bubble's box: frame, four corners, tail, and the flush-media edges. It maps a
 * `BubbleLayout` to data attributes and nothing else; `styles/bubble.css` owns the looks.
 */
import type { ReactNode } from 'react'
import type { BubbleLayout } from './bubbleLayout'
import { BubbleTail } from './icons'

export function BubbleFrame({
  layout,
  flushTop,
  flushBottom,
  children,
}: {
  layout: BubbleLayout
  /** The medium touches the top edge (nothing above it inside the bubble). */
  flushTop: boolean
  /** The medium touches the bottom edge (meta overlaid, no caption or reactions). */
  flushBottom: boolean
  children: ReactNode
}) {
  const { corners } = layout
  return (
    <div
      className="tg-bubble"
      data-frame={layout.frame}
      data-tail={layout.tail ? '' : undefined}
      data-tl={corners.topLeft}
      data-tr={corners.topRight}
      data-br={corners.bottomRight}
      data-bl={corners.bottomLeft}
    >
      <div
        className="tg-bubble__body"
        data-flush-top={flushTop ? '' : undefined}
        data-flush-bottom={flushBottom ? '' : undefined}
      >
        {children}
      </div>
      {layout.tail ? <BubbleTail /> : null}
    </div>
  )
}
