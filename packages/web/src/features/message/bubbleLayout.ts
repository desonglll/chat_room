/**
 * Pure bubble geometry: which frame, which corners, whether there is a tail, and where the
 * timestamp goes. Everything visual that depends on a combination of facts is decided here,
 * so it is testable without a DOM and the component only maps the answer to attributes.
 *
 * The corner grammar is tokens/semantic.css "Bubble geometry", transcribed from tweb:
 *
 *   incoming, middle   TL merged  TR full  BR full  BL merged
 *   incoming, first    TL full    TR full  BR full  BL merged
 *   incoming, last     TL merged  TR full  BR full  BL 0 + tail
 *   incoming, single   TL full    TR full  BR full  BL 0 + tail
 *   outgoing           the mirror image
 *
 * `last` without a tail (borderless media) keeps the full radius on that corner.
 */
import type { ContentFrame, MetaPlacement } from './content/contentTypes'
import type { GroupPosition } from './types'

export type Corner = 'full' | 'merged' | 'none'

export interface Corners {
  topStart: Corner
  topEnd: Corner
  bottomEnd: Corner
  bottomStart: Corner
}

/**
 * `inline` — spacer at the end of the content's text; `reactions` — spacer at the end of
 * the reaction row; `overlay` — pill over the media, no spacer.
 */
export type MetaMode = 'inline' | 'reactions' | 'overlay'

export interface LayoutInput {
  groupPosition: GroupPosition
  isOutgoing: boolean
  frame: ContentFrame
  metaPlacement: MetaPlacement
  hasSenderName: boolean
  hasForward: boolean
  hasReply: boolean
  hasReactions: boolean
}

export interface BubbleLayout {
  /** `bubble` | `media` (borderless) | `bare` — after the fallback rules. */
  frame: ContentFrame
  tail: boolean
  /** Physical corners, already mirrored for outgoing. */
  corners: { topLeft: Corner; topRight: Corner; bottomRight: Corner; bottomLeft: Corner }
  metaMode: MetaMode
}

const opensGroup = (position: GroupPosition) => position === 'single' || position === 'first'
const closesGroup = (position: GroupPosition) => position === 'single' || position === 'last'

/** Corners relative to the sender's side ("start" = the side the tail is on). */
export function groupCorners(position: GroupPosition, tail: boolean): Corners {
  const bottomStart: Corner = closesGroup(position) ? (tail ? 'none' : 'full') : 'merged'
  return {
    topStart: opensGroup(position) ? 'full' : 'merged',
    topEnd: 'full',
    bottomEnd: 'full',
    bottomStart,
  }
}

export function computeBubbleLayout(input: LayoutInput): BubbleLayout {
  const decorated = input.hasSenderName || input.hasForward || input.hasReply || input.hasReactions
  // A borderless medium only stays borderless alone; anything sharing the bubble needs a fill.
  const frame: ContentFrame = input.frame === 'media' && decorated ? 'bubble' : input.frame
  const tail = frame === 'bubble' && closesGroup(input.groupPosition)
  const c = groupCorners(input.groupPosition, tail)
  const corners = input.isOutgoing
    ? { topLeft: c.topEnd, topRight: c.topStart, bottomRight: c.bottomStart, bottomLeft: c.bottomEnd }
    : { topLeft: c.topStart, topRight: c.topEnd, bottomRight: c.bottomEnd, bottomLeft: c.bottomStart }
  const metaMode: MetaMode = input.hasReactions ? 'reactions' : input.metaPlacement === 'overlay' ? 'overlay' : 'inline'
  return { frame, tail, corners, metaMode }
}
