/**
 * A sticker set by short name: its stickers, and add / remove / unarchive / share.
 * Opened from the panel's menu, a sticker bubble, the management page, or an
 * `/addstickers/<name>` link. The installed copy is shown at once when there is one; the
 * server copy (authoritative for sets the viewer does not have) replaces it on arrival.
 */
import { useEffect, useMemo, useState } from 'react'
import { stickerStore, type StickerSet } from '@tg/core'
import { Button, Modal, Spinner } from '@tg/ui'
import { useStore } from 'zustand/react'
import { StickerGrid } from '../panel/StickerGrid'
import { stickerLibrary } from '../stickerLibrary'
import { copyStickerSetLink } from './copyLink'
import { t } from '../../../i18n/index'
import './manage.css'

export interface StickerSetModalProps {
  shortName: string
  onClose(): void
}

type Load = { state: 'loading' } | { state: 'missing' } | { state: 'error' } | { state: 'ready'; set: StickerSet }

export function StickerSetModal({ shortName, onClose }: StickerSetModalProps) {
  const installed = useStore(stickerStore, (state) => state.sets.find((set) => set.short_name === shortName))
  const [load, setLoad] = useState<Load>({ state: 'loading' })
  const [busy, setBusy] = useState(false)
  const [notice, setNotice] = useState('')

  useEffect(() => {
    let cancelled = false
    void stickerLibrary()
      .ensureLoaded()
      .catch(() => undefined)
    stickerLibrary()
      .lookupSet(shortName)
      .then((set) => !cancelled && setLoad(set ? { state: 'ready', set } : { state: 'missing' }))
      .catch(() => !cancelled && setLoad({ state: 'error' }))
    return () => {
      cancelled = true
    }
  }, [shortName])

  const set = installed ?? (load.state === 'ready' ? load.set : null)
  const sections = useMemo(() => (set ? [{ id: set.id, title: '', stickers: set.stickers }] : []), [set])
  const isInstalled = installed !== undefined

  const run = (action: () => Promise<void>, done?: string) => {
    setBusy(true)
    setNotice('')
    action()
      .then(() => done && setNotice(done))
      .catch(() => setNotice(t('w.sticker.51d3cb')))
      .finally(() => setBusy(false))
  }

  const footer = set ? (
    <div className="tg-sticker-set__actions">
      <Button
        variant="text"
        onClick={() =>
          void copyStickerSetLink(set.short_name).then((ok) =>
            setNotice(ok ? t('w.sticker.3e1fac') : t('w.sticker.91aaca')),
          )
        }
      >
        {t('w.sticker.abb22b')}
      </Button>
      {isInstalled && installed.archived ? (
        <Button variant="tonal" loading={busy} onClick={() => run(() => stickerLibrary().setArchived(set.id, false))}>
          {t('w.sticker.18362b')}
        </Button>
      ) : null}
      {isInstalled ? (
        <Button variant="danger" loading={busy} onClick={() => run(() => stickerLibrary().uninstall(set.id))}>
          {t('w.sticker.2f752c')} {set.stickers.length} {t('w.sticker.fdb87b')}
        </Button>
      ) : (
        <Button
          variant="filled"
          loading={busy}
          onClick={() => run(() => stickerLibrary().install(set.id), t('w.sticker.578286'))}
        >
          {t('w.sticker.94191c')} {set.stickers.length} {t('w.sticker.fdb87b')}
        </Button>
      )}
    </div>
  ) : null

  return (
    <Modal
      open
      onClose={onClose}
      title={set?.title ?? t('w.sticker.e0a6f3')}
      description={set ? t('w.sticker.438da3', set.stickers.length, set.short_name) : undefined}
      footer={footer}
      size="md"
      className="tg-sticker-set"
    >
      {set ? (
        <div className="tg-sticker-set__grid">
          <StickerGrid sections={sections} onPick={() => undefined} label={set.title} />
        </div>
      ) : load.state === 'loading' ? (
        <div className="tg-sticker-tab__state">
          <Spinner size="md" label={t('w.sticker.95e96f')} />
        </div>
      ) : (
        <p className="tg-sticker-tab__state">
          {load.state === 'missing' ? t('w.sticker.5087b7') : t('w.sticker.4922b4')}
        </p>
      )}
      {notice ? (
        <p className="tg-sticker-set__notice" role="status">
          {notice}
        </p>
      ) : null}
    </Modal>
  )
}
