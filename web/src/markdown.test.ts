import { describe, expect, test } from 'bun:test'
import { JSDOM } from 'jsdom'

describe('AI Markdown rendering', () => {
  test('renders structure and removes executable markup', async () => {
    const dom = new JSDOM('<!doctype html><html><body></body></html>')
    Object.assign(globalThis, {
      window: dom.window,
      document: dom.window.document,
      Element: dom.window.Element,
      Node: dom.window.Node,
      DocumentFragment: dom.window.DocumentFragment,
      HTMLTemplateElement: dom.window.HTMLTemplateElement,
    })
    const { renderMarkdown } = await import('./markdown')
    const rendered = renderMarkdown('**重点**\n\n- 第一项\n<script>alert(1)</script>')
    expect(rendered).toContain('<strong>重点</strong>')
    expect(rendered).toContain('<li>第一项</li>')
    expect(rendered).not.toContain('<script')
  })
})
