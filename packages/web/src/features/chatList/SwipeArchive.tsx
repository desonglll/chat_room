/**
 * Swipe a chat row left on a touch screen to archive (or, in the archive, to unarchive) it —
 * Telegram's gesture. The row follows the finger, an action strip is revealed behind it, and
 * the release either springs back or flies out and commits. Springs come from the token layer
 * (`readSpring`), so `prefers-reduced-motion` gets no spring: the row snaps.
 *
 * Only `pointerType === 'touch'` starts it: a mouse drag on desktop stays a text selection /
 * link drag, and the context menu is the desktop route. `touch-action: pan-y` leaves vertical
 * scrolling to the browser, which then cancels the pointer — a scroll never archives.
 */
import type { PointerEvent, ReactNode } from 'react'
import { useEffect, useRef } from 'react'
import { animate } from 'motion'
import { readSpring, usePrefersReducedMotion } from '@tg/ui'
import { ChatListIcon } from './chatListIcons'
import type { SwipeAxis } from './swipeGesture'
import { commitsArchive, lockAxis, swipeOffset, swipeProgress } from './swipeGesture'

export interface SwipeArchiveProps {
  archived: boolean
  onCommit: () => void
  disabled?: boolean | undefined
  children: ReactNode
}

interface Drag {
  pointerId: number
  x: number
  y: number
  axis: SwipeAxis
  offset: number
  lastX: number
  lastT: number
  velocity: number
}

export function SwipeArchive({ archived, onCommit, disabled = false, children }: SwipeArchiveProps) {
  const root = useRef<HTMLDivElement>(null)
  const content = useRef<HTMLDivElement>(null)
  const drag = useRef<Drag | null>(null)
  const running = useRef<{ stop(): void } | null>(null)
  const swallowClick = useRef(false)
  const reduced = usePrefersReducedMotion()

  const width = () => root.current?.getBoundingClientRect().width ?? 0

  const paint = (offset: number) => {
    const row = content.current
    const host = root.current
    if (!row || !host) return
    row.style.transform = offset === 0 ? '' : `translate3d(${offset}px, 0, 0)`
    host.style.setProperty('--tg-swipe-progress', String(swipeProgress(offset, width())))
    host.toggleAttribute('data-swiping', offset !== 0)
    host.toggleAttribute('data-armed', swipeProgress(offset, width()) >= 1)
  }

  const settle = (from: number, to: number, done?: () => void) => {
    running.current?.stop()
    const spring = reduced ? null : readSpring('smooth', root.current)
    if (!spring || from === to) {
      paint(to)
      done?.()
      return
    }
    const controls = animate(from, to, { type: 'spring', ...spring, restDelta: 1, onUpdate: paint })
    running.current = controls
    void controls.finished.then(() => {
      if (running.current !== controls) return
      running.current = null
      done?.()
    })
  }

  useEffect(() => () => running.current?.stop(), [])

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (disabled || event.pointerType !== 'touch' || drag.current) return
    running.current?.stop()
    running.current = null
    const current = content.current ? new DOMMatrixReadOnly(getComputedStyle(content.current).transform).m41 : 0
    drag.current = {
      pointerId: event.pointerId,
      x: event.clientX - current,
      y: event.clientY,
      axis: current !== 0 ? 'horizontal' : 'pending',
      offset: current,
      lastX: event.clientX,
      lastT: event.timeStamp,
      velocity: 0,
    }
  }

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const state = drag.current
    if (!state || state.pointerId !== event.pointerId) return
    const dx = event.clientX - state.x
    if (state.axis === 'pending') {
      state.axis = lockAxis(dx, event.clientY - state.y)
      if (state.axis === 'vertical') {
        drag.current = null
        return
      }
      if (state.axis === 'pending') return
      root.current?.setPointerCapture?.(event.pointerId)
      swallowClick.current = true
    }
    const elapsed = event.timeStamp - state.lastT
    if (elapsed > 0) state.velocity = (event.clientX - state.lastX) / elapsed
    state.lastX = event.clientX
    state.lastT = event.timeStamp
    state.offset = swipeOffset(dx, width())
    paint(state.offset)
  }

  const release = (event: PointerEvent<HTMLDivElement>, cancelled: boolean) => {
    const state = drag.current
    if (!state || state.pointerId !== event.pointerId) return
    drag.current = null
    if (state.axis !== 'horizontal') return
    const full = width()
    if (!cancelled && commitsArchive(state.offset, full, state.velocity)) {
      settle(state.offset, -full, () => {
        onCommit()
        // Still mounted (e.g. search spans both folders): come back to rest without a spring.
        paint(0)
      })
    } else {
      settle(state.offset, 0)
    }
  }

  return (
    <div
      ref={root}
      className="tg-swipe"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={(event) => release(event, false)}
      onPointerCancel={(event) => release(event, true)}
      onClickCapture={(event) => {
        if (!swallowClick.current) return
        swallowClick.current = false
        event.preventDefault()
        event.stopPropagation()
      }}
      onPointerDownCapture={() => {
        swallowClick.current = false
      }}
    >
      <div className={`tg-swipe__action tg-swipe__action--${archived ? 'unarchive' : 'archive'}`} aria-hidden="true">
        <span className="tg-swipe__action-inner">
          <ChatListIcon name={archived ? 'unarchive' : 'archive'} size={26} />
          <span>{archived ? '取消归档' : '归档'}</span>
        </span>
      </div>
      <div ref={content} className="tg-swipe__content">
        {children}
      </div>
    </div>
  )
}
