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
import { t } from '../../../i18n/index'

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
      .catch(() => setNotice(t('w.sticker.f9fd05')))
  }, [])

  const guard = (promise: Promise<unknown>) => void promise.catch(() => setNotice(t('w.sticker.51d3cb')))
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
      setNotice(ok ? t('w.sticker.40896f', set.title) : t('w.sticker.91aaca')),
    )

  const onDrop = (event: DragEvent, index: number) => {
    event.preventDefault()
    if (dragging !== null) move(dragging, index)
    setDragging(null)
  }
  const onAdd = (event: FormEvent) => {
    event.preventDefault()
    const name = shortNameFromLink(link)
    if (!name) return setNotice(t('w.sticker.531c7a'))
    setLink('')
    openStickerSet(name)
  }

  return (
    <section className="tg-sticker-manage" aria-label={t('w.sticker.2069bc')}>
      <form className="tg-sticker-manage__add" onSubmit={onAdd}>
        <TextField
          size="sm"
          fullWidth
          value={link}
          placeholder={t('w.sticker.1f0113')}
          aria-label={t('w.sticker.1f0113')}
          onChange={(event) => setLink(event.target.value)}
        />
        <Button type="submit" size="sm" variant="tonal">
          {t('w.sticker.f7acef')}
        </Button>
      </form>
      {notice ? (
        <p className="tg-sticker-manage__notice" role="status">
          {notice}
        </p>
      ) : null}
      <h3 className="tg-sticker-manage__heading">{t('w.sticker.da2c24')}</h3>
      {active.length === 0 ? <p className="tg-sticker-manage__empty">{t('w.sticker.42c27c')}</p> : null}
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
                <span className="tg-sticker-manage__count">
                  {set.stickers.length} {t('w.sticker.fdb87b')}
                </span>
              </span>
            </button>
            <IconButton
              size="sm"
              label={t('w.sticker.38e2f3', set.title)}
              disabled={index === 0}
              onClick={() => move(index, index - 1)}
            >
              <ArrowUpGlyph />
            </IconButton>
            <IconButton
              size="sm"
              label={t('w.sticker.451b58', set.title)}
              disabled={index === active.length - 1}
              onClick={() => move(index, index + 1)}
            >
              <ArrowDownGlyph />
            </IconButton>
            <IconButton size="sm" label={t('w.sticker.fe2d53', set.title)} onClick={() => share(set)}>
              <LinkGlyph />
            </IconButton>
            <Button size="sm" variant="text" onClick={() => guard(stickerLibrary().setArchived(set.id, true))}>
              {t('w.sticker.ddfde7')}
            </Button>
            <IconButton
              size="sm"
              variant="danger"
              label={t('w.sticker.6a13fa', set.title)}
              onClick={() => guard(stickerLibrary().uninstall(set.id))}
            >
              <CloseGlyph />
            </IconButton>
          </li>
        ))}
      </ol>
      {archived.length > 0 ? (
        <>
          <h3 className="tg-sticker-manage__heading">{t('w.sticker.5cfbea')}</h3>
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
                    <span className="tg-sticker-manage__count">
                      {set.stickers.length} {t('w.sticker.fdb87b')}
                    </span>
                  </span>
                </button>
                <Button size="sm" variant="tonal" onClick={() => guard(stickerLibrary().setArchived(set.id, false))}>
                  {t('w.sticker.18362b')}
                </Button>
                <IconButton
                  size="sm"
                  variant="danger"
                  label={t('w.sticker.6a13fa', set.title)}
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
