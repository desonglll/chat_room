/**
 * TG-507: the wallpaper behind the open chat — the chat's own, else the global one. It is its
 * own composited layer (absolutely positioned, `contain: strict`, never inside the scroller), so
 * scrolling the message list repaints nothing of it.
 */
import { useEffect } from 'react'
import type { CSSProperties } from 'react'
import { effectiveWallpaper } from '@tg/core'
import { useStore } from 'zustand/react'
import { wallpaperLayer } from './wallpaperModel'
import { ensureImage, loadWallpapers, wallpaperStore } from './wallpaperStore'
import './wallpaper.css'

export function ChatWallpaper({ chatId }: { chatId: string }) {
  const wallpapers = useStore(wallpaperStore, (state) => state.wallpapers)
  const images = useStore(wallpaperStore, (state) => state.images)
  useEffect(loadWallpapers, [])
  const wallpaper = effectiveWallpaper(wallpapers, chatId)
  const imageUrl = wallpaper?.kind === 'image' ? wallpaper.image_url : undefined
  useEffect(() => {
    if (imageUrl) ensureImage(imageUrl)
  }, [imageUrl])
  if (!wallpaper) return null
  const layer = wallpaperLayer(wallpaper, imageUrl ? images[imageUrl] : undefined)
  return <div className="tg-wallpaper" aria-hidden="true" {...layer} style={layer.style as CSSProperties} />
}
