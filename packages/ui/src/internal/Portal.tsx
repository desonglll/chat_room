import { useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

/**
 * Renders into `document.body` so an overlay is never clipped by an ancestor's `overflow` or
 * trapped under a `transform`ed parent's stacking context.
 *
 * The host is resolved during the first render on the client, NOT in an effect. An effect would
 * mean the children mount one commit later, and every ref inside them would still be null when the
 * overlay's own focus and positioning effects run - which is exactly the bug this shape avoids.
 *
 * On the server it renders nothing, because `createPortal` has no server implementation. That is
 * also why the surface of every overlay is a separate, portal-free component: the markup and ARIA
 * of a dialog or a menu can then be asserted directly (see `src/test/overlayAria.test.tsx`).
 */
export interface PortalProps {
  children: ReactNode
  /** Opt out and render in place, for a consumer that already owns a positioned layer root. */
  disabled?: boolean | undefined
}

export function Portal({ children, disabled = false }: PortalProps) {
  const [host] = useState<HTMLElement | null>(() => (typeof document === 'undefined' ? null : document.body))

  if (disabled) return <>{children}</>
  if (host === null) return null
  return createPortal(children, host)
}
