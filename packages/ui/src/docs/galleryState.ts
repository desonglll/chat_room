/**
 * Gallery state lives in the URL fragment, so any combination of page x theme x accent x wallpaper
 * is a link you can paste into a review. This is TG-009's `preview.html` approach, kept on purpose:
 * a reviewer comparing against Telegram needs to say "this exact state", not "click three things".
 *
 * Pure functions, unit tested in `galleryState.test.ts`.
 */
export type ThemeMode = 'day' | 'night' | 'split'

/** The empty accent is Telegram's factory accent, which the theme files declare directly. */
export const ACCENTS = ['', 'blue', 'cyan', 'green', 'orange', 'pink', 'purple'] as const

export const WALLPAPERS = [
  'none',
  'default',
  'sunset',
  'meadow',
  'orchid',
  'lagoon',
  'haze',
  'forest-night',
  'midnight-blue',
  'deep-teal',
  'pine',
  'plum-night',
  'ember',
  'mulberry',
] as const

export interface GalleryState {
  page: string
  theme: ThemeMode
  /** One of `ACCENTS`. */
  accent: string
  /** One of `WALLPAPERS`. */
  wallpaper: string
}

const THEME_MODES: readonly ThemeMode[] = ['day', 'night', 'split']

function pick<T extends string>(value: string | undefined, allowed: readonly T[], fallback: T): T {
  return value !== undefined && (allowed as readonly string[]).includes(value) ? (value as T) : fallback
}

export function parseFragment(fragment: string, pages: readonly string[]): GalleryState {
  const query = new URLSearchParams(fragment.replace(/^#/u, ''))
  const fallbackPage = pages[0] ?? ''
  return {
    page: pick(query.get('page') ?? undefined, pages, fallbackPage),
    theme: pick(query.get('theme') ?? undefined, THEME_MODES, 'split'),
    accent: pick(query.get('accent') ?? undefined, ACCENTS, ''),
    wallpaper: pick(query.get('wallpaper') ?? undefined, WALLPAPERS, 'none'),
  }
}

export function toFragment(state: GalleryState): string {
  const query = new URLSearchParams()
  query.set('page', state.page)
  query.set('theme', state.theme)
  // Omitted rather than written empty, so the default state produces the shortest link.
  if (state.accent !== '') query.set('accent', state.accent)
  if (state.wallpaper !== 'none') query.set('wallpaper', state.wallpaper)
  return `#${query.toString()}`
}

/** The themes a given mode paints, in pane order. */
export function panesFor(mode: ThemeMode): readonly ('day' | 'night')[] {
  return mode === 'split' ? ['day', 'night'] : [mode]
}
