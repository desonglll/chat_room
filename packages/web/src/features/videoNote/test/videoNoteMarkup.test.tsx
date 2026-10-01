/** Server-rendered round video bubble, viewfinder and composer button (no DOM under `bun test`). */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MessageBubble, messageContent } from '../../message'
import { makeCtx, makeMessage } from '../../message/fixtures/bubbleFixtures'
import { VoiceRecordView } from '../../voice/VoiceRecordButton'
import { IDLE_RECORD_STATE, type RecordController, type RecordState } from '../../voice/recordController'
import { pressGesture } from '../../voice/recordGesture'
import '../../voice/register'
import '../register'
import { CameraGlyph } from '../glyphs'
import { idlePreviewPlays } from '../VideoNoteContent'
import { VideoNoteBody, type VideoNoteBodyProps } from '../VideoNoteBody'
import { VideoNoteViewfinder } from '../VideoNoteViewfinder'

const NOTE = { duration_ms: 14_200, thumbnail: '/9j/AAA=', listened: false }
const VIDEO = {
  id: 'v1',
  file_name: 'video_note.webm',
  mime_type: 'video/webm',
  size_bytes: 90_000,
  download_url: '/api/attachments/v1?key=k',
  is_sensitive: false,
}
const noop = () => {}

function body(overrides: Partial<VideoNoteBodyProps> = {}) {
  return renderToStaticMarkup(
    <VideoNoteBody
      url={VIDEO.download_url}
      durationMs={NOTE.duration_ms}
      thumbnail={NOTE.thumbnail}
      outgoing={false}
      active={false}
      moving={true}
      positionMs={0}
      listened={false}
      onActivate={noop}
      {...overrides}
    />,
  )
}

describe('video note registration', () => {
  test('a message carrying `video_note` is a round video, not a rectangular player', () => {
    const message = makeMessage({ content: '', attachment: VIDEO, video_note: NOTE, media_kind: 'video_note' })
    const kind = messageContent.resolve(message)
    expect(kind?.kind).toBe('video_note')
    expect(kind?.frame(message)).toBe('bare')
    expect(kind?.metaPlacement(message)).toBe('overlay')
    expect(messageContent.resolve(makeMessage({ content: '', attachment: VIDEO }))?.kind).toBe('video')
    const recalled = makeMessage({ attachment: VIDEO, video_note: NOTE, recalled_at: '2026-10-01T00:00:00Z' })
    expect(messageContent.resolve(recalled)?.kind).toBe('deleted')
  })

  test('the whole bubble draws the circle with its thumbnail as poster', () => {
    const html = renderToStaticMarkup(
      <MessageBubble message={makeMessage({ content: '', attachment: VIDEO, video_note: NOTE })} ctx={makeCtx()} />,
    )
    expect(html).toContain('class="tg-video-note"')
    expect(html).toContain('poster="data:image/jpeg;base64,/9j/AAA="')
    expect(html).toContain('播放视频消息，0:14')
    expect(html).not.toContain('tg-bubble__file')
  })
})

describe('video note bubble', () => {
  test('muted preview: muted looping video, duration, muted speaker, unwatched dot, no ring', () => {
    const html = body()
    expect(html).toMatch(/<video[^>]*muted=""[^>]*loop=""|<video[^>]*loop=""[^>]*muted=""/)
    expect(html).toContain('playsInline=""')
    expect(html).toContain('0:14')
    expect(html).toContain('aria-label="未观看"')
    expect(html).not.toContain('tg-video-note__ring')
    expect(html).not.toContain('tg-video-note__play')
  })

  test('not moving (reduced motion / off-screen): a play badge over the poster', () => {
    const html = body({ moving: false })
    expect(html).toContain('tg-video-note__play')
  })

  test('active: enlarged, progress ring, remaining time, pause label; watched hides the dot', () => {
    const html = body({ active: true, positionMs: 4_200, listened: true })
    expect(html).toContain('data-active="true"')
    expect(html).toContain('tg-video-note__ring')
    expect(html).toContain('0:10')
    expect(html).toContain('暂停视频消息')
    expect(html).not.toContain('未观看')
  })

  test('autoplay policy: only in the viewport, never under reduced motion', () => {
    expect(idlePreviewPlays(true, false)).toBe(true)
    expect(idlePreviewPlays(false, false)).toBe(false)
    expect(idlePreviewPlays(true, true)).toBe(false)
  })
})

function controller(): RecordController {
  return {
    getState: () => IDLE_RECORD_STATE,
    subscribe: () => noop,
    press: noop,
    move: noop,
    release: noop,
    send: noop,
    cancel: noop,
    dismissError: noop,
    sendFile: noop,
    dispose: noop,
  }
}

function cameraButton(state: RecordState) {
  return renderToStaticMarkup(
    <VoiceRecordView
      chatId="c1"
      replyTo={null}
      canSend
      sendFrame={() => true}
      micGlyph={<CameraGlyph />}
      sendGlyph={<i>send</i>}
      state={state}
      controller={controller()}
      onTap={noop}
      kind="video_note"
      idleLabel="按住录制视频消息"
      sendLabel="发送视频消息"
      waveform={false}
      overlay={<VideoNoteViewfinder preview={null} elapsedMs={state.elapsedMs} />}
    />,
  )
}

describe('camera mode of the composer button', () => {
  test('idle: labelled for video, no viewfinder', () => {
    const html = cameraButton(IDLE_RECORD_STATE)
    expect(html).toContain('按住录制视频消息')
    expect(html).toContain('data-kind="video_note"')
    expect(html).not.toContain('tg-video-note-rec')
  })

  test('recording: round viewfinder with the minute as a ring, timer, no waveform bars', () => {
    const html = cameraButton({
      ...IDLE_RECORD_STATE,
      phase: 'recording',
      gesture: pressGesture(0, 0, 0),
      elapsedMs: 15_000,
      levels: [0, 0, 0],
    })
    expect(html).toContain('视频消息取景框')
    expect(html).toContain('tg-video-note-rec__ring')
    expect(html).toContain('正在录制视频消息')
    expect(html).toContain('0:15,0')
    expect(html).not.toMatch(/tg-voice-rec__wave"[^>]*><span/)
    // A quarter of the minute: the ring's dash offset is three quarters of its length.
    const circumference = 2 * Math.PI * 48
    expect(html).toContain(`stroke-dashoffset="${circumference * 0.75}"`)
  })

  test('locked: sends a video message', () => {
    const html = cameraButton({
      ...IDLE_RECORD_STATE,
      phase: 'recording',
      gesture: { ...pressGesture(0, 0, 0), phase: 'locked', lockProgress: 1 },
      elapsedMs: 2_000,
    })
    expect(html).toContain('发送视频消息')
  })
})
