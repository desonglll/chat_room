/**
 * TG-507: the viewer's wallpapers (global + per chat), loaded once per session and updated by
 * the appearance page. Image wallpapers come from an owner-only endpoint, so they are fetched
 * with the session token and shown through object URLs (one per image URL, revoked on change).
 */
import { createStore } from 'zustand/vanilla'
import type { Wallpaper, WallpaperWrite } from '@tg/core'
import { authStore, createWallpapersApi, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'

export const wallpapersApi = createWallpapersApi(
  apiClient,
  () => selectToken(authStore.getState()) || null,
  (input, init) => fetch(input, init),
)

export interface WallpaperState {
  wallpapers: Wallpaper[]
  loaded: boolean
  /** image_url → object URL */
  images: Record<string, string>
}

export const wallpaperStore = createStore<WallpaperState>()(() => ({ wallpapers: [], loaded: false, images: {} }))

function replace(next: Wallpaper | null, scope: string): void {
  wallpaperStore.setState((state) => ({
    wallpapers: [...state.wallpapers.filter((item) => item.scope !== scope), ...(next ? [next] : [])],
  }))
}

export function loadWallpapers(): void {
  if (wallpaperStore.getState().loaded) return
  wallpaperStore.setState({ loaded: true })
  wallpapersApi.list().then(
    (wallpapers) => wallpaperStore.setState({ wallpapers }),
    () => wallpaperStore.setState({ loaded: false }),
  )
}

export async function saveWallpaper(scope: string, write: WallpaperWrite): Promise<void> {
  replace(await wallpapersApi.put(scope, write), scope)
}

export async function uploadWallpaper(scope: string, file: File, options: { blur: boolean; dim: number }) {
  replace(await wallpapersApi.upload(scope, file, options), scope)
}

export async function resetWallpaper(scope: string): Promise<void> {
  await wallpapersApi.reset(scope).catch(() => undefined)
  replace(null, scope)
}

/** Fetch an image wallpaper once and keep its object URL. */
export function ensureImage(url: string): void {
  if (wallpaperStore.getState().images[url] || typeof URL.createObjectURL !== 'function') return
  wallpapersApi.image(url).then(
    (blob) => wallpaperStore.setState((state) => ({ images: { ...state.images, [url]: URL.createObjectURL(blob) } })),
    () => undefined,
  )
}
