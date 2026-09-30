import type { ComponentType, ReactNode } from 'react'

/**
 * Layout helpers for the documentation pages, so each page is a list of states rather than a list
 * of divs. They are not part of `@tg/ui`'s public surface: the gallery is a consumer of the
 * package, like `packages/web` will be, and it styles itself with the same semantic tokens.
 */
export interface DocPage {
  /** Fragment value; also the anchor id. */
  id: string
  title: string
  /** One line: what the component is for. */
  summary: string
  /**
   * Things a reviewer cannot see in a screenshot: the keyboard contract, which motion token drives
   * the animation, and any recorded contrast deviation.
   */
  notes?: readonly string[]
  Demo: ComponentType
}

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="doc-section">
      <h3 className="doc-section__title">{title}</h3>
      <div className="doc-section__body">{children}</div>
    </section>
  )
}

/** A horizontal run of variants. Wraps, because the night pane is half as wide in split mode. */
export function Row({ children, align = 'center' }: { children: ReactNode; align?: 'center' | 'end' | 'start' }) {
  return <div className={`doc-row doc-row--${align}`}>{children}</div>
}

/** A vertical stack, for list-shaped controls (settings rows, radio groups). */
export function Stack({ children }: { children: ReactNode }) {
  return <div className="doc-stack">{children}</div>
}

/** Labels a single state so the screenshot is self-describing. */
export function State({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="doc-state">
      <span className="doc-state__label">{label}</span>
      <div className="doc-state__slot">{children}</div>
    </div>
  )
}

/** A stand-in for an application icon. `@tg/ui` ships no icon set, on purpose. */
export function DemoIcon({ shape = 'square' }: { shape?: 'square' | 'circle' | 'plus' }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" className="doc-icon">
      {shape === 'circle' ? <circle cx="12" cy="12" r="7" /> : null}
      {shape === 'square' ? <rect x="5" y="5" width="14" height="14" rx="3" /> : null}
      {shape === 'plus' ? <path d="M12 6v12M6 12h12" /> : null}
    </svg>
  )
}

/** A block of body text, for demonstrating a surface that contains content. */
export function Filler({ lines = 2 }: { lines?: number }) {
  return (
    <p className="doc-filler">
      {Array.from({ length: lines }, () => 'The quick brown fox jumps over the lazy dog. ').join('')}
    </p>
  )
}
