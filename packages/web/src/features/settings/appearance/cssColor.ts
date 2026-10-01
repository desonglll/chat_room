/**
 * TG-1204: `<input type="color">` only accepts `#rrggbb`, but a computed custom property comes
 * back as authored — and the production CSS minifier shortens `#0088ff` to `#08f`. Anything
 * the input cannot take rendered it black.
 */
export function toHexColor(value: string): string {
  const text = value.trim().toLowerCase()
  const short = /^#([0-9a-f])([0-9a-f])([0-9a-f])([0-9a-f])?$/.exec(text)
  if (short) return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`
  const long = /^#([0-9a-f]{6})([0-9a-f]{2})?$/.exec(text)
  if (long) return `#${long[1]}`
  const rgb = /^rgba?\(\s*(\d{1,3})[\s,]+(\d{1,3})[\s,]+(\d{1,3})/.exec(text)
  if (rgb)
    return `#${rgb
      .slice(1, 4)
      .map((part) => Math.min(255, Number(part)).toString(16).padStart(2, '0'))
      .join('')}`
  return ''
}
