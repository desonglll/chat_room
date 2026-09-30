/**
 * Tab order as a pure function over candidate descriptors.
 *
 * `useFocusTrap` collects real elements and maps them onto `FocusCandidate`, so the cycling
 * rule — the part that is easy to get subtly wrong — is unit tested without a DOM.
 *
 * Positive `tabIndex` values are deliberately NOT re-sorted. Inside a trap the author controls
 * the DOM order, and honouring positive tabindex would mean reproducing the whole
 * sequential-focus-navigation algorithm for an anti-pattern this package never emits.
 */
export interface FocusCandidate {
  tabIndex: number
  disabled: boolean
  hidden: boolean
}

export function isTabbable(candidate: FocusCandidate): boolean {
  return !candidate.disabled && !candidate.hidden && candidate.tabIndex >= 0
}

export function tabbableIndexes(candidates: readonly FocusCandidate[]): number[] {
  const out: number[] = []
  candidates.forEach((candidate, index) => {
    if (isTabbable(candidate)) out.push(index)
  })
  return out
}

/**
 * Index of the next tab stop, wrapping at both ends. Returns -1 when the set has no tab stop,
 * which is the signal for the caller to keep focus on the container itself.
 */
export function nextTabStop(candidates: readonly FocusCandidate[], current: number, backwards: boolean): number {
  const stops = tabbableIndexes(candidates)
  if (stops.length === 0) return -1

  const position = stops.indexOf(current)
  if (position === -1) {
    // Focus is on the container or on something untabbable: enter at the appropriate end.
    return backwards ? (stops[stops.length - 1] as number) : (stops[0] as number)
  }
  const next = (position + (backwards ? -1 : 1) + stops.length) % stops.length
  return stops[next] as number
}
