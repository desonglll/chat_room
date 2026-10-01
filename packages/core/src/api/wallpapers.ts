/** TG-507 wallpapers (`/api/users/me/wallpapers`): global and per-chat, presets/colour/gradient/image. */
import { ApiError, encodePathSegment, type ApiClient, type FetchLike } from './http'

export type WallpaperKind = 'preset' | 'color' | 'gradient' | 'image'

export interface Wallpaper {
  /** `global` or a chat id. */
  scope: string
  kind: WallpaperKind
  preset?: string
  colors: string[]
  /** Owner-only; fetch with the session token. */
  image_url?: string
  blur: boolean
  dim: number
  updated_at: string
}

export interface WallpaperWrite {
  kind: Exclude<WallpaperKind, 'image'>
  preset?: string
  colors?: string[]
  blur?: boolean
  dim?: number
}

export const GLOBAL_WALLPAPER = 'global'

export interface WallpapersApi {
  list(): Promise<Wallpaper[]>
  put(scope: string, write: WallpaperWrite): Promise<Wallpaper>
  upload(scope: string, file: Blob, options: { blur: boolean; dim: number }): Promise<Wallpaper>
  reset(scope: string): Promise<void>
  /** The image bytes (owner-only endpoint), for an object URL. */
  image(url: string): Promise<Blob>
}

/** `fetchImpl` carries the multipart upload (the JSON client only sends JSON bodies). */
export function createWallpapersApi(
  client: ApiClient,
  token: () => string | null,
  fetchImpl: FetchLike,
): WallpapersApi {
  const auth = () => {
    const value = token()
    return value ? { token: value } : {}
  }
  const path = (scope: string) => `/api/users/me/wallpapers/${encodePathSegment(scope)}`
  return {
    list: () => client.json<Wallpaper[]>('GET', '/api/users/me/wallpapers', auth()),
    put: (scope, write) => client.json<Wallpaper>('PUT', path(scope), { ...auth(), body: write }),
    upload: async (scope, file, options) => {
      const form = new FormData()
      form.append('file', file)
      form.append('blur', String(options.blur))
      form.append('dim', String(options.dim))
      const url = `${path(scope)}/image`
      const value = token()
      const response = await fetchImpl(url, {
        method: 'POST',
        cache: 'no-store',
        headers: { Accept: 'application/json', ...(value ? { Authorization: `Bearer ${value}` } : {}) },
        body: form,
      })
      if (!response.ok) throw new ApiError(response.status, url, response.statusText)
      return (await response.json()) as Wallpaper
    },
    reset: async (scope) => {
      await client.request('DELETE', path(scope), auth())
    },
    image: async (url) => (await client.request('GET', url, auth())).blob(),
  }
}

/** The wallpaper a chat shows: its own override, else the global one, else none. */
export function effectiveWallpaper(wallpapers: readonly Wallpaper[], chatId: string): Wallpaper | null {
  return (
    wallpapers.find((wallpaper) => wallpaper.scope === chatId) ??
    wallpapers.find((wallpaper) => wallpaper.scope === GLOBAL_WALLPAPER) ??
    null
  )
}
