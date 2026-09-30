/**
 * Which sticker views may animate right now. Pure, so the whole policy is unit-tested
 * without a browser.
 *
 * - Nothing animates while the page is hidden or the user prefers reduced motion.
 * - A view animates only while it intersects the viewport and wants motion
 *   (autoplay, or a replay the user asked for).
 * - Views of one sticker at one size share a render group, and the cap counts groups, not
 *   views: a second copy of a playing sticker costs a blit, not a render.
 * - Groups already playing keep their slot (no flicker when a new sticker scrolls in), then
 *   the longest-visible groups win. Views over the cap show the static first frame.
 */

export interface PlaybackCandidate {
  id: number
  group: string
  visible: boolean
  wantsMotion: boolean
  /** Currently admitted — the stability tie-breaker. */
  playing: boolean
  /** Monotonic order in which the view last became visible. */
  visibleSince: number
}

export interface PlaybackEnvironment {
  hidden: boolean
  reducedMotion: boolean
  maxGroups: number
}

export function admitPlayback(candidates: readonly PlaybackCandidate[], env: PlaybackEnvironment): Set<number> {
  const admitted = new Set<number>()
  if (env.hidden || env.reducedMotion || env.maxGroups <= 0) return admitted
  const eligible = candidates
    .filter((candidate) => candidate.visible && candidate.wantsMotion)
    .sort((a, b) => Number(b.playing) - Number(a.playing) || a.visibleSince - b.visibleSince || a.id - b.id)
  const groups = new Set<string>()
  for (const candidate of eligible) {
    if (!groups.has(candidate.group)) {
      if (groups.size >= env.maxGroups) continue
      groups.add(candidate.group)
    }
    admitted.add(candidate.id)
  }
  return admitted
}
