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
      .catch(() => setNotice('操作失败，请重试'))
      .finally(() => setBusy(false))
  }

  const footer = set ? (
    <div className="tg-sticker-set__actions">
      <Button
        variant="text"
        onClick={() =>
          void copyStickerSetLink(set.short_name).then((ok) => setNotice(ok ? '链接已复制' : '无法访问剪贴板'))
        }
      >
        复制链接
      </Button>
      {isInstalled && installed.archived ? (
        <Button variant="tonal" loading={busy} onClick={() => run(() => stickerLibrary().setArchived(set.id, false))}>
          取消归档
        </Button>
      ) : null}
      {isInstalled ? (
        <Button variant="danger" loading={busy} onClick={() => run(() => stickerLibrary().uninstall(set.id))}>
          移除 {set.stickers.length} 个贴纸
        </Button>
      ) : (
        <Button variant="filled" loading={busy} onClick={() => run(() => stickerLibrary().install(set.id), '已添加')}>
          添加 {set.stickers.length} 个贴纸
        </Button>
      )}
    </div>
  ) : null

  return (
    <Modal
      open
      onClose={onClose}
      title={set?.title ?? '贴纸包'}
      description={set ? `${set.stickers.length} 个贴纸 · ${set.short_name}` : undefined}
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
          <Spinner size="md" label="正在加载贴纸包" />
        </div>
      ) : (
        <p className="tg-sticker-tab__state">{load.state === 'missing' ? '贴纸包不存在' : '贴纸包加载失败'}</p>
      )}
      {notice ? (
        <p className="tg-sticker-set__notice" role="status">
          {notice}
        </p>
      ) : null}
    </Modal>
  )
}
