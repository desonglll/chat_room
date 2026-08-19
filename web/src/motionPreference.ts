export function preferredScrollBehavior(
  reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches,
): ScrollBehavior {
  return reduced ? 'auto' : 'smooth'
}
