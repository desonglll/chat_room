/**
 * Generated fixture photos: gradients written pixel by pixel into ImageData (numbers, not
 * colour strings), so each photo has a distinct URL and a visible identity with no server.
 */
import type { Attachment, BroadcastMessage, ChatFileItem } from '@tg/core'

export const FIXTURE_CHAT = 'chat-fixture'

function gradientJpeg(width: number, height: number, seed: number): string {
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const context = canvas.getContext('2d')
  if (!context) return ''
  const image = context.createImageData(width, height)
  const phase = (seed * 47) % 256
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const i = (y * width + x) * 4
      const stripe = Math.floor((x + y) / 64) % 2 === 0 ? 32 : 0
      image.data[i] = (phase + (x * 255) / width) % 256
      image.data[i + 1] = ((y * 255) / height + stripe) % 256
      image.data[i + 2] = (255 - phase + stripe) % 256
      image.data[i + 3] = 255
    }
  }
  context.putImageData(image, 0, 0)
  return canvas.toDataURL('image/jpeg', 0.85)
}

const SHAPES = [
  [960, 540],
  [540, 960],
  [720, 720],
  [320, 200],
  [1400, 600],
] as const

export interface FixtureMedia {
  message: BroadcastMessage
  file: ChatFileItem
}

/** `count` media, oldest first, one per minute; `videoAt` indexes are the sample video. */
export function fixtureMedia(count: number, videoAt: ReadonlySet<number>, sensitiveAt: ReadonlySet<number>) {
  return Array.from({ length: count }, (_, n): FixtureMedia => {
    const [width, height] = SHAPES[n % SHAPES.length] ?? [800, 600]
    const video = videoAt.has(n)
    const attachment: Attachment = {
      id: `att-${n}`,
      file_name: video ? `clip-${n}.webm` : `photo-${n}.jpg`,
      mime_type: video ? 'video/webm' : 'image/jpeg',
      size_bytes: 200_000,
      download_url: video ? new URL('./sample.webm', import.meta.url).href : gradientJpeg(width, height, n),
      is_sensitive: sensitiveAt.has(n),
    }
    const timestamp = new Date(Date.UTC(2026, 8, 30, 9, n)).toISOString()
    const message: BroadcastMessage = {
      type: 'broadcast',
      message_id: `msg-${n}`,
      sender_id: n % 2 ? 'u-alice' : 'u-bob',
      sender: n % 2 ? 'Alice' : 'Bob',
      sender_avatar: '',
      content: n % 3 === 0 ? `第 ${n} 张：带说明文字的媒体 https://example.com/${n}` : '',
      attachment,
      reply_to: null,
      recalled_at: null,
      edited_at: null,
      timestamp,
      favorite_id: null,
      forwarded_from: null,
      reactions: [],
    }
    const file: ChatFileItem = {
      message_id: message.message_id,
      sender_id: message.sender_id,
      sender: message.sender,
      sender_avatar: '',
      created_at: timestamp,
      attachment,
    }
    return { message, file }
  })
}
