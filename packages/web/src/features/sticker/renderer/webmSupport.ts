/**
 * Can this browser show a WebM sticker as intended — VP9 with an alpha channel?
 *
 * `canPlayType` answers "VP9 in WebM", not "with alpha", and there is no feature query for
 * alpha. Apple WebKit (Safari, and every iOS browser) either cannot play VP9 WebM at all or,
 * where it can, has drawn the alpha plane as opaque black in the versions we must support.
 * So the rule is: VP9 WebM playable AND not Apple WebKit. Everything else gets the fallback
 * (thumbnail, else the emoji) — see `VideoSticker`. Firefox and Chromium support alpha.
 */

export interface WebmProbe {
  canPlayType(type: string): string
  userAgent: string
}

export const WEBM_STICKER_TYPE = 'video/webm; codecs="vp9"'

export function isAppleWebKit(userAgent: string): boolean {
  if (/\b(iPhone|iPad|iPod)\b/.test(userAgent)) return true
  if (!/AppleWebKit\//.test(userAgent)) return false
  return !/(Chrome|Chromium|CriOS|Edg|EdgA|OPR|Android|Firefox|FxiOS)\//.test(userAgent)
}

export function detectWebmStickerSupport(probe: WebmProbe): boolean {
  return probe.canPlayType(WEBM_STICKER_TYPE) !== '' && !isAppleWebKit(probe.userAgent)
}

let override: boolean | null = null
let detected: boolean | null = null

/** The page's answer, computed once. Without a DOM (server markup) it answers `true`. */
export function webmStickerSupported(): boolean {
  if (override !== null) return override
  if (detected === null) {
    if (typeof document === 'undefined' || typeof navigator === 'undefined') return true
    const video = document.createElement('video')
    detected = detectWebmStickerSupport({
      canPlayType: (type) => video.canPlayType(type),
      userAgent: navigator.userAgent,
    })
  }
  return detected
}

/** Force the answer (`null` = detect). For tests and the fixture page's `?webm=off`. */
export function setWebmStickerSupport(value: boolean | null): void {
  override = value
  detected = null
}
