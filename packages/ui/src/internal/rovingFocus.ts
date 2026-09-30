/**
 * Keyboard navigation inside a composite widget, as a pure function over indexes.
 *
 * Every component that owns a list of focusable children (`Menu`, `Tabs`, `RadioGroup`) routes
 * its `keydown` through here, so the arrow / Home / End semantics are defined once and unit
 * tested against fixtures instead of being re-derived per component. Nothing here touches the
 * DOM, which is what makes it testable in `bun test` without a DOM implementation.
 */
export type Orientation = 'horizontal' | 'vertical' | 'both'

export interface RovingInput {
  /** `KeyboardEvent.key`. */
  key: string
  /** Currently focused index, or -1 when nothing in the set is focused yet. */
  current: number
  count: number
  orientation?: Orientation
  /** Wrap past the ends. Telegram's menus and tab strips both wrap. */
  loop?: boolean
  /** Mirror ArrowLeft / ArrowRight for right-to-left writing directions. */
  rtl?: boolean
  /** Indexes for which this returns true are skipped, and can never be landed on. */
  isDisabled?: (index: number) => boolean
}

export interface RovingResult {
  /** Where focus should go. Equal to `current` when nothing moved. */
  index: number
  /** True when the key was consumed and the component must `preventDefault()`. */
  handled: boolean
}

const FORWARD_KEYS: Record<string, Orientation> = {
  ArrowDown: 'vertical',
  ArrowRight: 'horizontal',
}

const BACKWARD_KEYS: Record<string, Orientation> = {
  ArrowUp: 'vertical',
  ArrowLeft: 'horizontal',
}

function axisApplies(orientation: Orientation, axis: Orientation): boolean {
  return orientation === 'both' || orientation === axis
}

function step(input: RovingInput, from: number, delta: number): number {
  const { count, loop = true } = input
  const disabled = input.isDisabled ?? (() => false)
  // At most `count` probes: a set where every entry is disabled must terminate, not spin.
  let index = from
  for (let probe = 0; probe < count; probe += 1) {
    index += delta
    if (index < 0 || index >= count) {
      if (!loop) return from
      index = index < 0 ? count - 1 : 0
    }
    if (!disabled(index)) return index
  }
  return from
}

function edge(input: RovingInput, fromEnd: boolean): number {
  const disabled = input.isDisabled ?? (() => false)
  const order = fromEnd
    ? Array.from({ length: input.count }, (_, i) => input.count - 1 - i)
    : Array.from({ length: input.count }, (_, i) => i)
  for (const index of order) {
    if (!disabled(index)) return index
  }
  return input.current
}

export function nextFocusIndex(input: RovingInput): RovingResult {
  const { key, current, count, orientation = 'vertical', rtl = false } = input
  const unhandled: RovingResult = { index: current, handled: false }
  if (count <= 0) return unhandled

  if (key === 'Home') return { index: edge(input, false), handled: true }
  if (key === 'End') return { index: edge(input, true), handled: true }

  const mirrored = rtl && (key === 'ArrowLeft' || key === 'ArrowRight')
  const effectiveKey = mirrored ? (key === 'ArrowLeft' ? 'ArrowRight' : 'ArrowLeft') : key

  const forwardAxis = FORWARD_KEYS[effectiveKey]
  if (forwardAxis !== undefined && axisApplies(orientation, forwardAxis)) {
    // From "nothing focused", a forward key lands on the first enabled entry rather than the
    // second one, which is what `step` from -1 would give for a disabled index 0.
    const from = current < 0 ? -1 : current
    return { index: step({ ...input, count }, from, 1), handled: true }
  }

  const backwardAxis = BACKWARD_KEYS[effectiveKey]
  if (backwardAxis !== undefined && axisApplies(orientation, backwardAxis)) {
    const from = current < 0 ? count : current
    return { index: step({ ...input, count }, from, -1), handled: true }
  }

  return unhandled
}

/**
 * First-character typeahead, the behaviour a native menu has and a `<div role="menu">` does not.
 * Matching restarts after `current` so repeated presses of the same letter cycle.
 */
export function typeaheadIndex(labels: readonly string[], query: string, current: number): number {
  const needle = query.toLowerCase()
  if (needle === '') return -1
  for (let offset = 1; offset <= labels.length; offset += 1) {
    const index = (current + offset + labels.length) % labels.length
    const label = labels[index]
    if (label !== undefined && label.toLowerCase().startsWith(needle)) return index
  }
  return -1
}
