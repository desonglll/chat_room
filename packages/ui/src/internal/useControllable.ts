import { useCallback, useRef, useState } from 'react'

/**
 * One hook for the controlled/uncontrolled split that `Toggle`, `Checkbox`, `RadioGroup` and
 * `Tabs` all need, so the rule is written once: a prop that is `undefined` on first render makes
 * the component uncontrolled for its whole lifetime.
 *
 * Pinning the mode on first render (rather than per render) is what stops the classic bug where
 * a parent passes `value={maybeUndefined}` and the component silently flips between modes,
 * losing state.
 */
export function useControllable<T>(
  controlled: T | undefined,
  fallback: T,
  onChange?: ((value: T) => void) | undefined,
): [T, (value: T) => void] {
  const isControlled = useRef(controlled !== undefined).current
  const [internal, setInternal] = useState<T>(controlled !== undefined ? controlled : fallback)

  const value = isControlled && controlled !== undefined ? controlled : internal

  const set = useCallback(
    (next: T) => {
      if (!isControlled) setInternal(next)
      onChange?.(next)
    },
    [isControlled, onChange],
  )

  return [value, set]
}
