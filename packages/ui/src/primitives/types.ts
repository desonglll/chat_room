/**
 * Prop vocabulary shared by more than one primitive. Kept in one file so that `size="md"` means
 * the same thing on a `Button` as on an `IconButton`, and so the union can be widened in one
 * place instead of five.
 *
 * Nothing here names a business concept: these are shapes and scales, not chats or messages.
 */
export type ControlSize = 'sm' | 'md' | 'lg'

/**
 * Telegram has exactly four button treatments: the accent-filled primary, a soft accent tint,
 * a plain text button, and the destructive variant.
 */
export type ControlVariant = 'filled' | 'tonal' | 'text' | 'danger'

/** Which token supplies the ink of a decorative element (ripple, spinner). */
export type Tint = 'default' | 'accent' | 'inverse' | 'muted'

export type { Align, Placement, Side } from '../internal/positioning'
