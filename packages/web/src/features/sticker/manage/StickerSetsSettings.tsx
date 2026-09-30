/**
 * Settings → 贴纸: the installed sets in the viewer's order, with reorder (drag the handle,
 * or ↑/↓ for keyboard users), archive, remove and share; archived sets below with
 * unarchive; and "add by name or link". Every write goes through `stickerLibrary`, whose
 * answer is the server's whole library, so the list is always the truth after a write.
 */
import { useEffect, useMemo, useState, type DragEvent, type FormEvent } from 'react'
import { selectActiveStickerSets, selectArchivedStickerSets, stickerStore, type StickerSet } from '@tg/core'
import { Button, IconButton, TextField } from '@tg/ui'
import { useStore } from 'zustand/react'
import { ArrowDownGlyph, ArrowUpGlyph, CloseGlyph, HandleGlyph, LinkGlyph } from '../icons'
import { StickerView } from '../StickerView'
import { stickerLibrary } from '../stickerLibrary'
import { openStickerSet } from '../overlayStore'
import { copyStickerSetLink } from './copyLink'
import './manage.css'
import { moveId, shortNameFromLink } from './setOrder'

function SetThumb({ set }: { set: StickerSet }) {
  const first = set.stickers[0]
  return (
    <span className="tg-sticker-manage__thumb">
      {first ? (
        <StickerView
          src={first.file_url}
          format={first.format}
          size={40}
          label={first.emoji}
          autoplay={false}
          loop={false}
        />
      ) : null}
    </span>
  )
}

export function StickerSetsSettings() {
  const all = useStore(stickerStore, (state) => state.sets)
  const active = useMemo(() => selectActiveStickerSets({ sets: all }), [all])
  const archived = useMemo(() => selectArchivedStickerSets({ sets: all }), [all])
  const [dragging, setDragging] = useState<number | null>(null)
  const [notice, setNotice] = useState('')
  const [link, setLink] = useState('')

  useEffect(() => {
    void stickerLibrary()
      .ensureLoaded()
      .catch(() => setNotice('贴纸加载失败'))
  }, [])

  const guard = (promise: Promise<unknown>) => void promise.catch(() => setNotice('操作失败，请重试'))
  const move = (from: number, to: number) => {
    if (from === to || to < 0 || to >= active.length) return
    guard(
      stickerLibrary().reorder(
        moveId(
          active.map((set) => set.id),
          from,
          to,
        ),
      ),
    )
  }
  const share = (set: StickerSet) =>
    void copyStickerSetLink(set.short_name).then((ok) =>
      setNotice(ok ? `已复制「${set.title}」的链接` : '无法访问剪贴板'),
    )

  const onDrop = (event: DragEvent, index: number) => {
    event.preventDefault()
    if (dragging !== null) move(dragging, index)
    setDragging(null)
  }
  const onAdd = (event: FormEvent) => {
    event.preventDefault()
    const name = shortNameFromLink(link)
    if (!name) return setNotice('请输入贴纸包名称或 addstickers 链接')
    setLink('')
    openStickerSet(name)
  }

  return (
    <section className="tg-sticker-manage" aria-label="贴纸包管理">
      <form className="tg-sticker-manage__add" onSubmit={onAdd}>
        <TextField
          size="sm"
          fullWidth
          value={link}
          placeholder="贴纸包名称或链接"
          aria-label="贴纸包名称或链接"
          onChange={(event) => setLink(event.target.value)}
        />
        <Button type="submit" size="sm" variant="tonal">
          查看
        </Button>
      </form>
      {notice ? (
        <p className="tg-sticker-manage__notice" role="status">
          {notice}
        </p>
      ) : null}
      <h3 className="tg-sticker-manage__heading">我的贴纸包</h3>
      {active.length === 0 ? <p className="tg-sticker-manage__empty">还没有贴纸包</p> : null}
      <ol className="tg-sticker-manage__list">
        {active.map((set, index) => (
          <li
            key={set.id}
            className="tg-sticker-manage__row"
            data-dragging={dragging === index ? 'true' : undefined}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => onDrop(event, index)}
          >
            <span
              className="tg-sticker-manage__handle"
              draggable
              aria-hidden="true"
              onDragStart={(event) => {
                event.dataTransfer.effectAllowed = 'move'
                setDragging(index)
              }}
              onDragEnd={() => setDragging(null)}
            >
              <HandleGlyph />
            </span>
            <button type="button" className="tg-sticker-manage__open" onClick={() => openStickerSet(set.short_name)}>
              <SetThumb set={set} />
              <span className="tg-sticker-manage__text">
                <span className="tg-sticker-manage__title">{set.title}</span>
                <span className="tg-sticker-manage__count">{set.stickers.length} 个贴纸</span>
              </span>
            </button>
            <IconButton
              size="sm"
              label={`上移 ${set.title}`}
              disabled={index === 0}
              onClick={() => move(index, index - 1)}
            >
              <ArrowUpGlyph />
            </IconButton>
            <IconButton
              size="sm"
              label={`下移 ${set.title}`}
              disabled={index === active.length - 1}
              onClick={() => move(index, index + 1)}
            >
              <ArrowDownGlyph />
            </IconButton>
            <IconButton size="sm" label={`复制 ${set.title} 的链接`} onClick={() => share(set)}>
              <LinkGlyph />
            </IconButton>
            <Button size="sm" variant="text" onClick={() => guard(stickerLibrary().setArchived(set.id, true))}>
              归档
            </Button>
            <IconButton
              size="sm"
              variant="danger"
              label={`移除 ${set.title}`}
              onClick={() => guard(stickerLibrary().uninstall(set.id))}
            >
              <CloseGlyph />
            </IconButton>
          </li>
        ))}
      </ol>
      {archived.length > 0 ? (
        <>
          <h3 className="tg-sticker-manage__heading">已归档</h3>
          <ol className="tg-sticker-manage__list">
            {archived.map((set) => (
              <li key={set.id} className="tg-sticker-manage__row" data-archived="true">
                <button
                  type="button"
                  className="tg-sticker-manage__open"
                  onClick={() => openStickerSet(set.short_name)}
                >
                  <SetThumb set={set} />
                  <span className="tg-sticker-manage__text">
                    <span className="tg-sticker-manage__title">{set.title}</span>
                    <span className="tg-sticker-manage__count">{set.stickers.length} 个贴纸</span>
                  </span>
                </button>
                <Button size="sm" variant="tonal" onClick={() => guard(stickerLibrary().setArchived(set.id, false))}>
                  取消归档
                </Button>
                <IconButton
                  size="sm"
                  variant="danger"
                  label={`移除 ${set.title}`}
                  onClick={() => guard(stickerLibrary().uninstall(set.id))}
                >
                  <CloseGlyph />
                </IconButton>
              </li>
            ))}
          </ol>
        </>
      ) : null}
    </section>
  )
}
