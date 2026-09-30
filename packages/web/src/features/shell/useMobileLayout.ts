/**
 * Whether the viewport is in Telegram's single-column (mobile) layout. The breakpoint
 * matches `shell.css`; both are the one number `MOBILE_MAX_WIDTH`.
 */
import { useEffect, useState } from 'react'

export const MOBILE_MAX_WIDTH = 599
const QUERY = `(max-width: ${MOBILE_MAX_WIDTH}px)`

export function useMobileLayout(): boolean {
  const [mobile, setMobile] = useState(() => typeof window !== 'undefined' && window.matchMedia(QUERY).matches)
  useEffect(() => {
    const media = window.matchMedia(QUERY)
    const update = () => setMobile(media.matches)
    update()
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])
  return mobile
}
