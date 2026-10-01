/**
 * TG-507: how a wallpaper becomes attributes and custom properties on the wallpaper layer.
 * Presets are token presets (`data-tg-wallpaper`); colours and gradients feed the same mesh
 * through `--tg-wallpaper-1..4`; images set `--tg-wallpaper-image`. Blur and dim are custom
 * properties the CSS applies — no colour is decided here.
 */
import type { Wallpaper } from '@tg/core'

export interface WallpaperLayerProps {
  'data-tg-wallpaper'?: string
  'data-blur'?: ''
  /** Custom properties only (`--tg-wallpaper-*`). */
  style: Record<string, string>
}

export function wallpaperLayer(wallpaper: Wallpaper | null, imageObjectUrl: string | undefined): WallpaperLayerProps {
  if (!wallpaper) return { style: {} }
  const dim = { '--tg-wallpaper-dim': String(Math.max(0, Math.min(80, wallpaper.dim)) / 100) }
  switch (wallpaper.kind) {
    case 'preset':
      return { 'data-tg-wallpaper': wallpaper.preset || 'default', style: dim }
    case 'color': {
      const colour = wallpaper.colors[0] ?? ''
      return {
        'data-tg-wallpaper': 'custom',
        style: {
          ...dim,
          '--tg-wallpaper-1': colour,
          '--tg-wallpaper-2': colour,
          '--tg-wallpaper-3': colour,
          '--tg-wallpaper-4': colour,
        },
      }
    }
    case 'gradient': {
      const [a = '', b = a, c = b, d = c] = wallpaper.colors
      return {
        'data-tg-wallpaper': 'custom',
        style: { ...dim, '--tg-wallpaper-1': a, '--tg-wallpaper-2': b, '--tg-wallpaper-3': c, '--tg-wallpaper-4': d },
      }
    }
    case 'image':
      return {
        'data-tg-wallpaper': 'image',
        ...(wallpaper.blur ? { 'data-blur': '' as const } : {}),
        style: { ...dim, ...(imageObjectUrl ? { '--tg-wallpaper-photo': `url("${imageObjectUrl}")` } : {}) },
      }
  }
}
