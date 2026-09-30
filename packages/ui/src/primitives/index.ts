/**
 * `@tg/ui` primitives — the package's public entry point.
 *
 * Rules (architecture.md section 3, agent-protocol.md section 5): a primitive knows nothing
 * about Chat, Message or User; it consumes only the `semantic` CSS variable layer, never a
 * `primitive` value; every animation has a `prefers-reduced-motion` fallback.
 *
 * TG-010 owns the 19 atoms (`Button` `IconButton` `Ripple` `TextField` `Toggle` `Checkbox`
 * `Radio` `Menu` `ContextMenu` `Popover` `Modal` `Sheet` `Tooltip` `Tabs` `Avatar` `Badge`
 * `Spinner` `Skeleton` `ScrollArea`) and adds them here. TG-009 owns `../tokens/`.
 *
 * `VisuallyHidden` below is the skeleton's example primitive: it needs no design tokens, so it
 * can exist before TG-009, and it is not one of the 19 atoms, so TG-010 will not collide here.
 */
export { VisuallyHidden } from './VisuallyHidden'
export type { VisuallyHiddenProps } from './VisuallyHidden'
