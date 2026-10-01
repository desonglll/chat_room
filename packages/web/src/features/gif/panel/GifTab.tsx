/**
 * The GIF tab of the media panel (registered by `../register.ts` as `id: 'gif'`): the
 * account's saved GIFs, then GIFs recently sent into its chats, in a virtual masonry grid.
 * A click sends through `POST /api/chats/:id/gif-messages` (by reference), a right click
 * offers 删除 / 保存, and 上传 sends a new GIF or MP4/WebM animation from disk.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { composerStore, type RecentGif, type SavedGif, type SendGifSource } from '@tg/core'
import { Button, Menu, Spinner, type MenuItem } from '@tg/ui'
import { useStore } from 'zustand/react'
import type { MediaPanelTabContext } from '../../sticker/panel/mediaPanelTabs'
import { gifErrorMessage, gifLibrary, gifStore } from '../gifLibrary'
import { GifGrid, type GifGridItem } from './GifGrid'
import type { GifSection } from './gifPanelLayout'
import { t } from '../../../i18n/index'
import '../gif.css'
import './gifTab.css'

type Entry = GifGridItem & ({ kind: 'saved'; gif: SavedGif } | { kind: 'recent'; gif: RecentGif })

function aspectOf(gif: SavedGif | RecentGif, measured: Record<string, number>): number | null {
  if (gif.width && gif.height) return gif.width / gif.height
  return measured[gif.file_url] ?? null
}

export function buildGifSections(
  saved: readonly SavedGif[],
  recent: readonly RecentGif[],
  measured: Record<string, number>,
): GifSection<Entry>[] {
  const savedEntries: Entry[] = saved.map((gif) => ({
    kind: 'saved',
    gif,
    key: gif.id,
    src: gif.file_url,
    mimeType: gif.mime_type,
    aspect: aspectOf(gif, measured),
  }))
  const recentEntries: Entry[] = recent.map((gif) => ({
    kind: 'recent',
    gif,
    key: gif.message_id,
    src: gif.file_url,
    mimeType: gif.mime_type,
    aspect: aspectOf(gif, measured),
  }))
  return [
    { id: 'saved', title: t('w.gif.19571c'), items: savedEntries },
    { id: 'recent', title: t('w.gif.9bc2e3'), items: recentEntries },
  ]
}

export function GifTab({ chatId, canSend, close }: MediaPanelTabContext) {
  const status = useStore(gifStore, (state) => state.status)
  const saved = useStore(gifStore, (state) => state.saved)
  const recent = useStore(gifStore, (state) => state.recent)
  const aspects = useStore(gifStore, (state) => state.aspects)
  const [menu, setMenu] = useState<{ entry: Entry; x: number; y: number } | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const fileRef = useRef<HTMLInputElement | null>(null)
  const sections = useMemo(() => buildGifSections(saved, recent, aspects), [saved, recent, aspects])

  useEffect(() => {
    void gifLibrary()
      .ensureLoaded()
      .catch(() => undefined)
  }, [])

  const finish = (result: { ok: true } | { ok: false; code: string }) => {
    setBusy(false)
    if (!result.ok) {
      setNotice(gifErrorMessage(result.code))
      return
    }
    // Like a sticker, a sent GIF consumes the reply target.
    composerStore.getState().setReplyTarget(chatId, null)
    close()
  }
  const replyTo = () => composerStore.getState().drafts[chatId]?.replyToMessageId ?? null
  const send = (entry: Entry) => {
    if (!canSend || busy) return
    const source: SendGifSource =
      entry.kind === 'saved' ? { saved_gif_id: entry.gif.id } : { message_id: entry.gif.message_id }
    setBusy(true)
    setNotice(null)
    void gifLibrary().send(chatId, source, replyTo()).then(finish)
  }
  const upload = (file: File) => {
    setBusy(true)
    setNotice(null)
    void gifLibrary().upload(chatId, file, replyTo()).then(finish)
  }

  const menuItems = useMemo<MenuItem[]>(() => {
    if (!menu) return []
    const { entry } = menu
    const items: MenuItem[] = [
      { id: 'send', label: t('w.gif.f02ec6'), disabled: !canSend, onSelect: () => send(entry) },
    ]
    if (entry.kind === 'saved') {
      items.push({
        id: 'remove',
        label: t('w.gif.f014c1'),
        danger: true,
        onSelect: () => void gifLibrary().remove(entry.gif.id),
      })
    } else if (!saved.some((gif) => gif.file_url === entry.gif.file_url)) {
      items.push({ id: 'save', label: t('w.gif.ca767d'), onSelect: () => void gifLibrary().save(entry.gif.message_id) })
    }
    return items
    // `send` closes over the latest props; the menu is rebuilt whenever it opens.
  }, [menu, canSend, saved])

  const empty = saved.length === 0 && recent.length === 0
  return (
    <div className="tg-gif-tab">
      <div className="tg-gif-tab__toolbar">
        <Button size="sm" variant="tonal" disabled={!canSend || busy} onClick={() => fileRef.current?.click()}>
          {t('w.gif.132b72')}
        </Button>
        {busy ? <Spinner size="sm" label={t('w.gif.a11211')} /> : null}
        <input
          ref={fileRef}
          type="file"
          accept="image/gif,video/mp4,video/webm"
          hidden
          onChange={(event) => {
            const file = event.target.files?.[0]
            event.target.value = ''
            if (file) upload(file)
          }}
        />
      </div>
      {notice ? (
        <p className="tg-gif-tab__notice" role="alert">
          {notice}
        </p>
      ) : null}
      {status === 'loading' && empty ? (
        <div className="tg-gif-tab__state">
          <Spinner size="md" label={t('w.gif.864bf0')} />
        </div>
      ) : empty ? (
        <p className="tg-gif-tab__state">{status === 'error' ? t('w.gif.bb9d9f') : t('w.gif.eee0ac')}</p>
      ) : (
        <GifGrid
          sections={sections}
          disabled={!canSend || busy}
          onPick={send}
          onMenu={(entry, point) => setMenu({ entry, ...point })}
          onAspect={(entry, aspect) => gifLibrary().reportAspect(entry.src, aspect)}
        />
      )}
      <Menu
        open={menu !== null}
        onClose={() => setMenu(null)}
        anchor={menu ? { x: menu.x, y: menu.y } : null}
        items={menuItems}
        aria-label={t('w.gif.4be557')}
      />
    </div>
  )
}
