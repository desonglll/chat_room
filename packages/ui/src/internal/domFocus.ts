/**
 * The DOM adapter for `focusOrder.ts`: it turns a subtree into `FocusCandidate` descriptors and
 * back. All of the rule lives in the pure module; this file only reads the DOM.
 */
import type { FocusCandidate } from './focusOrder'

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'area[href]',
  'button',
  'input',
  'select',
  'textarea',
  'summary',
  'iframe',
  'object',
  'embed',
  'audio[controls]',
  'video[controls]',
  '[contenteditable]:not([contenteditable="false"])',
  '[tabindex]',
].join(',')

function isHidden(element: HTMLElement): boolean {
  if (element.hidden) return true
  if (element.getAttribute('aria-hidden') === 'true') return true
  // offsetParent is null for display:none and for fixed-position elements; the second case is
  // rescued by the client-rect check, which is cheap because the element is already laid out.
  if (element.offsetParent === null && element.getClientRects().length === 0) return true
  return false
}

export function describe(element: HTMLElement): FocusCandidate {
  const attribute = element.getAttribute('tabindex')
  const tabIndex = attribute === null ? element.tabIndex : Number.parseInt(attribute, 10)
  return {
    tabIndex: Number.isNaN(tabIndex) ? -1 : tabIndex,
    disabled: element.matches(':disabled') || element.getAttribute('aria-disabled') === 'true',
    hidden: isHidden(element),
  }
}

/** Focusable descendants in DOM order. The root itself is included when it is focusable. */
export function collectFocusable(root: HTMLElement): HTMLElement[] {
  const found = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
  if (root.matches(FOCUSABLE_SELECTOR)) found.unshift(root)
  return found
}
