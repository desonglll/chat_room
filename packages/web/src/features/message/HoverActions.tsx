/**
 * The pointer-only action bar beside a bubble: react, reply, more. Hidden until the row is
 * hovered or holds focus (CSS), and never shown on touch-only devices, where the long-press
 * context menu is the route. Every button is also reachable from the context menu, so the
 * bar is a shortcut and never the only way to an action.
 */
import { useRef, useState } from 'react'
import { IconButton, Menu, Popover, type MenuItem } from '@tg/ui'
import { QUICK_REACTIONS } from '@tg/core'
import { MoreGlyph, ReplyGlyph, SmileGlyph } from './icons'
import { t } from '../../i18n/index'

export function HoverActions({
  items,
  onReply,
  onReact,
}: {
  items: readonly MenuItem[]
  onReply?: (() => void) | undefined
  onReact?: ((emoji: string) => void) | undefined
}) {
  const reactRef = useRef<HTMLButtonElement>(null)
  const moreRef = useRef<HTMLButtonElement>(null)
  const [picker, setPicker] = useState(false)
  const [more, setMore] = useState(false)

  if (onReact === undefined && onReply === undefined && items.length === 0) return null

  return (
    <div className="tg-bubble__actions" data-open={picker || more ? '' : undefined}>
      {onReact === undefined ? null : (
        <>
          <IconButton ref={reactRef} label={t('w.message.f98e94')} size="sm" onClick={() => setPicker(true)}>
            <SmileGlyph />
          </IconButton>
          <Popover
            open={picker}
            onClose={() => setPicker(false)}
            anchor={reactRef}
            placement="top"
            surface="menu"
            focus="move"
            aria-label={t('w.message.483ee7')}
            insideRefs={[reactRef]}
          >
            <div className="tg-bubble__reaction-picker" role="group" aria-label={t('w.message.57e26d')}>
              {QUICK_REACTIONS.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className="tg-bubble__reaction-pick"
                  aria-label={t('w.message.0929b9', emoji)}
                  onClick={() => {
                    setPicker(false)
                    onReact(emoji)
                  }}
                >
                  {emoji}
                </button>
              ))}
            </div>
          </Popover>
        </>
      )}
      {onReply === undefined ? null : (
        <IconButton label={t('w.message.ffc785')} size="sm" onClick={onReply}>
          <ReplyGlyph />
        </IconButton>
      )}
      {items.length === 0 ? null : (
        <>
          <IconButton ref={moreRef} label={t('w.message.77836d')} size="sm" onClick={() => setMore(true)}>
            <MoreGlyph />
          </IconButton>
          <Menu
            open={more}
            onClose={() => setMore(false)}
            anchor={moreRef}
            items={items}
            placement="bottom-end"
            aria-label={t('w.message.9f2251')}
            triggerRef={moreRef}
          />
        </>
      )}
    </div>
  )
}
