/**
 * The five built-in content kinds: text, image, video, file, deleted. Each is a plain
 * `MessageContentProps` component — exactly what a later task registers for its own kind.
 */
import type { MessageContentProps } from './contentTypes'
import { MessageText } from './MessageText'
import { MediaFrame } from './MediaFrame'
import { BubbleImage } from './BubbleImage'
import { AutoDownloadGate } from '../../settings/storage/AutoDownloadGate'
import { formatBytes, hasCaption } from './attachmentKind'
import { DeletedGlyph, FileGlyph, PlayGlyph } from '../icons'
import { t } from '../../../i18n/index'

export function TextContent({ message, metaSpacer }: MessageContentProps) {
  return <MessageText text={message.content} metaSpacer={metaSpacer} />
}

function Caption({ message, metaSpacer }: MessageContentProps) {
  return hasCaption(message) ? (
    <MessageText text={message.content} metaSpacer={metaSpacer} className="tg-bubble__caption" />
  ) : null
}

export function ImageContent(props: MessageContentProps) {
  const { message, actions } = props
  const attachment = message.attachment
  if (attachment === null) return null
  return (
    <>
      <MediaFrame attachment={attachment} actions={actions} label={t('w.message.d5948e', attachment.file_name)}>
        <AutoDownloadGate kind="photo" sizeBytes={attachment.size_bytes}>
          <BubbleImage src={attachment.download_url} name={attachment.file_name} />
        </AutoDownloadGate>
      </MediaFrame>
      <Caption {...props} />
    </>
  )
}

export function VideoContent(props: MessageContentProps) {
  const { message, actions } = props
  const attachment = message.attachment
  if (attachment === null) return null
  return (
    <>
      <MediaFrame attachment={attachment} actions={actions} label={t('w.message.c81777', attachment.file_name)}>
        <AutoDownloadGate kind="video" sizeBytes={attachment.size_bytes}>
          <video className="tg-bubble__image" src={attachment.download_url} preload="metadata" muted playsInline />
        </AutoDownloadGate>
        <span className="tg-bubble__play">
          <PlayGlyph />
        </span>
      </MediaFrame>
      <Caption {...props} />
    </>
  )
}

export function FileContent(props: MessageContentProps) {
  const { message, metaSpacer } = props
  const attachment = message.attachment
  if (attachment === null) return null
  const captioned = hasCaption(message)
  return (
    <>
      <a
        className="tg-bubble__file"
        href={attachment.download_url}
        download={attachment.file_name}
        onClick={(event) => event.stopPropagation()}
      >
        <span className="tg-bubble__file-icon">
          <FileGlyph />
        </span>
        <span className="tg-bubble__file-text">
          <span className="tg-bubble__file-name">{attachment.file_name}</span>
          <span className="tg-bubble__file-size">
            {formatBytes(attachment.size_bytes)}
            {captioned ? null : metaSpacer}
          </span>
        </span>
      </a>
      <Caption {...props} />
    </>
  )
}

export function DeletedContent({ metaSpacer }: MessageContentProps) {
  return (
    <div className="tg-bubble__text tg-bubble__deleted">
      <DeletedGlyph />
      <span>{t('w.message.26b4ea')}</span>
      {metaSpacer}
    </div>
  )
}
