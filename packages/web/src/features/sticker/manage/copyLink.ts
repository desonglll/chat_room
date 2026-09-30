/** Copies a set's share link; resolves false when the clipboard is unavailable. */
import { stickerSetLink } from './setOrder'

export async function copyStickerSetLink(shortName: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(stickerSetLink(window.location.origin, shortName))
    return true
  } catch {
    return false
  }
}
