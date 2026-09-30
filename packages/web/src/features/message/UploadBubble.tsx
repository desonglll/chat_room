/**
 * The local placeholder for an attachment still hashing / uploading. It uses the same row
 * and frame as a real message so the swap to the acknowledged message does not jump; it
 * offers no menu, because there is no server message to act on yet.
 */
import type { UploadMessage } from '@tg/core'
import { computeBubbleLayout } from './bubbleLayout'
import { BubbleFrame } from './BubbleFrame'
import { formatBytes } from './content/attachmentKind'
import { MessageRow } from './MessageRow'
import { ClockGlyph, FailedGlyph, FileGlyph } from './icons'
import type { MessageRenderContext } from './types'

const PHASE_LABEL: Record<UploadMessage['phase'], string> = {
  queued: '排队中',
  hashing: '校验中',
  uploading: '上传中',
  deduplicating: '去重中',
  finalizing: '处理中',
}

export function uploadPercent(upload: UploadMessage): number {
  if (upload.total_bytes <= 0) return 0
  return Math.max(0, Math.min(100, Math.round((upload.processed_bytes / upload.total_bytes) * 100)))
}

export function UploadBubble({ upload, ctx }: { upload: UploadMessage; ctx: MessageRenderContext }) {
  const failed = upload.status === 'failed'
  const percent = uploadPercent(upload)
  const preview = upload.mime_type.startsWith('image/') && upload.preview_url !== ''
  const layout = computeBubbleLayout({
    groupPosition: ctx.groupPosition,
    isOutgoing: true,
    frame: preview ? 'media' : 'bubble',
    metaPlacement: preview ? 'overlay' : 'inline',
    hasSenderName: false,
    hasForward: false,
    hasReply: false,
    hasReactions: false,
  })
  const status = failed
    ? `上传失败${upload.error === '' ? '' : `：${upload.error}`}`
    : `${PHASE_LABEL[upload.phase]} ${percent}%`

  const progress = (
    <div className="tg-bubble__upload" data-failed={failed ? '' : undefined} role="status">
      <span className="tg-bubble__upload-track">
        <span className="tg-bubble__upload-bar" style={{ inlineSize: `${percent}%` }} />
      </span>
      <span className="tg-bubble__upload-label">
        {failed ? <FailedGlyph /> : <ClockGlyph />} {status}
      </span>
    </div>
  )

  return (
    <MessageRow
      ctx={{ ...ctx, isOutgoing: true, showAvatar: false }}
      selectionMode={false}
      reserveAvatar={false}
      avatar={{ label: '', emoji: '' }}
      menuItems={[]}
    >
      <BubbleFrame layout={layout} flushTop={preview} flushBottom={preview}>
        {preview ? (
          <div className="tg-bubble__media" data-uploading="">
            <img className="tg-bubble__image" src={upload.preview_url} alt={upload.file_name} />
            {progress}
          </div>
        ) : (
          <>
            <div className="tg-bubble__file">
              <span className="tg-bubble__file-icon">
                <FileGlyph />
              </span>
              <span className="tg-bubble__file-text">
                <span className="tg-bubble__file-name">{upload.file_name}</span>
                <span className="tg-bubble__file-size">{formatBytes(upload.size_bytes)}</span>
              </span>
            </div>
            {progress}
          </>
        )}
      </BubbleFrame>
    </MessageRow>
  )
}
