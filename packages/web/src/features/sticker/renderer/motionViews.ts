/**
 * Motion views: stickers the browser animates by itself (WebM `<video>`), registered with the
 * sticker manager so they obey the same playback policy as TGS — viewport pause, hidden-tab
 * pause, reduced motion, and one shared concurrency cap.
 *
 * Every motion view is its own group in the cap: each `<video>` runs its own decoder, so two
 * copies of one WebM cost twice (unlike TGS, where copies share one render).
 */
import type { PlaybackCandidate } from './playbackPolicy'
import type { ViewportWatcher } from './viewportWatcher'

export interface MotionViewOptions {
  /** Observed for viewport intersection. */
  element: Element
  /** Identity of the media (usually its URL); informational. */
  key: string
  loop: boolean
  autoplay: boolean
  /**
   * Admission changed. `playing` false with `reducedMotion` true means "show the first frame",
   * otherwise "hold the current frame". Called once right after attach.
   */
  onPlayback(playing: boolean, reducedMotion: boolean): void
}

export interface MotionView {
  /** Play from the start (tap-to-play, or replay a finished non-looping sticker). */
  replay(): void
  /** The media reached its end (non-looping playback). */
  ended(): void
  destroy(): void
}

interface MotionRecord {
  readonly id: number
  readonly options: MotionViewOptions
  visible: boolean
  visibleSince: number
  replayRequested: boolean
  finished: boolean
  playing: boolean | null
  reducedMotion: boolean
}

export interface MotionViews {
  attach(options: MotionViewOptions): MotionView
  candidates(): PlaybackCandidate[]
  apply(admitted: ReadonlySet<number>, reducedMotion: boolean): void
  count(): { views: number; playing: number }
}

export function createMotionViews(deps: {
  viewport: ViewportWatcher
  nextId(): number
  nextVisible(): number
  schedulePlan(): void
}): MotionViews {
  const records = new Map<number, MotionRecord>()
  const wantsMotion = (record: MotionRecord) =>
    record.replayRequested || (record.options.autoplay && (record.options.loop || !record.finished))

  return {
    attach(options) {
      const record: MotionRecord = {
        id: deps.nextId(),
        options,
        visible: false,
        visibleSince: 0,
        replayRequested: false,
        finished: false,
        playing: null,
        reducedMotion: false,
      }
      records.set(record.id, record)
      const unobserve = deps.viewport.observe(options.element, (visible) => {
        if (visible && !record.visible) record.visibleSince = deps.nextVisible()
        record.visible = visible
        deps.schedulePlan()
      })
      deps.schedulePlan()
      let destroyed = false
      return {
        replay() {
          if (destroyed) return
          record.finished = false
          record.replayRequested = true
          deps.schedulePlan()
        },
        ended() {
          if (destroyed) return
          record.finished = true
          record.replayRequested = false
          deps.schedulePlan()
        },
        destroy() {
          if (destroyed) return
          destroyed = true
          unobserve()
          records.delete(record.id)
          deps.schedulePlan()
        },
      }
    },
    candidates: () =>
      [...records.values()].map((record) => ({
        id: record.id,
        group: `motion#${record.id}`,
        visible: record.visible,
        wantsMotion: wantsMotion(record),
        playing: record.playing === true,
        visibleSince: record.visibleSince,
      })),
    apply(admitted, reducedMotion) {
      for (const record of records.values()) {
        const playing = admitted.has(record.id)
        if (playing === record.playing && reducedMotion === record.reducedMotion) continue
        record.playing = playing
        record.reducedMotion = reducedMotion
        record.options.onPlayback(playing, reducedMotion)
      }
    },
    count() {
      const all = [...records.values()]
      return { views: all.length, playing: all.filter((record) => record.playing === true).length }
    },
  }
}
