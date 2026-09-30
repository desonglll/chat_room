import type { DocPage } from '../kit'
import { buttonPages } from './buttons'
import { dialogPages } from './dialogs'
import { displayPages } from './display'
import { inputPages } from './inputs'
import { navigationPages } from './navigation'
import { overlayPages } from './overlays'

/**
 * Every one of the 19 atoms has a page, in the order the roadmap card lists them.
 * `pageCoverage.test.ts` asserts that, so a component cannot be added without one.
 */
export const DOC_PAGES: readonly DocPage[] = [
  ...buttonPages,
  ...inputPages,
  ...overlayPages,
  ...dialogPages,
  ...navigationPages,
  ...displayPages,
]

export const DOC_PAGE_IDS: readonly string[] = DOC_PAGES.map((page) => page.id)
