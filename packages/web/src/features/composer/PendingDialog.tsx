/**
 * Telegram's «发送 3 张图片» dialog: previews of every pending file (thumbnails for
 * photos/videos, a file row otherwise), per-item remove and progress, one caption for
 * the batch, «以文件形式发送», and send. Enter in the caption sends.
 */
import type { PendingAttachment } from '@tg/core'
import { pendingBatchTitle } from '@tg/core'
import { Button, Checkbox, Modal } from '@tg/ui'
import { CloseGlyph, FileGlyph } from './icons'
import type { PendingBatchHandle } from './usePendingBatch'
import { t } from '../../i18n/index'

export interface PendingDialogProps {
  pending: PendingBatchHandle
}

const sizeLabel = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function PendingItem({ item, pending }: { item: PendingAttachment<File>; pending: PendingBatchHandle }) {
  const visual = item.kind === 'photo' || item.kind === 'video'
  return (
    <li className="tg-compose__pending-item" data-kind={item.kind} data-status={item.status}>
      {visual ? (
        item.kind === 'photo' ? (
          <img className="tg-compose__pending-thumb" src={pending.previewUrl(item)} alt={item.name} />
        ) : (
          <video className="tg-compose__pending-thumb" src={pending.previewUrl(item)} muted aria-label={item.name} />
        )
      ) : (
        <span className="tg-compose__pending-file">
          <span className="tg-compose__pending-file-icon">
            <FileGlyph />
          </span>
          <span className="tg-compose__pending-file-text">
            <span className="tg-compose__pending-file-name">{item.name}</span>
            <span className="tg-compose__pending-file-size">{sizeLabel(item.size)}</span>
          </span>
        </span>
      )}
      {item.status === 'uploading' ? (
        <progress
          className="tg-compose__pending-progress"
          max={100}
          value={item.progress}
          aria-label={t('w.composer.211354')}
        />
      ) : null}
      {item.status === 'failed' ? (
        <span className="tg-compose__pending-error" role="alert">
          {item.error}
        </span>
      ) : null}
      {item.status === 'ready' || item.status === 'failed' ? (
        <button
          type="button"
          className="tg-compose__pending-remove"
          aria-label={t('w.composer.6a13fa', item.name)}
          disabled={pending.sending}
          onClick={() => pending.remove(item.id)}
        >
          <CloseGlyph size={16} />
        </button>
      ) : null}
    </li>
  )
}

export function PendingDialog({ pending }: PendingDialogProps) {
  const { batch } = pending
  const open = batch.items.length > 0
  const retry = batch.items.some((item) => item.status === 'failed')
  return (
    <Modal
      open={open}
      onClose={() => {
        if (!pending.sending) pending.clear()
      }}
      title={pendingBatchTitle(batch)}
      size="md"
      className="tg-compose__pending"
      footer={
        <>
          <Button variant="text" disabled={pending.sending} onClick={pending.clear}>
            {t('w.composer.4d0b46')}
          </Button>
          <Button variant="filled" loading={pending.sending} onClick={() => void pending.send()}>
            {retry ? t('w.composer.e2d53a') : t('w.composer.1214d6')}
          </Button>
        </>
      }
    >
      <ul className="tg-compose__pending-list" aria-label={t('w.composer.278a4b')}>
        {batch.items.map((item) => (
          <PendingItem key={item.id} item={item} pending={pending} />
        ))}
      </ul>
      {pending.rejected.length > 0 ? (
        <p className="tg-compose__pending-rejected" role="status">
          {pending.rejected.map((file) => `${file.name}：${file.reason}`).join('；')}
        </p>
      ) : null}
      <Checkbox
        checked={batch.sendAsFiles}
        onCheckedChange={pending.setSendAsFiles}
        disabled={pending.sending}
        label={t('w.composer.e02394')}
      />
      <input
        className="tg-compose__pending-caption"
        value={batch.caption}
        placeholder={t('w.composer.812512')}
        aria-label={t('w.composer.26670d')}
        maxLength={4096}
        disabled={pending.sending}
        onChange={(event) => pending.setCaption(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault()
            void pending.send()
          }
        }}
      />
    </Modal>
  )
}
