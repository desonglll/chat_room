/**
 * `/addstickers/:shortName` — the share link of a sticker set. Opens the set preview over
 * the workspace and replaces the URL with `/`, like Telegram's `t.me/addstickers/<name>`.
 */
import { useEffect } from 'react'
import { Navigate, useParams } from 'react-router-dom'
import { openStickerSet } from './overlayStore'

export function StickerSetLinkRoute() {
  const { shortName = '' } = useParams()
  useEffect(() => {
    if (shortName) openStickerSet(shortName)
  }, [shortName])
  return <Navigate to="/" replace />
}
