/** React hooks for `<Sticker>`'s byte paths: sniffing an undeclared format, and blob URLs. */
import { useEffect, useState } from 'react'
import { sniffStickerFormat, STICKER_MIME, type StickerFormat } from './stickerFormat'
import { fetchStickerBytes } from './stickerSource'

export type Sniffed =
  | { status: 'pending' }
  | { status: 'ready'; format: StickerFormat; bytes: Uint8Array }
  | { status: 'unknown' }

/**
 * The format of a sticker whose wire shape did not declare one: sniffed from `bytes` at once,
 * or from `url` after one same-origin fetch (the bytes are then reused, not fetched again).
 * `null` input (format already known) does nothing.
 */
export function useSniffedSticker(input: { bytes: Uint8Array } | { url: string } | null): Sniffed {
  const url = input && 'url' in input ? input.url : null
  const bytes = input && 'bytes' in input ? input.bytes : null
  const [fetched, setFetched] = useState<{ url: string; result: Sniffed } | null>(null)

  useEffect(() => {
    if (url === null) return
    let cancelled = false
    fetchStickerBytes(url).then(
      (body) => {
        const format = sniffStickerFormat(body)
        if (!cancelled)
          setFetched({ url, result: format ? { status: 'ready', format, bytes: body } : { status: 'unknown' } })
      },
      () => {
        if (!cancelled) setFetched({ url, result: { status: 'unknown' } })
      },
    )
    return () => {
      cancelled = true
    }
  }, [url])

  if (bytes) {
    const format = sniffStickerFormat(bytes)
    return format ? { status: 'ready', format, bytes } : { status: 'unknown' }
  }
  if (url !== null && fetched?.url === url) return fetched.result
  return { status: 'pending' }
}

/** An object URL for `bytes` (revoked when the bytes change or the component unmounts). */
export function useObjectUrl(bytes: Uint8Array | null, format: StickerFormat | null): string | null {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (!bytes || !format || format === 'tgs') {
      setUrl(null)
      return
    }
    const created = URL.createObjectURL(new Blob([bytes as BlobPart], { type: STICKER_MIME[format] }))
    setUrl(created)
    return () => URL.revokeObjectURL(created)
  }, [bytes, format])
  return url
}
