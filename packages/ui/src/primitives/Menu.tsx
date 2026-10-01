import type { ReactNode, RefObject } from 'react'
import { cx } from '../internal/cx'
import type { Placement } from '../internal/positioning'
import type { AnchorTarget } from '../internal/useAnchoredPosition'
import { MenuList, type MenuItem } from './MenuList'
import { Popover, type DismissReason } from './Popover'

/**
 * An anchored menu: `Popover` for the layer, `MenuList` for the rows and the keyboard.
 *
 * Focus mode is `move`, not `trap`, and that is a decision rather than an omission: the ARIA menu
 * pattern says Tab dismisses a menu instead of cycling inside it, and `MenuList` implements that.
 * Focus is still fully managed - the first enabled row is focused on open and the trigger gets
 * focus back on close, which is what `restoreFocus` on the popover does.
 *
 * Selecting a row closes the menu, because every Telegram menu is one-shot. Pass
 * `closeOnSelect={false}` for a menu of toggles that should stay open.
 */
export interface MenuProps {
  open: boolean
  onClose: (reason: DismissReason) => void
  anchor: AnchorTarget
  items: readonly MenuItem[]
  placement?: Placement | undefined
  offset?: number | undefined
  closeOnSelect?: boolean | undefined
  /** Accessible name. Required in practice: an unnamed menu announces as "menu" and nothing else. */
  'aria-label'?: string | undefined
  'aria-labelledby'?: string | undefined
  /** The trigger, so a pointerdown on it counts as inside and does not immediately reopen. */
  triggerRef?: RefObject<HTMLElement | null> | undefined
  className?: string | undefined
  /** Content above the items (e.g. a quick-reaction strip); gets the menu's close function. */
  header?: ((close: () => void) => ReactNode) | undefined
}

export function Menu({
  open,
  onClose,
  anchor,
  items,
  placement = 'bottom-start',
  offset = 4,
  closeOnSelect = true,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
  triggerRef,
  className,
  header,
}: MenuProps) {
  return (
    <Popover
      open={open}
      onClose={onClose}
      anchor={anchor}
      placement={placement}
      offset={offset}
      surface="menu"
      focus="move"
      restoreFocus
      insideRefs={triggerRef === undefined ? undefined : [triggerRef]}
      className={cx('tg-popover--menu-list', className)}
    >
      {header?.(() => onClose('select'))}
      <MenuList
        items={items}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        onDismiss={() => onClose('escape')}
        onSelect={() => {
          if (closeOnSelect) onClose('select')
        }}
      />
    </Popover>
  )
}
