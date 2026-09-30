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
          <IconButton ref={reactRef} label="添加回应" size="sm" onClick={() => setPicker(true)}>
            <SmileGlyph />
          </IconButton>
          <Popover
            open={picker}
            onClose={() => setPicker(false)}
            anchor={reactRef}
            placement="top"
            surface="menu"
            focus="move"
            aria-label="选择回应"
            insideRefs={[reactRef]}
          >
            <div className="tg-bubble__reaction-picker" role="group" aria-label="快速回应">
              {QUICK_REACTIONS.map((emoji) => (
                <button
                  key={emoji}
                  type="button"
                  className="tg-bubble__reaction-pick"
                  aria-label={`回应 ${emoji}`}
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
        <IconButton label="回复" size="sm" onClick={onReply}>
          <ReplyGlyph />
        </IconButton>
      )}
      {items.length === 0 ? null : (
        <>
          <IconButton ref={moreRef} label="更多操作" size="sm" onClick={() => setMore(true)}>
            <MoreGlyph />
          </IconButton>
          <Menu
            open={more}
            onClose={() => setMore(false)}
            anchor={moreRef}
            items={items}
            placement="bottom-end"
            aria-label="消息操作"
            triggerRef={moreRef}
          />
        </>
      )}
    </div>
  )
}
