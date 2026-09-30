/**
 * Class-name join. Deliberately not a dependency: the whole of `clsx` that this package needs
 * is four lines, and `@tg/ui` is meant to ship with zero runtime dependencies.
 */
export type ClassValue = string | false | null | undefined

export function cx(...values: ClassValue[]): string {
  let out = ''
  for (const value of values) {
    if (value) out = out === '' ? value : `${out} ${value}`
  }
  return out
}
