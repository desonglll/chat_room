import type { CSSProperties, ReactNode } from 'react'

/**
 * Removes content from the visual layout while keeping it in the accessibility tree.
 *
 * Inline styles rather than a class, because this primitive must work before any stylesheet
 * loads and it carries no themeable value — there is nothing here for TG-009's token layer to
 * own.
 */
const hidden: CSSProperties = {
  position: 'absolute',
  width: 1,
  height: 1,
  margin: -1,
  padding: 0,
  border: 0,
  overflow: 'hidden',
  clipPath: 'inset(50%)',
  whiteSpace: 'nowrap',
}

export interface VisuallyHiddenProps {
  children: ReactNode
}

export function VisuallyHidden({ children }: VisuallyHiddenProps) {
  return <span style={hidden}>{children}</span>
}
