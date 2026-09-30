import { useEffect } from 'react'

/**
 * Freezes document scrolling while a modal layer is open, reference counted so that a `Menu`
 * inside a `Modal` closing does not unlock the page underneath the `Modal`.
 *
 * The scrollbar's width is compensated with padding, otherwise the page shifts sideways the
 * instant a dialog opens — a one-line detail that is very visible and always noticed late.
 */
let locks = 0
let restoreOverflow = ''
let restorePadding = ''

function lock(): void {
  locks += 1
  if (locks > 1) return
  const body = document.body
  restoreOverflow = body.style.overflow
  restorePadding = body.style.paddingInlineEnd
  const gap = window.innerWidth - document.documentElement.clientWidth
  body.style.overflow = 'hidden'
  if (gap > 0) body.style.paddingInlineEnd = `${gap}px`
}

function unlock(): void {
  locks = Math.max(0, locks - 1)
  if (locks > 0) return
  document.body.style.overflow = restoreOverflow
  document.body.style.paddingInlineEnd = restorePadding
}

export function useScrollLock(active: boolean): void {
  useEffect(() => {
    if (!active) return
    lock()
    return unlock
  }, [active])
}
