// Replaces web/src/markdown.test.ts (which needed a DOM for marked+DOMPurify). What is
// under test here is what TG-011 froze: the parse options, the sanitize profile, and the
// parse-then-sanitize order, with the engines injected.
import { describe, expect, test } from 'bun:test'
import { createMarkdownRenderer, MARKDOWN_PARSE_OPTIONS, MARKDOWN_SANITIZE_PROFILE } from './markdown'

describe('markdown rendering policy', () => {
  test('keeps the frozen options: GFM, hard breaks, synchronous parse', () => {
    expect(MARKDOWN_PARSE_OPTIONS).toEqual({ async: false, breaks: true, gfm: true })
    expect(MARKDOWN_SANITIZE_PROFILE).toEqual({ USE_PROFILES: { html: true } })
  })

  test('parses first and sanitises the parser OUTPUT with the frozen profile', () => {
    const calls: string[] = []
    const render = createMarkdownRenderer({
      parse: (markdown, options) => {
        calls.push(`parse:${markdown}`)
        expect(options).toBe(MARKDOWN_PARSE_OPTIONS)
        return `<p>${markdown}</p>`
      },
      sanitize: (html, profile) => {
        calls.push(`sanitize:${html}`)
        expect(profile).toBe(MARKDOWN_SANITIZE_PROFILE)
        return html.replaceAll('<script>', '')
      },
    })
    expect(render('hi')).toBe('<p>hi</p>')
    expect(calls).toEqual(['parse:hi', 'sanitize:<p>hi</p>'])
  })
})
