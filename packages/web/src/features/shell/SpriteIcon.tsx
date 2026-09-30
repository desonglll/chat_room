/**
 * Icons come from the embedded `/icons/icon-sprite.svg` (staged by build.rs, served by
 * the Rust server, proxied in dev). `currentColor` strokes, so the token layer colours
 * them. Always decorative — the owning control carries the accessible name.
 */
export type SpriteIconName =
  | 'rooms'
  | 'room-add'
  | 'members'
  | 'message'
  | 'send'
  | 'search'
  | 'settings'
  | 'back'
  | 'lock'

export function SpriteIcon({ name, size = 24 }: { name: SpriteIconName; size?: number }) {
  return (
    <svg className="tg-sprite-icon" width={size} height={size} aria-hidden="true" focusable="false">
      <use href={`/icons/icon-sprite.svg#cr-icon-${name}`} />
    </svg>
  )
}
