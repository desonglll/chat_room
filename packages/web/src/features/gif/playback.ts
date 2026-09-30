/**
 * The GIF autoplay policy (TG-305 acceptance: off-screen GIFs stop decoding; no autoplay
 * under `prefers-reduced-motion` or Save-Data).
 *
 * `createGifPlayback` is a small state machine over an injected media element, so the
 * policy is tested without a DOM: it plays only while the GIF is on screen AND either
 * autoplay is allowed or the viewer tapped play; leaving the viewport always pauses.
 */

export interface AutoplayEnvironment {
  /** `prefers-reduced-motion: reduce`. */
  reducedMotion: boolean
  /** `navigator.connection.saveData` — the browser's data-saver / battery-saver hint. */
  saveData: boolean
}

export function autoplayAllowed(environment: AutoplayEnvironment): boolean {
  return !environment.reducedMotion && !environment.saveData
}

export interface PlaybackState {
  visible: boolean
  /** The viewer tapped play (and has not tapped pause since). */
  started: boolean
  /** The viewer tapped pause on an autoplaying GIF. */
  stopped: boolean
}

export function shouldPlay(state: PlaybackState, allowed: boolean): boolean {
  if (!state.visible || state.stopped) return false
  return allowed || state.started
}

export interface PlayableMedia {
  play(): void
  pause(): void
}

export interface GifPlayback {
  setVisible(visible: boolean): void
  /** A tap on the GIF: play when paused, pause when playing. */
  toggle(): void
  isPlaying(): boolean
  /** Swap the element (a re-render may replace it); applies the current decision. */
  attach(media: PlayableMedia | null): void
}

export function createGifPlayback(
  allowed: boolean,
  onChange: (playing: boolean) => void = () => undefined,
): GifPlayback {
  const state: PlaybackState = { visible: false, started: false, stopped: false }
  let media: PlayableMedia | null = null
  let playing = false

  const apply = () => {
    const next = shouldPlay(state, allowed)
    if (media) {
      if (next) media.play()
      else media.pause()
    }
    if (next !== playing) {
      playing = next
      onChange(next)
    }
  }

  return {
    setVisible(visible) {
      state.visible = visible
      apply()
    },
    toggle() {
      if (playing) {
        state.started = false
        state.stopped = true
      } else {
        state.started = true
        state.stopped = false
      }
      apply()
    },
    isPlaying: () => playing,
    attach(next) {
      media = next
      apply()
    },
  }
}
