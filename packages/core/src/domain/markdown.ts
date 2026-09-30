/**
 * The message-markdown rendering policy, migrated from `web/src/markdown.ts` (TG-011).
 *
 * The old module was eleven lines of configuration around `marked` + `dompurify`;
 * `dompurify` needs a DOM and neither library is inside this package's dependency grant,
 * so what migrates is the frozen POLICY — GFM with hard line breaks, then sanitisation
 * with the html profile, in that order — while the two engines are injected by the host
 * (packages/web keeps using marked/DOMPurify; RN injects its own).
 */

/** Options the old client passed to `marked.parse` — frozen rendering behaviour. */
export const MARKDOWN_PARSE_OPTIONS = Object.freeze({
  async: false,
  breaks: true,
  gfm: true,
} as const)

/** Profile the old client passed to `DOMPurify.sanitize` — frozen sanitisation scope. */
export const MARKDOWN_SANITIZE_PROFILE = Object.freeze({
  USE_PROFILES: Object.freeze({ html: true } as const),
} as const)

export type MarkdownParseOptions = typeof MARKDOWN_PARSE_OPTIONS

export type MarkdownSanitizeProfile = typeof MARKDOWN_SANITIZE_PROFILE

export interface MarkdownEngines {
  /** e.g. `(md, options) => marked.parse(md, options) as string` */
  parse(markdown: string, options: MarkdownParseOptions): string
  /** e.g. `(html, profile) => DOMPurify.sanitize(html, profile)` */
  sanitize(html: string, profile: MarkdownSanitizeProfile): string
}

export type MarkdownRenderer = (content: string) => string

/**
 * Parse first, sanitise second — sanitising the OUTPUT html is the invariant that makes
 * the pipeline safe regardless of what the parser emits.
 */
export function createMarkdownRenderer(engines: MarkdownEngines): MarkdownRenderer {
  return (content) => engines.sanitize(engines.parse(content, MARKDOWN_PARSE_OPTIONS), MARKDOWN_SANITIZE_PROFILE)
}
