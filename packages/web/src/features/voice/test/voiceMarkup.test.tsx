/** Server-rendered voice bubble and recording panel (packages/web has no DOM under `bun test`). */
import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MessageBubble, messageContent } from '../../message'
import { makeCtx, makeMessage } from '../../message/fixtures/bubbleFixtures'
import '../register'
import { VoiceBody } from '../VoiceContent'
import { VoiceRecordView } from '../VoiceRecordButton'
import { IDLE_RECORD_STATE, type RecordController, type RecordState } from '../recordController'
import { pressGesture } from '../recordGesture'

const VOICE = { duration_ms: 7_400, waveform: Array.from({ length: 100 }, (_, i) => i % 32), listened: false }
const AUDIO = {
  id: 'a1',
  file_name: 'voice.webm',
  mime_type: 'audio/webm',
  size_bytes: 9_000,
  download_url: '/api/attachments/a1?key=k',
  is_sensitive: false,
}
const noop = () => {}
const player = { toggle: noop, seek: noop, cycleRate: noop }

function body(overrides: Partial<Parameters<typeof VoiceBody>[0]> = {}) {
  return renderToStaticMarkup(
    <VoiceBody
      track={{ messageId: 'm1', url: AUDIO.download_url, durationMs: VOICE.duration_ms }}
      waveform={VOICE.waveform}
      listened={false}
      loaded={false}
      playing={false}
      positionMs={0}
      rate={1}
      player={player}
      metaSpacer={null}
      {...overrides}
    />,
  )
}

describe('voice content registration', () => {
  test('a message carrying `voice` resolves to the voice kind; a plain audio file stays a file', () => {
    expect(messageContent.resolve(makeMessage({ content: '', attachment: AUDIO, voice: VOICE }))?.kind).toBe('voice')
    expect(messageContent.resolve(makeMessage({ content: '', attachment: AUDIO }))?.kind).toBe('file')
    const recalled = makeMessage({ attachment: AUDIO, voice: VOICE, recalled_at: '2026-10-01T00:00:00Z' })
    expect(messageContent.resolve(recalled)?.kind).toBe('deleted')
  })

  test('the whole bubble renders the player instead of a file card', () => {
    const html = renderToStaticMarkup(
      <MessageBubble message={makeMessage({ content: '', attachment: AUDIO, voice: VOICE })} ctx={makeCtx()} />,
    )
    expect(html).toContain('class="tg-voice"')
    expect(html).toContain('播放语音')
    expect(html).not.toContain('tg-bubble__file')
  })
})

describe('voice bubble', () => {
  test('idle: duration, unlistened dot, waveform bars sized by duration, no speed chip', () => {
    const html = body()
    expect(html).toContain('0:07')
    expect(html).toContain('aria-label="未收听"')
    expect(html.match(/class="tg-voice__bar"/g)?.length).toBe(30)
    expect(html).not.toContain('tg-voice__rate')
  })

  test('playing: position, played bars, pause control, speed chip; listened hides the dot', () => {
    const html = body({ listened: true, loaded: true, playing: true, positionMs: 3_700, rate: 1.5 })
    expect(html).toContain('0:03')
    expect(html).toContain('暂停语音')
    expect(html).toContain('1.5x')
    expect(html).not.toContain('未收听')
    expect(html.match(/data-played="true"/g)?.length).toBe(15)
    expect(html).toContain('aria-valuenow="4"')
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
    dispose: noop,
  }
}

function recorder(state: RecordState) {
  return renderToStaticMarkup(
    <VoiceRecordView
      chatId="c1"
      replyTo={null}
      canSend
      sendFrame={() => true}
      micGlyph={<i>mic</i>}
      sendGlyph={<i>send</i>}
      state={state}
      controller={controller()}
    />,
  )
}

describe('recording panel', () => {
  test('idle: only the mic button', () => {
    const html = recorder(IDLE_RECORD_STATE)
    expect(html).toContain('按住录制语音消息')
    expect(html).not.toContain('tg-voice-rec')
  })

  test('holding: timer, live bars, slide-to-cancel hint, rising lock', () => {
    const html = recorder({
      ...IDLE_RECORD_STATE,
      phase: 'recording',
      gesture: { ...pressGesture(0, 0, 0), cancelProgress: 0.25 },
      elapsedMs: 3_450,
      levels: [0.1, 0.5, 0.9],
    })
    expect(html).toContain('0:03,4')
    expect(html).toContain('滑动取消')
    expect(html).toContain('tg-voice-lock')
    expect(html).toContain('--tg-voice-cancel-progress:0.25')
    expect(html).toContain('data-recording="true"')
  })

  test('locked: a cancel button, and the mic turns into send', () => {
    const html = recorder({
      ...IDLE_RECORD_STATE,
      phase: 'recording',
      gesture: { ...pressGesture(0, 0, 0), phase: 'locked', lockProgress: 1 },
      elapsedMs: 12_000,
    })
    expect(html).toContain('>取消<')
    expect(html).toContain('发送语音')
    expect(html).toContain('<i>send</i>')
    expect(html).not.toContain('滑动取消')
  })

  test('an error is announced', () => {
    expect(recorder({ ...IDLE_RECORD_STATE, error: '麦克风不可用' })).toContain('role="alert"')
  })
})
