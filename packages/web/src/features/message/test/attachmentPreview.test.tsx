/** TG-1302: chats show the server thumbnail and fall back to the original. */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { Attachment } from '@tg/core'
import { attachmentPreviewUrl, fallBackToOriginal } from '../content/attachmentPreview'
import { BubbleImage } from '../content/BubbleImage'

const photo = (overrides: Partial<Attachment> = {}): Attachment => ({
  id: 'a1',
  file_name: 'p.jpg',
  mime_type: 'image/jpeg',
  size_bytes: 4_000_000,
  download_url: '/api/attachments/a1?key=k',
  thumbnail_url: '/api/attachments/a1/thumbnail?key=k',
  is_sensitive: false,
  ...overrides,
})

/** What an older server sends: no `thumbnail_url` key at all. */
function withoutThumbnail(): Attachment {
  const { thumbnail_url: _omitted, ...rest } = photo()
  return rest
}

describe('attachmentPreviewUrl', () => {
  test('an image shows its thumbnail', () => {
    expect(attachmentPreviewUrl(photo())).toBe('/api/attachments/a1/thumbnail?key=k')
  })

  test('no thumbnail (older server, non-image) means the original', () => {
    expect(attachmentPreviewUrl(withoutThumbnail())).toBe('/api/attachments/a1?key=k')
    expect(attachmentPreviewUrl(photo({ thumbnail_url: null }))).toBe('/api/attachments/a1?key=k')
  })

  test('a GIF keeps its original — the thumbnail is a still', () => {
    expect(attachmentPreviewUrl(photo({ mime_type: 'image/gif' }))).toBe('/api/attachments/a1?key=k')
  })
})

describe('fallBackToOriginal', () => {
  test('swaps a failed thumbnail for the original exactly once', () => {
    const handler = fallBackToOriginal(photo())!
    const image = { src: '/api/attachments/a1/thumbnail?key=k', dataset: {} as Record<string, string> }
    handler({ currentTarget: image as unknown as HTMLImageElement })
    expect(image.src).toBe('/api/attachments/a1?key=k')
    image.src = 'still-broken'
    handler({ currentTarget: image as unknown as HTMLImageElement })
    expect(image.src).toBe('still-broken')
  })

  test('nothing to fall back to when the preview already is the original', () => {
    expect(fallBackToOriginal(withoutThumbnail())).toBeUndefined()
  })
})

test('the bubble image requests the thumbnail, not the original', () => {
  const html = renderToStaticMarkup(
    <BubbleImage src={attachmentPreviewUrl(photo())} fallbackSrc={photo().download_url} name="p.jpg" />,
  )
  expect(html).toContain('src="/api/attachments/a1/thumbnail?key=k"')
  expect(html).not.toContain('src="/api/attachments/a1?key=k"')
})
