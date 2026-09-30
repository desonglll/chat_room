/**
 * The bubble body of a GIF message (TG-103 content registry): the GIF, borderless, with
 * the time overlaid; a tap plays or pauses it. Captions, if any, render like a photo's.
 */
import { MessageText, type MessageContentProps } from '../../message'
import { GifPlayer } from '../GifPlayer'

export function GifMessage({ message, metaSpacer }: MessageContentProps) {
  const attachment = message.attachment
  if (attachment === null) return null
  return (
    <>
      <GifPlayer
        src={attachment.download_url}
        mimeType={attachment.mime_type}
        label="GIF"
        className="tg-bubble__media tg-gif--bubble"
        tapToToggle
      />
      {message.content.trim() === '' ? null : (
        <MessageText text={message.content} metaSpacer={metaSpacer} className="tg-bubble__caption" />
      )}
    </>
  )
}
