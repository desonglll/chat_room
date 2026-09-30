/**
 * `@tg/ui` primitives — the package's public entry point.
 *
 * Rules (architecture.md section 3, agent-protocol.md section 5): a primitive knows nothing
 * about Chat, Message or User; it consumes only the `semantic` CSS variable layer, never a
 * `primitive` value; every animation has a `prefers-reduced-motion` fallback. The first two are
 * enforced by `../test/tokenDiscipline.test.ts`, not by convention.
 *
 * Stylesheet: an application must import `@tg/ui/styles.css` once. The components carry class
 * names only; they never import CSS themselves (see the header of `../styles.css`).
 *
 * The 19 atoms of TG-010 are all exported below, together with `MenuList` (the keyboard-owning
 * part of `Menu`, exported because `ContextMenu` and any future submenu reuse it) and
 * `RadioGroup` (a `Radio` without its group is not usable). `VisuallyHidden` is TG-002's example
 * primitive and stays.
 */

export { Avatar, AVATAR_PALETTE_SLOTS } from './Avatar'
export type { AvatarProps } from './Avatar'

export { Badge } from './Badge'
export type { BadgeProps } from './Badge'

export { Button } from './Button'
export type { ButtonProps } from './Button'

export { Checkbox } from './Checkbox'
export type { CheckboxProps } from './Checkbox'

export { ContextMenu } from './ContextMenu'
export type { ContextMenuProps } from './ContextMenu'

export { IconButton } from './IconButton'
export type { IconButtonProps } from './IconButton'

export { Menu } from './Menu'
export type { MenuProps } from './Menu'

export { MenuList } from './MenuList'
export type { MenuItem, MenuListProps } from './MenuList'

export { Modal } from './Modal'
export type { DialogDismissReason, ModalProps } from './Modal'

export { Popover } from './Popover'
export type { DismissReason, PopoverProps } from './Popover'

export { Radio, RadioGroup } from './Radio'
export type { RadioGroupProps, RadioOption, RadioProps } from './Radio'

export { Ripple } from './Ripple'
export type { RippleProps } from './Ripple'

export { ScrollArea } from './ScrollArea'
export type { ScrollAreaProps } from './ScrollArea'

export { Sheet } from './Sheet'
export type { SheetProps } from './Sheet'

export { Skeleton } from './Skeleton'
export type { SkeletonProps } from './Skeleton'

export { Spinner } from './Spinner'
export type { SpinnerProps } from './Spinner'

export { Tabs } from './Tabs'
export type { TabItem, TabsProps } from './Tabs'

export { TextField } from './TextField'
export type { TextFieldProps } from './TextField'

export { Toggle } from './Toggle'
export type { ToggleProps } from './Toggle'

export { Tooltip } from './Tooltip'
export type { TooltipProps } from './Tooltip'

export { VisuallyHidden } from './VisuallyHidden'
export type { VisuallyHiddenProps } from './VisuallyHidden'

export type { Align, ControlSize, ControlVariant, Placement, Side, Tint } from './types'

/**
 * The bridge to a JS animation library. `@tg/ui` animates in CSS and does not use these, but M1
 * consumers driving `motion`/framer-motion must read TG-009's spring tokens rather than retyping
 * the numbers, and this is where that lives.
 */
export { readSpring, usePrefersReducedMotion } from '../internal/motionTokens'
export type { SpringName, SpringTokens } from '../internal/motionTokens'
