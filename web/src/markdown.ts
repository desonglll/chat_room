import DOMPurify from 'dompurify'
import { marked } from 'marked'

export function renderMarkdown(content: string): string {
  const rendered = marked.parse(content, {
    async: false,
    breaks: true,
    gfm: true,
  }) as string
  return DOMPurify.sanitize(rendered, { USE_PROFILES: { html: true } })
}
